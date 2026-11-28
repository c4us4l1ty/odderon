//! Ecliptic frame tests: the ICRF ↔ ecliptic transformation.
//!
//! `CREDIBILITY.md` §3.1 row "Frame transformations" is listed as "partially implemented" — the
//! body-fixed leg is done, the ecliptic leg was not. This test file covers the ecliptic leg.
//!
//! The checks are not "does it run." They are the mathematical properties a rotation must satisfy,
//! plus the physical meaning of the obliquity constant:
//!
//! * Round trip: ICRF → ecliptic → ICRF returns the input (a rotation and its inverse compose to
//!   the identity).
//! * Magnitude preservation: a rotation does not change vector lengths.
//! * Angle preservation: the angle between two vectors is the same in both frames.
//! * Pole mapping: the ecliptic north pole maps to the celestial pole, tilted by exactly ε.
//! * Axis mapping: the vernal equinox is the x-axis in both frames, so it maps to itself.
//! * Published constant: the obliquity at J2000 matches the IAU 2006 value, checked against the
//!   published number rather than the code's own arithmetic.
//! * Secular drift: ε decreases over time, at the published rate.

use odderon_astro::ecliptic::*;
use odderon_astro::Vec3;
use odderon_dmath::{fabs, PI};

// ---------------------------------------------------------------------------------------------
// The published constant
// ---------------------------------------------------------------------------------------------

/// The published IAU 2006 mean obliquity at J2000, in degrees.
///
/// From the IAU 2006 precession model (Capitaine et al. 2003), as adopted by IAU Resolution B1
/// (2006). This is the value the transformation is built from, and the test pins the code's
/// constant against this published number rather than against the code's own arithmetic.
const PUBLISHED_OBLIQUITY_J2000_DEG: f64 = 23.439_291_0;

/// Tolerance for comparing the code's obliquity constant against the published value.
///
/// The published value has 7 significant figures; the code carries the same. A relative tolerance
/// of 1e-9 is ample to catch a transcription error while not being tighter than the published
/// precision.
const OBLIQUITY_CONST_TOL: f64 = 1e-9;

#[test]
fn obliquity_constant_matches_published_value() {
    let code_deg = OBLIQUITY_J2000_RAD * 180.0 / PI;
    let rel_err = fabs(code_deg - PUBLISHED_OBLIQUITY_J2000_DEG) / PUBLISHED_OBLIQUITY_J2000_DEG;
    assert!(
        rel_err < OBLIQUITY_CONST_TOL,
        "obliquity constant {code_deg}° vs published {PUBLISHED_OBLIQUITY_J2000_DEG}°, \
         relative error {rel_err}"
    );
}

#[test]
fn obliquity_rate_is_negative_and_small() {
    // The obliquity decreases over time. The rate is ~−47″/century ≈ −0.0131°/century,
    // which is ~5.6e-4 of the total angle per century — small, but the right sign.
    let rate = OBLIQUITY_RATE_RAD_PER_S;
    assert!(
        rate < 0.0,
        "obliquity must decrease over time, got rate {rate}"
    );
    // The magnitude is tiny: over a century it changes by ~47 arcseconds ≈ 0.0131°.
    let change_per_century = OBLIQUITY_RATE_RAD_PER_S * 100.0 * 365.25 * 86400.0;
    let rel_change = fabs(change_per_century) / OBLIQUITY_J2000_RAD;
    assert!(
        rel_change < 1e-3,
        "obliquity change per century is {rel_change:.2e}, should be < 1e-3"
    );
}

// ---------------------------------------------------------------------------------------------
// Round trip
// ---------------------------------------------------------------------------------------------

#[test]
fn round_trip_closes_for_various_vectors() {
    let ec = Ecliptic::standard();
    let vectors: [Vec3; 5] = [
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(0.0, 1.0, 0.0),
        Vec3::new(0.0, 0.0, 1.0),
        Vec3::new(1.0, 2.0, 3.0),
        Vec3::new(-4.5, 2.3, -1.7),
    ];
    for &v in vectors.iter() {
        let result = ec.round_trip(v, 0.0).expect("round trip should succeed");
        let err = (result - v).norm() / v.norm();
        assert!(
            err < 1e-14,
            "round trip of {v:?}: relative error {err}"
        );
    }
}

#[test]
fn round_trip_closes_at_future_epochs() {
    // The secular drift is tiny but nonzero; the round trip must still close at any epoch.
    let ec = Ecliptic::standard();
    let v = Vec3::new(3.0, -1.5, 2.0);
    for dt in [0.0, 86400.0, 86400.0 * 365.25, 86400.0 * 365.25 * 100.0] {
        let result = ec.round_trip(v, dt).expect("round trip should succeed");
        let err = (result - v).norm() / v.norm();
        assert!(
            err < 1e-14,
            "round trip at dt={dt}: relative error {err}"
        );
    }
}

#[test]
fn round_trip_refuses_non_finite_dt() {
    let ec = Ecliptic::standard();
    let v = Vec3::new(1.0, 0.0, 0.0);
    assert!(ec.round_trip(v, f64::NAN).is_none());
    assert!(ec.round_trip(v, f64::INFINITY).is_none());
}

// ---------------------------------------------------------------------------------------------
// Magnitude preservation
// ---------------------------------------------------------------------------------------------

#[test]
fn rotation_preserves_vector_magnitudes() {
    let ec = Ecliptic::standard();
    let vectors: [Vec3; 5] = [
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(0.0, 1.0, 0.0),
        Vec3::new(0.0, 0.0, 1.0),
        Vec3::new(100.0, -50.0, 25.0),
        Vec3::new(-1.0, 2.0, -3.0),
    ];
    for &v in vectors.iter() {
        let original = v.norm();
        let rotated = ec.to_ecliptic(v, 0.0).expect("transform should succeed");
        let rotated_norm = rotated.norm();
        let rel_err = fabs(rotated_norm - original) / original;
        assert!(
            rel_err < 1e-15,
            "magnitude of {v:?}: original {original}, rotated {rotated_norm}, rel err {rel_err}"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Angle preservation
// ---------------------------------------------------------------------------------------------

#[test]
fn rotation_preserves_angles_between_vectors() {
    let ec = Ecliptic::standard();
    let pairs: [(Vec3, Vec3); 4] = [
        (Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0)),
        (Vec3::new(1.0, 2.0, 3.0), Vec3::new(-2.0, 1.0, 0.5)),
        (Vec3::new(0.0, 0.0, 1.0), Vec3::new(1.0, 0.0, 0.0)),
        (Vec3::new(5.0, -3.0, 1.0), Vec3::new(0.0, 0.0, 2.0)),
    ];
    for &(a, b) in pairs.iter() {
        let angle_icrf = angle_between(a, b).expect("angle should be computable");
        let a_ecl = ec.to_ecliptic(a, 0.0).expect("transform should succeed");
        let b_ecl = ec.to_ecliptic(b, 0.0).expect("transform should succeed");
        let angle_ecl = angle_between(a_ecl, b_ecl).expect("angle should be computable");
        let diff = fabs(angle_icrf - angle_ecl);
        assert!(
            diff < 1e-13,
            "angle between {a:?} and {b:?}: ICRF {angle_icrf}, ecliptic {angle_ecl}, diff {diff}"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Physical meaning: pole and axis mapping
// ---------------------------------------------------------------------------------------------

#[test]
fn ecliptic_north_pole_maps_to_celestial_pole() {
    // The ecliptic north pole (0, 0, 1) in the ecliptic frame maps to (0, -sin ε, cos ε) in ICRF.
    // This is the celestial pole, tilted from the ecliptic pole by exactly the obliquity.
    let ec = Ecliptic::standard();
    let pole_ecl = Vec3::new(0.0, 0.0, 1.0);
    let pole_icrf = ec.to_icrf(pole_ecl, 0.0).expect("transform should succeed");

    let eps = OBLIQUITY_J2000_RAD;
    let expected = Vec3::new(0.0, -odderon_astro::sin_rad(eps), odderon_astro::cos_rad(eps));

    let err = (pole_icrf - expected).norm();
    assert!(
        err < 1e-14,
        "ecliptic north pole: got {pole_icrf:?}, expected {expected:?}, err {err}"
    );

    // The angle between the two poles must be exactly the obliquity.
    let angle = angle_between(pole_ecl, pole_icrf).expect("angle should be computable");
    let angle_err = fabs(angle - eps);
    assert!(
        angle_err < 1e-13,
        "angle between poles: got {angle}, expected {eps}, err {angle_err}"
    );
}

#[test]
fn vernal_equinox_maps_to_itself() {
    // The vernal equinox (1, 0, 0) is the x-axis in both frames, so it maps to itself.
    let ec = Ecliptic::standard();
    let equinox = vernal_equinox();
    let mapped = ec.to_ecliptic(equinox, 0.0).expect("transform should succeed");
    let err = (mapped - equinox).norm();
    assert!(
        err < 1e-15,
        "vernal equinox: got {mapped:?}, expected {equinox:?}, err {err}"
    );

    // And the inverse does too.
    let mapped_back = ec.to_icrf(equinox, 0.0).expect("transform should succeed");
    let err_back = (mapped_back - equinox).norm();
    assert!(
        err_back < 1e-15,
        "vernal equinox (inverse): got {mapped_back:?}, expected {equinox:?}, err {err_back}"
    );
}

#[test]
fn ecliptic_north_pole_icrf_function_matches_transform() {
    // The free function and the transform must agree.
    let ec = Ecliptic::standard();
    let pole = Vec3::new(0.0, 0.0, 1.0);
    let via_transform = ec.to_icrf(pole, 0.0).expect("transform should succeed");
    let via_function = ecliptic_north_pole_icrf(0.0).expect("function should succeed");
    let err = (via_transform - via_function).norm();
    assert!(err < 1e-14, "pole mismatch: {via_transform:?} vs {via_function:?}, err {err}");
}

// ---------------------------------------------------------------------------------------------
// Secular drift
// ---------------------------------------------------------------------------------------------

#[test]
fn obliquity_decreases_with_time() {
    let ec = Ecliptic::standard();
    let eps_0 = ec.obliquity_at(0.0).expect("obliquity at J2000");
    // One century later.
    let century_s = 100.0 * 365.25 * 86400.0;
    let eps_century = ec.obliquity_at(century_s).expect("obliquity at century");
    assert!(
        eps_century < eps_0,
        "obliquity should decrease: J2000 {eps_0}, century later {eps_century}"
    );
    // The change is ~47 arcseconds ≈ 0.0131 degrees.
    let change_deg = (eps_0 - eps_century) * 180.0 / PI;
    assert!(
        change_deg > 0.013 && change_deg < 0.014,
        "obliquity change per century: {change_deg}°, expected ~0.0131°"
    );
}

#[test]
fn obliquity_at_j2000_is_the_constant() {
    let ec = Ecliptic::standard();
    let eps = ec.obliquity_at(0.0).expect("obliquity at J2000");
    let err = fabs(eps - OBLIQUITY_J2000_RAD);
    assert!(
        err < 1e-15,
        "obliquity at J2000: got {eps}, expected {OBLIQUITY_J2000_RAD}"
    );
}

// ---------------------------------------------------------------------------------------------
// Degenerate input
// ---------------------------------------------------------------------------------------------

#[test]
fn transforms_refuse_non_finite_dt() {
    let ec = Ecliptic::standard();
    let v = Vec3::new(1.0, 0.0, 0.0);
    assert!(ec.to_ecliptic(v, f64::NAN).is_none());
    assert!(ec.to_ecliptic(v, f64::INFINITY).is_none());
    assert!(ec.to_icrf(v, f64::NAN).is_none());
    assert!(ec.to_icrf(v, f64::INFINITY).is_none());
}

#[test]
fn obliquity_at_refuses_non_finite_dt() {
    let ec = Ecliptic::standard();
    assert!(ec.obliquity_at(f64::NAN).is_none());
    assert!(ec.obliquity_at(f64::INFINITY).is_none());
}

// ---------------------------------------------------------------------------------------------
// Physical sanity: the ecliptic plane
// ---------------------------------------------------------------------------------------------

#[test]
fn ecliptic_plane_contains_the_equinox_and_is_tilted_from_icrf_equator() {
    // The ecliptic plane contains the vernal equinox (x-axis) and is tilted from the ICRF equatorial
    // plane by the obliquity. A vector in the ecliptic plane has z_ecl = 0. A vector in the ICRF
    // equatorial plane has z_icrf = 0.
    let ec = Ecliptic::standard();

    // The y-axis in the ecliptic frame is in the ecliptic plane.
    let y_ecl = Vec3::new(0.0, 1.0, 0.0);
    let y_icrf = ec.to_icrf(y_ecl, 0.0).expect("transform should succeed");
    // Its z-component in ICRF should be sin(ε) — the tilt.
    let expected_z = odderon_astro::sin_rad(OBLIQUITY_J2000_RAD);
    let z_err = fabs(y_icrf.z - expected_z);
    let got_z = y_icrf.z;
    assert!(
        z_err < 1e-14,
        "y-axis z-component: got {got_z:?}, expected {expected_z:?}, err {z_err}"
    );

    // The ICRF equatorial plane (z=0) maps to a plane tilted by ε from the ecliptic.
    // The ICRF north pole (0,0,1) has an ecliptic z-component of cos(ε).
    let icrf_north = Vec3::new(0.0, 0.0, 1.0);
    let north_ecl = ec
        .to_ecliptic(icrf_north, 0.0)
        .expect("transform should succeed");
    let expected_ecl_z = odderon_astro::cos_rad(OBLIQUITY_J2000_RAD);
    let z_err = fabs(north_ecl.z - expected_ecl_z);
    let got_ecl_z = north_ecl.z;
    assert!(
        z_err < 1e-14,
        "ICRF north pole ecliptic z: got {got_ecl_z:?}, expected {expected_ecl_z:?}, err {z_err}"
    );
}
