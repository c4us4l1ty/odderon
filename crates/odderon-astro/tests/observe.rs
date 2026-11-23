//! Ground stations and look angles, checked against geometry that is true independently of the code.
//!
//! The failure this file is built around is **geodetic vs geocentric latitude**, because it is the one that
//! produces a system which looks completely correct. A station at 45° latitude sits 21 km from where the
//! centre-to-surface direction says it does, and every elevation angle computed against that wrong "up" is
//! tilted by up to 0.19°. Nothing crashes; passes just start a few seconds early.
//!
//! So the tests below do not merely check that elevation is 90° when a satellite is overhead — that passes
//! for either convention. They check the *spheroidal* facts: the polar radius, the geodetic normal, and the
//! horizon range, all of which change if the shape is wrong.

use odderon_astro::catalogue::{EARTH, EARTH_FRAME};
use odderon_astro::observe::{above_horizon, horizon_range, look_at, Site, SiteError};
use odderon_astro::provenance::{Lookup, Quantity};
use odderon_astro::{radius, Vec3};
use odderon_dmath::{fabs, sqrt};

const RE: f64 = radius::EARTH;
const DEG: f64 = core::f64::consts::PI / 180.0;

/// A station on the equator at the prime meridian, at sea level.
fn equatorial_site() -> Site {
    Site::from_geodetic(&EARTH, 0.0, 0.0, 0.0).expect("equatorial site is valid")
}

// ---------------------------------------------------------------------------------------------
// 1. The ellipsoid is actually an ellipsoid
// ---------------------------------------------------------------------------------------------

/// The polar radius must be `a(1 − f)`, i.e. 6356.7523 km, not 6378.137.
///
/// This is the test that fails if flattening is ignored, and it is worth stating what the error would be:
/// treating Earth as a sphere puts the pole **21.4 km** too high. Every station at high latitude inherits
/// that error, and every elevation angle from it is wrong by a corresponding amount.
#[test]
fn the_pole_is_flattened_by_the_right_amount() {
    let north_pole = Site::from_geodetic(&EARTH, 90.0 * DEG, 0.0, 0.0).expect("pole is placeable");
    let r = north_pole.fixed.norm();

    let f = EARTH.physical.flattening.known().expect("Earth knows its shape").value;
    let polar = RE * (1.0 - f);

    assert!(
        fabs(r - polar) < 1e-6,
        "polar radius should be {polar:.4} km (WGS-84 6356.7523), got {r:.4} km - is flattening ignored?"
    );
    // and it must be distinguishable from the spherical answer
    assert!(fabs(r - RE) > 20.0, "the pole must not be at the equatorial radius");
}

/// The equatorial station sits exactly at the equatorial radius.
#[test]
fn the_equator_is_not_flattened() {
    let s = equatorial_site();
    assert!(fabs(s.fixed.norm() - RE) < 1e-9, "equatorial radius should be unchanged by flattening");
}

/// The local up is the **ellipsoid normal**, which is not the normalised position at mid-latitudes.
///
/// At 45° the two differ by 0.19°. This asserts the float is the geodetic one, by checking the angle between
/// `up` and the position vector is the geodetic-minus-geocentric difference and nothing else.
#[test]
fn local_up_is_the_geodetic_normal_not_the_radius_vector() {
    let lat = 45.0 * DEG;
    let s = Site::from_geodetic(&EARTH, lat, 0.0, 0.0).expect("valid");

    // `up` should be exactly (cos lat, 0, sin lat) for a station on the prime meridian
    assert!(fabs(s.up.x - lat.cos()) < 1e-12, "up.x should be cos(latitude)");
    assert!(fabs(s.up.z - lat.sin()) < 1e-12, "up.z should be sin(latitude)");

    // and the position vector's direction differs from it
    let radial = s.fixed.unit();
    let dot = (radial.x * s.up.x + radial.z * s.up.z).min(1.0);
    let separation = dot.acos() / DEG;
    assert!(
        (0.1..0.3).contains(&separation),
        "geodetic vs geocentric latitude at 45 deg should differ by about 0.19 deg, got {separation}"
    );
}

/// Up, east and north form an orthonormal right-handed frame everywhere, including at the poles.
#[test]
fn the_local_frame_is_orthonormal_everywhere() {
    for lat_deg in [-90.0, -45.0, 0.0, 23.5, 45.0, 89.9, 90.0] {
        let s = Site::from_geodetic(&EARTH, lat_deg * DEG, 37.0 * DEG, 0.0).expect("valid");
        assert!(fabs(s.up.norm() - 1.0) < 1e-12, "up not unit at {lat_deg}");
        assert!(fabs(s.east.norm() - 1.0) < 1e-12, "east not unit at {lat_deg}");
        assert!(fabs(s.north.norm() - 1.0) < 1e-12, "north not unit at {lat_deg}");
        assert!(fabs(s.up.dot(s.east)) < 1e-12, "up not perpendicular to east at {lat_deg}");
        assert!(fabs(s.up.dot(s.north)) < 1e-12, "up not perpendicular to north at {lat_deg}");
        assert!(fabs(s.east.dot(s.north)) < 1e-12, "east not perpendicular to north at {lat_deg}");
    }
}

// ---------------------------------------------------------------------------------------------
// 2. Look angles against cases with known answers
// ---------------------------------------------------------------------------------------------

/// A satellite directly overhead has elevation 90°, and azimuth is then meaningless but finite.
#[test]
fn overhead_is_ninety_degrees_of_elevation() {
    let s = equatorial_site();
    // straight up from the station, 400 km up
    let target = Vec3::new(s.fixed.x + 400.0 * s.up.x, s.fixed.y + 400.0 * s.up.y, s.fixed.z + 400.0 * s.up.z);

    let look = look_at(&s, target).expect("a target 400 km up has a direction");
    assert!(fabs(look.elevation_deg - 90.0) < 1e-9, "overhead should be 90 deg, got {}", look.elevation_deg);
    assert!(look.azimuth_deg.is_finite(), "azimuth at the zenith is arbitrary but must be finite");
    assert!(fabs(look.range_km - 400.0) < 1e-9, "range should be 400 km");
}

/// A satellite on the geometric horizon reads elevation exactly 0.
#[test]
fn the_horizon_is_exactly_zero_elevation() {
    let s = equatorial_site();
    // the tangent point: at angle acos(RE/r) round from the station
    let r = RE + 400.0;
    let phi = (RE / r).acos();
    let target = Vec3::new(r * phi.cos(), r * phi.sin(), 0.0);

    let look = look_at(&s, target).expect("valid");
    assert!(fabs(look.elevation_deg) < 1e-9, "horizon should be 0 deg, got {}", look.elevation_deg);
    assert!(!above_horizon(&look) || look.elevation_deg >= 0.0);
}

/// Azimuth is measured clockwise from north: a satellite due east reads 90°.
#[test]
fn azimuth_is_clockwise_from_north() {
    let s = Site::from_geodetic(&EARTH, 0.0, 0.0, 0.0).expect("valid");

    // due east is +y in the body-fixed frame at lon 0 (east = (-sin lon, cos lon, 0) = (0,1,0))
    let east_target = Vec3::new(s.fixed.x, s.fixed.y + 1000.0, 0.0);
    let look_e = look_at(&s, east_target).expect("valid");
    assert!(
        fabs(look_e.azimuth_deg - 90.0) < 1e-6,
        "due east should be azimuth 90, got {}",
        look_e.azimuth_deg
    );

    // due north from the equator is +z
    let north_target = Vec3::new(s.fixed.x, s.fixed.y, 1000.0);
    let look_n = look_at(&s, north_target).expect("valid");
    let az = look_n.azimuth_deg;
    assert!(
        fabs(az) < 1e-6 || fabs(az - 360.0) < 1e-6,
        "due north should be azimuth 0, got {az}"
    );

    // and south is 180
    let south_target = Vec3::new(s.fixed.x, s.fixed.y, -1000.0);
    let look_s = look_at(&s, south_target).expect("valid");
    assert!(fabs(look_s.azimuth_deg - 180.0) < 1e-6, "due south should be 180, got {}", look_s.azimuth_deg);
}

/// A geostationary satellite directly above the station is at 90°; one 90° of longitude away is below the
/// horizon, because the Earth's curvature wins at that separation.
#[test]
fn a_geostationary_satellite_sets_below_the_horizon_at_ninety_degrees_of_longitude() {
    let s = equatorial_site();
    let r_geo = 42_164.17;

    let overhead = look_at(&s, Vec3::new(r_geo, 0.0, 0.0)).expect("valid");
    assert!(fabs(overhead.elevation_deg - 90.0) < 1e-9);
    assert!(fabs(overhead.range_km - (r_geo - RE)) < 1e-6, "range should be the altitude");

    let far = look_at(&s, Vec3::new(0.0, r_geo, 0.0)).expect("valid");
    assert!(
        far.elevation_deg < 0.0,
        "a GEO satellite 90 deg of longitude away must be below the horizon, got {}",
        far.elevation_deg
    );
    assert!(!above_horizon(&far));
}

// ---------------------------------------------------------------------------------------------
// 3. The horizon range, which decides how long a pass lasts
// ---------------------------------------------------------------------------------------------

/// The horizon range at 400 km is 2294 km.
///
/// This is the number that sets a LEO pass duration: 2294 km each side of the closest approach at about
/// 7.7 km/s of ground-relative geometry gives a pass of minutes, not hours. A test that merely asserted
/// "the horizon exists" would not pin the number that matters.
#[test]
fn horizon_range_at_leo_is_the_tangent_length() {
    let r_target = RE + 400.0;
    let h = horizon_range(RE, r_target).expect("valid");
    assert!(fabs(h - 2294.0) < 5.0, "horizon range at 400 km should be ~2294 km, got {h:.1}");

    // and it must be the tangent, so the line is perpendicular to the radius at the observer
    // (Pythagoras: observer^2 + range^2 == target^2 is exactly the tangency condition when the
    //  observer is the tangent point)
    let lhs = RE * RE + h * h;
    assert!(fabs(lhs - r_target * r_target) / (r_target * r_target) < 1e-12, "not a tangent");
}

/// A target inside the observer's radius has no horizon range, and is reported as zero rather than NaN.
#[test]
fn horizon_range_is_zero_or_none_for_degenerate_input() {
    assert_eq!(horizon_range(RE, RE), Some(0.0), "a target on the surface is at zero range");
    assert_eq!(horizon_range(RE, RE * 0.5), Some(0.0), "a target inside the body is unreachable");
    assert!(horizon_range(0.0, RE).is_none(), "a zero observer radius is refused");
    assert!(horizon_range(RE, f64::NAN).is_none(), "NaN is refused");
}

// ---------------------------------------------------------------------------------------------
// 4. Refusals
// ---------------------------------------------------------------------------------------------

/// A body without a known flattening cannot host a geodetic station.
///
/// The Sun is such a body. Silently assuming a sphere would be exactly the 21 km error this module exists to
/// avoid, so the constructor refuses and the caller learns the frame is unavailable.
#[test]
fn a_body_with_no_flattening_cannot_host_a_station() {
    let sun = &odderon_astro::catalogue::SUN;
    assert_eq!(
        Site::from_geodetic(sun, 0.0, 0.0, 0.0),
        Err(SiteError::FlatteningUnknown),
        "the Sun's shape is not settled; a station on it must be refused"
    );
}

/// Out-of-range and non-finite inputs are refused.
#[test]
fn bad_site_input_is_refused() {
    assert_eq!(
        Site::from_geodetic(&EARTH, 91.0 * DEG, 0.0, 0.0),
        Err(SiteError::BadLatitude)
    );
    assert_eq!(
        Site::from_geodetic(&EARTH, -91.0 * DEG, 0.0, 0.0),
        Err(SiteError::BadLatitude)
    );
    assert_eq!(
        Site::from_geodetic(&EARTH, f64::NAN, 0.0, 0.0),
        Err(SiteError::BadInput)
    );
    assert_eq!(
        Site::from_geodetic(&EARTH, 0.0, f64::INFINITY, 0.0),
        Err(SiteError::BadInput)
    );
}

/// A target exactly at the observer has no direction, and says so rather than returning a NaN bearing.
#[test]
fn a_target_at_the_observer_has_no_direction() {
    let s = equatorial_site();
    assert!(look_at(&s, s.fixed).is_none(), "zero range must be refused");
}

/// A station can be placed on Mars, and its polar radius reflects that planet's much larger flattening.
#[test]
fn mars_hosts_a_station_and_is_more_oblate_than_earth() {
    let mars = &odderon_astro::catalogue::MARS;
    let pole = Site::from_geodetic(mars, 90.0 * DEG, 0.0, 0.0).expect("mars pole");
    let re = mars.physical.equatorial_radius.value;
    let f = mars.physical.flattening.known().expect("mars knows its shape").value;

    // Mars differs from a sphere by about 40 km, five times Earth's 21
    let diff = re - pole.fixed.norm();
    assert!(diff > 15.0, "Mars's polar flattening should be tens of km, got {diff:.1}");
    assert!(fabs(pole.fixed.norm() - re * (1.0 - f)) < 1e-6);
}

// ---------------------------------------------------------------------------------------------
// 5. Time: the station moves
// ---------------------------------------------------------------------------------------------

/// A station's inertial position returns to its start after one rotation period **of the stored rate**.
///
/// The period used here is `2π/Ẇ`, not the published 86 164.0905 s, and the difference is worth stating
/// because the first version of this test used the published figure and failed by **4.3 m**.
///
/// The catalogue stores `Ẇ = 7.2921150e-5` — eight significant figures, which is what WGS-84 and the IAU
/// report publish. `2π/Ẇ` is therefore **86164.1006 s**, not 86164.0905 s: a difference of 0.0101 s, which
/// is 7.4e-7 rad of rotation, which is 4.7 m at the equator. That is the rounding of the *constant*, not an
/// error in the frame, and asking for millimetres from an eight-figure constant is asking the arithmetic to
/// be more precise than the data.
///
/// Testing against `2π/Ẇ` isolates the thing this test is actually about — that the transform is periodic
/// and lossless over many rotations — from the constant's own precision, which is tested separately in
/// `catalogue.rs`.
#[test]
fn a_station_returns_to_its_inertial_position_after_one_rotation() {
    let s = Site::from_geodetic(&EARTH, 45.0 * DEG, 10.0 * DEG, 0.0).expect("valid");
    let rate = EARTH_FRAME.rate.value;
    let period = core::f64::consts::TAU / rate;

    let p0 = s.inertial_position(&EARTH_FRAME, 0.0).expect("frame available");
    let p1 = s.inertial_position(&EARTH_FRAME, period).expect("frame available");

    let d = (p1.x - p0.x).abs() + (p1.y - p0.y).abs() + (p1.z - p0.z).abs();
    assert!(
        d < 1e-6,
        "station should return exactly after 2pi/rate ({period:.4} s), moved {d} km"
    );

    // And state the size of the published-constant discrepancy explicitly, so it is a recorded fact rather
    // than a surprise: one sidereal day of the *published* length leaves the station 4.7 m short.
    let p_pub = s.inertial_position(&EARTH_FRAME, 86164.0905).expect("frame available");
    let d_pub = (p_pub.x - p0.x).abs() + (p_pub.y - p0.y).abs() + (p_pub.z - p0.z).abs();
    assert!(
        d_pub > 1e-4 && d_pub < 1e-2,
        "the published sidereal day should leave a few metres of residual, got {d_pub} km"
    );
}

/// The station's height above the ellipsoid is preserved when placed at altitude.
#[test]
fn altitude_is_measured_against_the_ellipsoid() {
    let h = 50.0;
    let s = Site::from_geodetic(&EARTH, 60.0 * DEG, 20.0 * DEG, h).expect("valid");
    let flat = Site::from_geodetic(&EARTH, 60.0 * DEG, 20.0 * DEG, 0.0).expect("valid");

    // The displacement from the surface point to the elevated point is exactly h along `up`.
    let d = Vec3::new(
        s.fixed.x - flat.fixed.x,
        s.fixed.y - flat.fixed.y,
        s.fixed.z - flat.fixed.z,
    );
    assert!(fabs(d.norm() - h) < 1e-9, "altitude should displace by {h} km, got {}", d.norm());
    let aligned = d.unit().dot(flat.up);
    assert!(
        fabs(aligned - 1.0) < 1e-12,
        "altitude must be along the ellipsoid normal, dot product {aligned}"
    );
}

/// A site built on a body whose radius is unknown is refused, using the catalogue's own type.
#[test]
fn an_unknown_radius_is_refused() {
    // synthesise a body with an unknown radius rather than mutating the catalogue
    let mut physical = EARTH.physical;
    physical.equatorial_radius = Quantity::unknown();
    let body = odderon_astro::catalogue::Body {
        id: "synthetic",
        physical,
    };
    assert_eq!(
        Site::from_geodetic(&body, 0.0, 0.0, 0.0),
        Err(SiteError::RadiusUnknown)
    );
    let _ = Lookup::<Quantity>::NoSuchBody; // keep the import meaningful
}

/// Unused-import guard: `sqrt` is used above via the tangency check.
#[test]
fn tangent_length_is_a_sqrt_of_a_difference() {
    let a = horizon_range(RE, RE + 400.0).expect("valid");
    let manual = sqrt((RE + 400.0) * (RE + 400.0) - RE * RE);
    assert!(fabs(a - manual) < 1e-9);
}
