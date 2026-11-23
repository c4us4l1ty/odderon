//! Body-fixed frames: turning an inertial position into "above this point on the surface".
//!
//! ## What this is for
//!
//! An orbit is a curve in inertial space. A *ground station* is a place on a rotating body. Every question
//! the game cares about — can the player see the satellite, when is the launch window, where does the
//! re-entry track cross the coast — is a question about the angle between those two, and that angle changes
//! 360° per rotation.
//!
//! ## The one number that matters, and why it is not a round one
//!
//! The transformation needs the body's **rotation angle** at the epoch, `W`, measured from the inertial
//! reference direction to the body's prime meridian. It is:
//!
//! ```text
//! W(t) = W₀ + Ẇ · (t − t₀)
//! ```
//!
//! `W₀` is the prime meridian angle at `J2000`, and it is a **published constant per body**, not a zero.
//! Using zero — the obvious default — puts the prime meridian somewhere arbitrary and silently shifts every
//! ground position by up to half a rotation. There is no error, no NaN, and no visual cue: the satellite
//! still orbits, the ground station still exists, and they are simply never in the same place. That is the
//! failure mode this module is written to make impossible, and it is why [`BodyFixed`] requires `W0` as a
//! constructor argument rather than defaulting it.
//!
//! ## Why the constants are IAU 2015 and not 2009
//!
//! `DATA-LICENSING`-style discipline applies to *physical* constants too: pick an authority, name it, and do
//! not mix. The IAU Working Group on Cartographic Coordinates and Rotational Elements published revised
//! values in 2009 and again in 2015, and **for several bodies the two disagree about the prime meridian by
//! tens of degrees** — Mars differs by about 0.1°, but the Moon and the outer planets differ more. Mixing
//! them produces a frame that is self-consistent and wrong.
//!
//! This module ships **Earth only**, for a reason stated plainly: Earth's orientation is the one where a
//! small error is both likely and expensive, because the player starts there. The general `BodyFixed` type
//! is body-agnostic and tested against synthetic constants; populating it for other bodies is a data task
//! that should happen when a mission needs it, not speculatively.
//!
//! ## Frames, spelled out
//!
//! * **Inertial (ICRF-aligned, geocentric)** — what [`crate::State`] is in.
//! * **Body-fixed (ITRF-aligned for Earth)** — x through the prime meridian at the equator, z through the
//!   pole, y completing the right-handed set.
//!
//! The transform is a single rotation about z by `−W`, because the body rotates *eastward*: a point fixed on
//! the surface moves to a *larger* inertial longitude as time passes, so converting inertial→body-fixed
//! subtracts the accumulated rotation. Getting the sign backwards makes the planet spin the wrong way,
//! which is subtle in a still image and obvious the moment anything is tracked.

use crate::provenance::Quantity;
use crate::{cos_rad, sin_rad, Vec3};
use odderon_dmath::{atan2, sqrt, TAU};

/// A body's rotation model: where its prime meridian points, and how fast it turns.
///
/// Deliberately *not* `Default`. A default rotation would silently be "prime meridian at the inertial
/// origin, not rotating", which is the exact bug this module exists to prevent.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BodyFixed {
    /// Prime meridian angle at `J2000`, radians.
    pub w0: Quantity,
    /// Rotation rate, rad/s. Positive for prograde (eastward) spin.
    pub rate: Quantity,
}

/// Why a rotation angle could not be produced.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FrameError {
    /// The body does not publish a prime-meridian constant, so no honest angle exists.
    ///
    /// The alternative — assuming `W₀ = 0` — is the failure mode documented in the module header.
    NoPrimeMeridianConstant,
    /// A quantity needed for the transform is not known.
    PropertyUnknown,
}

impl BodyFixed {
    /// Rotation angle `W` at `dt` seconds after `J2000`, wrapped into `[0, 2π)`.
    ///
    /// Wrapping matters: over a game session `Ẇ·t` for Earth reaches `2π` in 86 164 s, and an unwrapped
    /// angle loses precision in its low bits as it grows. `sin`/`cos` are periodic so the answer is the same,
    /// but the *derived* quantities a caller computes from `W` (a longitude, a difference) are not, and
    /// subtracting two large nearly-equal angles is the classic way to lose a ground track to rounding.
    pub fn angle_at(&self, dt: f64) -> Option<f64> {
        if !dt.is_finite() {
            return None;
        }
        let w0 = self.w0.value;
        let rate = self.rate.value;
        if !w0.is_finite() || !rate.is_finite() {
            return None;
        }
        let raw = w0 + rate * dt;
        Some(wrap_two_pi(raw))
    }

    /// Inertial → body-fixed.
    ///
    /// Rotates by `−W` about z. Returns `None` when the rotation angle is not available, rather than
    /// returning a position that is confidently half a world away from the truth.
    pub fn to_body_fixed(&self, inertial: Vec3, dt: f64) -> Option<Vec3> {
        let w = self.angle_at(dt)?;
        let c = cos_rad(w);
        let s = sin_rad(w);
        // rotate by -W:  x' =  x cos W + y sin W
        //                y' = -x sin W + y cos W
        Some(Vec3::new(
            inertial.x * c + inertial.y * s,
            -inertial.x * s + inertial.y * c,
            inertial.z,
        ))
    }

    /// Body-fixed → inertial. The exact inverse of [`BodyFixed::to_body_fixed`].
    pub fn to_inertial(&self, body: Vec3, dt: f64) -> Option<Vec3> {
        let w = self.angle_at(dt)?;
        let c = cos_rad(w);
        let s = sin_rad(w);
        Some(Vec3::new(
            body.x * c - body.y * s,
            body.x * s + body.y * c,
            body.z,
        ))
    }
}

/// Wrap an angle into `[0, 2π)`.
///
/// The input can be large: `Ẇ·t` over a year at Earth is about 2.3e6 radians. A single conditional is not
/// enough, so this uses a remainder against `TAU`. `f64::rem_euclid` is `std`, hence the manual form.
fn wrap_two_pi(x: f64) -> f64 {
    if !x.is_finite() {
        return x;
    }
    // `f64::floor` is a host method and unavailable; the cast to i64 truncates toward zero, which is what
    // a floor is for a positive quotient and one step off for a negative one - corrected immediately below
    // by the `r < 0` branch. Truncation is also exact here, because `x/TAU` is far inside i64's range.
    let q = x / TAU;
    let n = if q >= 0.0 { q as i64 } else { (q as i64) - 1 };
    let r = x - (n as f64) * TAU;
    if r < 0.0 {
        r + TAU
    } else if r >= TAU {
        r - TAU
    } else {
        r
    }
}

/// Geodetic latitude and longitude of a body-fixed position, in radians.
///
/// `longitude` is in `(−π, π]`, positive eastward — the convention a map uses. The kernel's internal angles
/// are `[0, 2π)`; converting to the map convention at the display boundary is the right place to do it,
/// because a longitude of `4.9` rad is technically correct and useless to a player.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Geodetic {
    /// Geocentric latitude, radians, `[−π/2, π/2]`.
    ///
    /// **Geocentric, not geodetic.** The distinction is up to 0.19° at mid-latitudes for Earth, which is
    /// 21 km on the surface — large enough to matter for a launch site and small enough to be forgotten. The
    /// geodetic conversion needs the flattening and a solver; this is the honest, simple quantity until that
    /// exists, and the name says which one it is.
    pub latitude: f64,
    /// Longitude, radians, `(−π, π]`, positive eastward.
    pub longitude: f64,
    /// Distance from the body's centre, km — not height above the surface.
    pub radius: f64,
}

/// Body-fixed position → latitude, longitude, radius.
pub fn geodetic_of(body_fixed: Vec3) -> Geodetic {
    let r = body_fixed.norm();
    let lat = if r > 0.0 {
        // asin(z/r) via atan2, which stays well conditioned near the poles where asin's derivative blows up
        atan2(body_fixed.z, sqrt(body_fixed.x * body_fixed.x + body_fixed.y * body_fixed.y))
    } else {
        0.0
    };
    let lon = atan2(body_fixed.y, body_fixed.x);
    Geodetic {
        latitude: lat,
        longitude: lon,
        radius: r,
    }
}
