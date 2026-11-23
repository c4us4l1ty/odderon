//! Ground stations and look angles: *can this observer see that satellite, right now?*
//!
//! ## Why this is the first playable thing
//!
//! Everything before this described the universe. This module lets the universe be **observed**, which is
//! the smallest honest unit of gameplay in a game about knowing where things are. A pass prediction is a
//! prediction about the player's own horizon, and every later system — launch windows, comms links,
//! astrometry, "Be Your Own Gaia" — is this calculation with more terms in it.
//!
//! ## The three angles
//!
//! * **Elevation** — the angle above the observer's local horizon. Negative means below it: the body is
//!   occluded by the planet itself, not merely "far away".
//! * **Azimuth** — the compass bearing, measured clockwise from **north**. 0 = north, 90 = east.
//! * **Range** — straight-line distance from observer to target, km.
//!
//! ## The trap this module exists to avoid: geodetic is not geocentric
//!
//! A station's latitude on a map is **geodetic** — measured against the reference ellipsoid, with the
//! normal to that ellipsoid as "up". A line from the body's centre to the same station is **geocentric**,
//! and at 45° latitude the two differ by about **0.19°**, which is **21 km** on Earth's surface.
//!
//! That matters here for a specific reason: the local horizon is perpendicular to *geodetic* up. Using the
//! centre-to-station direction as up tilts every elevation angle by up to 0.19° near the horizon, which at
//! a 400 km orbit moves the predicted start of a pass by several seconds. Small, systematic, and invisible
//! in a test that only checks "elevation is positive when overhead".
//!
//! So [`Site`] is constructed with `from_geodetic`, which solves the ellipsoid properly, and the up vector
//! is the ellipsoid normal — not the radius vector.
//!
//! ## The other trap: elevation means nothing without a range sign
//!
//! An observer *inside* the body (a bug, an uninitialised state, a bad altitude) still gets a well-defined
//! elevation angle, and it is meaningless. The constructor refuses non-finite input, and [`look_at`]
//! returns the range so a caller can tell a horizon grazing from an observer who is nowhere near the
//! surface.

use crate::frames::BodyFixed;
use crate::provenance::Lookup;
use crate::{atan2, sqrt, Vec3};
use odderon_dmath::{asin, TAU};

/// A direction is undefined; the body is exactly at the observer.
///
/// Returned rather than producing a `NaN` azimuth, because a NaN bearing reaching a display is the kind of
/// failure that looks like a rendering bug for a week.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    /// Degrees above the local horizon. Negative means the planet is between observer and target.
    pub elevation_deg: f64,
    /// Degrees clockwise from north, in `[0, 360)`.
    pub azimuth_deg: f64,
    /// Straight-line distance, km.
    pub range_km: f64,
}

/// An observer fixed to a rotating body.
///
/// The station's body-fixed position is computed once; the inertial position is a function of time, because
/// the body turns underneath it. That is the whole of the observation geometry.
///
/// `PartialEq` so a test can assert on `Result<Site, SiteError>` directly. Equality is the exact-field
/// comparison, which is what a caller checking "did the constructor accept this input" wants; it is not a
/// geometric tolerance and should not be used as one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Site {
    /// Body-fixed position, km. Fixed to the surface for all time.
    pub fixed: Vec3,
    /// Unit vector along the ellipsoid normal — the local "up". Not the radius vector; see the module docs.
    pub up: Vec3,
    /// Unit vector pointing east.
    pub east: Vec3,
    /// Unit vector pointing north.
    pub north: Vec3,
    /// Geodetic latitude, radians.
    pub latitude: f64,
    /// Longitude, radians, positive east.
    pub longitude: f64,
    /// Height above the ellipsoid, km.
    pub height: f64,
}

/// Why a site could not be placed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SiteError {
    /// The body's flattening is not known, so geodetic latitude is not defined against it.
    FlatteningUnknown,
    /// The body's equatorial radius is not known.
    RadiusUnknown,
    /// Latitude outside `[−90°, 90°]` or a non-finite input.
    BadLatitude,
    /// Longitude or height non-finite.
    BadInput,
}

impl Site {
    /// Place a station at a geodetic latitude and longitude on a body.
    ///
    /// `lat` and `lon` in radians, `height` in km above the ellipsoid (0 for sea level).
    ///
    /// This solves the standard ellipsoid relation — the station sits at
    ///
    /// ```text
    /// N = a / sqrt(1 − e² sin²φ)
    /// x = (N + h) cos φ cos λ
    /// y = (N + h) cos φ sin λ
    /// z = (N(1 − e²) + h) sin φ
    /// ```
    ///
    /// and the local up is `(cos φ cos λ, cos φ sin λ, sin φ)` — the *geodetic* normal, which is the right
    /// answer and is one of the two things a naive implementation gets wrong.
    pub fn from_geodetic(
        body: &crate::catalogue::Body,
        lat: f64,
        lon: f64,
        height: f64,
    ) -> Result<Site, SiteError> {
        if !lat.is_finite() || !lon.is_finite() || !height.is_finite() {
            return Err(SiteError::BadInput);
        }
        // Clippy wants `contains` here and it is right: the comparison form leaves the NaN case to the
        // caller's discipline, whereas a range with an inclusive bound reads as the closed interval it is.
        if !(-core::f64::consts::FRAC_PI_2..=core::f64::consts::FRAC_PI_2).contains(&lat) {
            return Err(SiteError::BadLatitude);
        }
        let a = body.physical.equatorial_radius.value;
        if !crate::is_positive_finite(a) {
            return Err(SiteError::RadiusUnknown);
        }
        let f = match body.physical.flattening {
            Lookup::Known(q) => q.value,
            Lookup::NotKnown { .. } => return Err(SiteError::FlatteningUnknown),
            Lookup::NoSuchBody => return Err(SiteError::FlatteningUnknown),
        };
        // f = 0 is a sphere (legal, and the Moon is nearly one); f = 1 is a disc (not a body of revolution
        // anyone has found), and the inside of the ellipsoid solution divides by zero there.
        if !f.is_finite() || !(0.0..1.0).contains(&f) {
            return Err(SiteError::FlatteningUnknown);
        }

        let e2 = f * (2.0 - f);
        let sin_lat = sin_of(lat);
        let cos_lat = cos_of(lat);
        let sin_lon = sin_of(lon);
        let cos_lon = cos_of(lon);

        let n = a / sqrt(1.0 - e2 * sin_lat * sin_lat);

        let fixed = Vec3::new(
            (n + height) * cos_lat * cos_lon,
            (n + height) * cos_lat * sin_lon,
            (n * (1.0 - e2) + height) * sin_lat,
        );

        // Geodetic up: the ellipsoid normal. NOT the normalised position vector.
        let up = Vec3::new(cos_lat * cos_lon, cos_lat * sin_lon, sin_lat);
        // East: the direction of increasing longitude, which is Z × up normalised. At the poles this
        // degenerates, and east/north are arbitrary there - but they stay *orthonormal*, so the azimuth is
        // meaningless at the pole and the elevation is not. Returning a NaN azimuth instead would be worse.
        let mut east = Vec3::new(-sin_lon, cos_lon, 0.0);
        if east.norm() < 1e-12 {
            east = Vec3::new(1.0, 0.0, 0.0);
        }
        let east = east.unit();
        // North completes the right-handed set: north = up × east.
        let north = up.cross(east).unit();

        Ok(Site {
            fixed,
            up,
            east,
            north,
            latitude: lat,
            longitude: lon,
            height,
        })
    }

    /// This station's inertial position at `dt` seconds past J2000.
    pub fn inertial_position(&self, frame: &BodyFixed, dt: f64) -> Option<Vec3> {
        frame.to_inertial(self.fixed, dt)
    }
}

/// Compute elevation, azimuth and range from an observer to a target, both in the **same** frame.
///
/// Body-fixed is the natural frame: the observer does not move in it, so this is one subtraction and three
/// dot products rather than a rotation of both vectors every time. Callers with inertial states should
/// rotate the target into the body frame first (`frames::BodyFixed::to_body_fixed`).
pub fn look_at(site: &Site, target_fixed: Vec3) -> Option<Look> {
    let d = Vec3::new(
        target_fixed.x - site.fixed.x,
        target_fixed.y - site.fixed.y,
        target_fixed.z - site.fixed.z,
    );
    let range = d.norm();
    if !crate::is_positive_finite(range) {
        // The target is at the observer: no direction exists. Refusing beats a NaN bearing.
        return None;
    }
    let unit = Vec3::new(d.x / range, d.y / range, d.z / range);

    let up_component = unit.dot(site.up);
    // `asin` clamps its own argument to [-1, 1], which matters because `up` and `unit` are both
    // unit-length only to rounding: a target exactly at the zenith gives a dot product of 1 + 2e-16 and a
    // bare `asin` would return NaN for a perfectly ordinary geometry.
    let elevation = asin(up_component);

    // Azimuth from the horizontal components, clockwise from north through east.
    let east_component = unit.dot(site.east);
    let north_component = unit.dot(site.north);
    let az = atan2(east_component, north_component);
    let azimuth = if az < 0.0 { az + TAU } else { az };

    Some(Look {
        elevation_deg: elevation * 180.0 / core::f64::consts::PI,
        azimuth_deg: azimuth * 180.0 / core::f64::consts::PI,
        range_km: range,
    })
}

/// The geometric horizon: is the target above the observer's local plane at all?
///
/// Deliberately **geometric only**. It knows nothing about terrain, atmosphere, refraction or minimum
/// elevation masks — a radio horizon, for instance, extends a few degrees below the geometric one because
/// the atmosphere bends the path. Returning `false` from here means "the planet is in the way", which is a
/// statement of geometry and not of link feasibility.
pub fn above_horizon(look: &Look) -> bool {
    look.elevation_deg > 0.0
}

/// The maximum straight-line distance at which a target can be seen from a given radius.
///
/// `sqrt(r_target² − r_observer²)` — the tangent length. For a 400 km orbit this is about **2294 km**, and
/// a test asserts it, because it is the number that decides whether a LEO pass is a five-minute or an
/// eight-minute event.
pub fn horizon_range(observer_radius: f64, target_radius: f64) -> Option<f64> {
    if !crate::is_positive_finite(target_radius) || !crate::is_positive_finite(observer_radius) {
        return None;
    }
    if target_radius <= observer_radius {
        return Some(0.0);
    }
    Some(sqrt(
        target_radius * target_radius - observer_radius * observer_radius,
    ))
}

// ---------------------------------------------------------------------------------------------
// trig, from the kernel's own implementations
// ---------------------------------------------------------------------------------------------

fn sin_of(x: f64) -> f64 {
    crate::sin_rad(x)
}
fn cos_of(x: f64) -> f64 {
    crate::cos_rad(x)
}

/// Re-export the geodetic helper so a caller does not have to reach into `frames` for it.
pub use crate::frames::geodetic_of as body_fixed_geodetic;
