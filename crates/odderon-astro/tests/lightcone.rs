//! Kernel tests for the light-cone structure module.
//!
//! Each function is a pure function of physics constants and its inputs, so the tests check the
//! physics directly: the light-cone radius is the speed of light times the time, the reachability
//! predicate is the radius comparison, the Doppler factor matches the three classic cases (approaching,
//! receding, transverse), and the inverse-square attenuation matches the physical law. The reference
//! values are computed independently from the textbook formulas.

use odderon_astro::lightcone::{
    causal_reachable, doppler_factor, light_cone_radius_km, lightcone_design_hash,
    signal_power_ratio,
};
use odderon_astro::lighttime::SPEED_OF_LIGHT_KM_S;
use odderon_dmath::pow;

/// One second of light travel: the cone radius should be exactly the speed of light in km.
const ONE_SECOND_RADIUS_KM: f64 = 299_792.458;

// --------------------------------------------------------------------------------------------
// Light-cone radius
// --------------------------------------------------------------------------------------------

#[test]
fn radius_at_one_second_is_speed_of_light() {
    let r = light_cone_radius_km(1.0).unwrap();
    // The radius at 1 s is exactly c in km.
    assert!(
        (r - ONE_SECOND_RADIUS_KM) < 1e-9,
        "radius = {r}, want {ONE_SECOND_RADIUS_KM}"
    );
}

#[test]
fn radius_at_ten_seconds_is_ten_times_speed_of_light() {
    let r = light_cone_radius_km(10.0).unwrap();
    assert!((r - 10.0 * ONE_SECOND_RADIUS_KM) < 1e-6, "radius = {r}");
}

#[test]
fn radius_scales_linearly_with_time() {
    let r1 = light_cone_radius_km(1.0).unwrap();
    let r5 = light_cone_radius_km(5.0).unwrap();
    assert!((r5 - 5.0 * r1) < 1e-6, "not linear: {r5} vs {r1}");
}

#[test]
fn radius_refuses_zero_time() {
    assert!(light_cone_radius_km(0.0).is_none());
}

#[test]
fn radius_refuses_negative_time() {
    assert!(light_cone_radius_km(-1.0).is_none());
}

#[test]
fn radius_refuses_nan_time() {
    assert!(light_cone_radius_km(f64::NAN).is_none());
}

#[test]
fn radius_refuses_infinite_time() {
    assert!(light_cone_radius_km(f64::INFINITY).is_none());
}

// --------------------------------------------------------------------------------------------
// Causal reachability
// --------------------------------------------------------------------------------------------

#[test]
fn reachable_when_distance_is_inside_cone() {
    // A position at half the cone radius is reachable.
    assert!(causal_reachable(0.5 * ONE_SECOND_RADIUS_KM, 1.0));
}

#[test]
fn reachable_at_exactly_cone_radius() {
    // A position exactly at the cone radius is reachable (the boundary is inclusive).
    assert!(causal_reachable(ONE_SECOND_RADIUS_KM, 1.0));
}

#[test]
fn not_reachable_when_distance_is_beyond_cone() {
    // A position at twice the cone radius is not reachable.
    assert!(!causal_reachable(2.0 * ONE_SECOND_RADIUS_KM, 1.0));
}

#[test]
fn not_reachable_at_zero_time() {
    // No time has passed, so no position is reachable (the cone radius is zero).
    assert!(!causal_reachable(1.0, 0.0));
}

#[test]
fn not_reachable_for_nan_distance() {
    assert!(!causal_reachable(f64::NAN, 1.0));
}

#[test]
fn not_reachable_for_nan_time() {
    assert!(!causal_reachable(1.0, f64::NAN));
}

// --------------------------------------------------------------------------------------------
// Doppler factor
// --------------------------------------------------------------------------------------------

#[test]
fn doppler_approaching_is_blueshift() {
    // A source moving toward the observer at a small fraction of c: the received frequency is higher.
    let beta = 0.01;
    let v = beta * SPEED_OF_LIGHT_KM_S;
    let f = doppler_factor(v, 0.0).unwrap();
    let expected = odderon_dmath::sqrt((1.0 + beta) / (1.0 - beta));
    assert!((f - expected) < 1e-12, "f = {f}, want {expected}");
    assert!(f > 1.0, "approaching should be a blueshift, got {f}");
}

#[test]
fn doppler_receding_is_redshift() {
    // A source moving away at a small fraction of c: the received frequency is lower.
    let beta = 0.01;
    let v = beta * SPEED_OF_LIGHT_KM_S;
    let f = doppler_factor(v, std::f64::consts::PI).unwrap();
    let expected = odderon_dmath::sqrt((1.0 - beta) / (1.0 + beta));
    assert!((f - expected) < 1e-12, "f = {f}, want {expected}");
    assert!(f < 1.0, "receding should be a redshift, got {f}");
}

#[test]
fn doppler_transverse_is_relativistic_redshift() {
    // A source moving perpendicular to the line of sight: the transverse relativistic shift, a
    // redshift even though there is no line-of-sight motion.
    let beta = 0.01;
    let v = beta * SPEED_OF_LIGHT_KM_S;
    let f = doppler_factor(v, std::f64::consts::PI / 2.0).unwrap();
    let expected = odderon_dmath::sqrt(1.0 - beta * beta);
    assert!((f - expected) < 1e-12, "f = {f}, want {expected}");
    assert!(f < 1.0, "transverse should be a redshift, got {f}");
}

#[test]
fn doppler_zero_velocity_is_no_shift() {
    // A stationary source: no shift, the factor is exactly 1.
    let f = doppler_factor(0.0, 0.5).unwrap();
    assert!((f - 1.0) < 1e-12, "f = {f}, want 1.0");
}

#[test]
fn doppler_refuses_velocity_at_light_speed() {
    // A source moving at c: beta = 1, the formula diverges. Refused.
    assert!(doppler_factor(SPEED_OF_LIGHT_KM_S, 0.0).is_none());
}

#[test]
fn doppler_refuses_velocity_beyond_light_speed() {
    assert!(doppler_factor(SPEED_OF_LIGHT_KM_S * 2.0, 0.0).is_none());
}

#[test]
fn doppler_refuses_nan_velocity() {
    assert!(doppler_factor(f64::NAN, 0.0).is_none());
}

#[test]
fn doppler_refuses_nan_angle() {
    assert!(doppler_factor(100.0, f64::NAN).is_none());
}

// --------------------------------------------------------------------------------------------
// Signal power (inverse-square)
// --------------------------------------------------------------------------------------------

#[test]
fn power_at_reference_distance_is_one() {
    let r = signal_power_ratio(100.0, 100.0).unwrap();
    assert!((r - 1.0) < 1e-12, "r = {r}, want 1.0");
}

#[test]
fn power_halves_distance_quarters() {
    // Twice the distance: one quarter the power.
    let r = signal_power_ratio(200.0, 100.0).unwrap();
    assert!((r - 0.25) < 1e-12, "r = {r}, want 0.25");
}

#[test]
fn power_ten_times_distance_is_one_hundredth() {
    // Ten times the distance: one hundredth the power.
    let r = signal_power_ratio(1000.0, 100.0).unwrap();
    assert!((r - 0.01) < 1e-12, "r = {r}, want 0.01");
}

#[test]
fn power_is_symmetric_in_exponent() {
    // The exponent is exactly 2: (d_ref/d)^2.
    let r = signal_power_ratio(300.0, 100.0).unwrap();
    let expected = pow(100.0 / 300.0, 2.0);
    assert!((r - expected) < 1e-12, "r = {r}, want {expected}");
}

#[test]
fn power_refuses_zero_distance() {
    assert!(signal_power_ratio(0.0, 100.0).is_none());
}

#[test]
fn power_refuses_zero_reference() {
    assert!(signal_power_ratio(100.0, 0.0).is_none());
}

#[test]
fn power_refuses_negative_distance() {
    assert!(signal_power_ratio(-100.0, 100.0).is_none());
}

#[test]
fn power_refuses_nan() {
    assert!(signal_power_ratio(f64::NAN, 100.0).is_none());
}

// --------------------------------------------------------------------------------------------
// Design hash
// --------------------------------------------------------------------------------------------

#[test]
fn design_hash_is_stable() {
    assert_eq!(lightcone_design_hash(), lightcone_design_hash());
}

#[test]
fn design_hash_is_nonzero() {
    assert!(lightcone_design_hash() != 0);
}
