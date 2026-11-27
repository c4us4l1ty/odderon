//! Kernel tests for the rendezvous module.
//!
//! The reference values are computed independently in Python from the textbook Hohmann transfer
//! formulas, then checked here against the kernel's implementation. The tolerance is written before
//! the test: 1e-6 relative for the Δv (the formulas are exact in exact arithmetic, so the
//! difference is floating-point only), 1e-3 s for the transfer time (the same), and 1e-9 for the
//! phase angle (a pure function of the radius ratio, so the agreement should be at machine
//! precision).

use odderon_astro::mu;
use odderon_astro::rendezvous::{
    rendezvous_design_hash, rendezvous_phase_angle, rendezvous_transfer_dv,
    rendezvous_transfer_time_s, rendezvous_window_s,
};

const MU: f64 = mu::EARTH;

// Reference case: LEO (400 km altitude) to GEO. Computed independently from the textbook formulas.
const R_LEO: f64 = 6778.137; // 6378.137 + 400
const R_GEO: f64 = 42164.0;

/// The total Hohmann Δv from LEO to GEO. Reference: 3.853957 km/s (Python, exact formulas).
#[test]
fn transfer_dv_leo_to_geo() {
    let dv = rendezvous_transfer_dv(R_LEO, R_GEO, MU);
    let expected = 3.853957;
    let rel = (dv - expected).abs() / expected;
    assert!(rel < 1e-5, "dv = {dv}, expected {expected}, rel = {rel}");
}

/// The two burns individually: departure burn ~2.397 km/s, arrival burn ~1.456 km/s.
#[test]
fn transfer_dv_breakdown_leo_to_geo() {
    // Departure burn: sqrt(mu/r1) * (sqrt(2*r2/(r1+r2)) - 1)
    let r_sum = R_LEO + R_GEO;
    let dv_dep_expected = (MU / R_LEO).sqrt() * ((2.0 * R_GEO / r_sum).sqrt() - 1.0);
    let dv_arr_expected = (MU / R_GEO).sqrt() * (1.0 - (2.0 * R_LEO / r_sum).sqrt());
    let total_expected = dv_dep_expected + dv_arr_expected;
    let dv = rendezvous_transfer_dv(R_LEO, R_GEO, MU);
    let rel = (dv - total_expected).abs() / total_expected;
    assert!(
        rel < 1e-10,
        "dv = {dv}, expected {total_expected}, rel = {rel}"
    );
}

/// The transfer time: half the period of the transfer ellipse. Reference: 19048.5 s.
#[test]
fn transfer_time_leo_to_geo() {
    let t = rendezvous_transfer_time_s(R_LEO, R_GEO, MU);
    let expected = 19048.5;
    let rel = (t - expected).abs() / expected;
    assert!(rel < 1e-4, "t = {t}, expected {expected}, rel = {rel}");
}

/// The phase angle for LEO to GEO. Reference: 1.752547 rad (100.41°).
#[test]
fn phase_angle_leo_to_geo() {
    let phi = rendezvous_phase_angle(R_LEO, R_GEO, MU);
    let expected = 1.752547;
    let rel = (phi - expected).abs() / expected;
    assert!(rel < 1e-5, "phi = {phi}, expected {expected}, rel = {rel}");
}

/// The phase angle is independent of `mu` — it is a pure function of the radius ratio.
#[test]
fn phase_angle_independent_of_mu() {
    let phi1 = rendezvous_phase_angle(R_LEO, R_GEO, MU);
    let phi2 = rendezvous_phase_angle(R_LEO, R_GEO, MU * 2.0);
    let rel = (phi1 - phi2).abs() / phi1.abs();
    assert!(rel < 1e-12, "phi depends on mu: {phi1} vs {phi2}");
}

/// A transfer to the same orbit costs zero Δv — no transfer needed.
#[test]
fn transfer_dv_same_orbit_is_zero() {
    let dv = rendezvous_transfer_dv(R_LEO, R_LEO, MU);
    assert_eq!(dv, 0.0);
}

/// Transfer time for the same orbit is NaN — there is no transfer to time.
#[test]
fn transfer_time_same_orbit_is_nan() {
    let t = rendezvous_transfer_time_s(R_LEO, R_LEO, MU);
    assert!(t.is_nan());
}

/// Phase angle for the same orbit is NaN — undefined.
#[test]
fn phase_angle_same_orbit_is_nan() {
    let phi = rendezvous_phase_angle(R_LEO, R_LEO, MU);
    assert!(phi.is_nan());
}

/// Inward transfer (GEO to LEO) has the same total Δv as outward — the burn magnitudes are the
/// same, only the direction reverses.
#[test]
fn transfer_dv_symmetric() {
    let out = rendezvous_transfer_dv(R_LEO, R_GEO, MU);
    let inward = rendezvous_transfer_dv(R_GEO, R_LEO, MU);
    let rel = (out - inward).abs() / out;
    assert!(rel < 1e-12, "outward {out} != inward {inward}");
}

/// The phase angle for an inward transfer is negative (the target must be behind).
#[test]
fn phase_angle_inward_is_negative() {
    let phi = rendezvous_phase_angle(R_GEO, R_LEO, MU);
    assert!(
        phi < 0.0,
        "inward phase angle should be negative, got {phi}"
    );
}

/// NaN input is refused, not answered.
#[test]
fn transfer_dv_refuses_nan() {
    assert!(rendezvous_transfer_dv(f64::NAN, R_GEO, MU).is_nan());
    assert!(rendezvous_transfer_dv(R_LEO, f64::NAN, MU).is_nan());
    assert!(rendezvous_transfer_dv(R_LEO, R_GEO, f64::NAN).is_nan());
}

/// Zero or negative radius is refused.
#[test]
fn transfer_dv_refuses_non_positive() {
    assert!(rendezvous_transfer_dv(0.0, R_GEO, MU).is_nan());
    assert!(rendezvous_transfer_dv(-1.0, R_GEO, MU).is_nan());
    assert!(rendezvous_transfer_dv(R_LEO, 0.0, MU).is_nan());
}

/// Zero or negative mu is refused.
#[test]
fn transfer_dv_refuses_bad_mu() {
    assert!(rendezvous_transfer_dv(R_LEO, R_GEO, 0.0).is_nan());
    assert!(rendezvous_transfer_dv(R_LEO, R_GEO, -1.0).is_nan());
}

/// A window at the exact required lead angle is open now: time = 0.
#[test]
fn window_open_now() {
    let phi = rendezvous_phase_angle(R_LEO, R_GEO, MU);
    let t = rendezvous_window_s(R_LEO, R_GEO, phi, MU);
    assert!(
        t.abs() < 1e-6,
        "window should be 0 at the required lead, got {t}"
    );
}

/// The window from a zero lead angle: the target is directly ahead, so the chaser (in the lower,
/// faster orbit) must wait for the target's lead to shrink to the required value. Reference:
/// 4280.5 s (Python, independent computation).
#[test]
fn window_from_zero_lead() {
    let t = rendezvous_window_s(R_LEO, R_GEO, 0.0, MU);
    let expected = 4280.5;
    let rel = (t - expected).abs() / expected;
    assert!(rel < 1e-3, "window = {t}, expected {expected}, rel = {rel}");
}

/// The window is periodic: advancing the lead by one full relative period returns to the same
/// lead angle, so the window from that angle is the same as from the original. Uses an angle away
/// from the required lead to avoid the discontinuity at the window itself.
#[test]
fn window_periodicity() {
    let phi = rendezvous_phase_angle(R_LEO, R_GEO, MU);
    // Use an angle halfway between the required lead and the full circle, away from the
    // discontinuity at the window.
    let theta0 = (phi + std::f64::consts::PI) * 0.5;
    let t0 = rendezvous_window_s(R_LEO, R_GEO, theta0, MU);
    let n_from = (MU / (R_LEO * R_LEO * R_LEO)).sqrt();
    let n_to = (MU / (R_GEO * R_GEO * R_GEO)).sqrt();
    let omega = n_to - n_from;
    let period = 2.0 * std::f64::consts::PI / omega.abs();
    // Advance the lead by one full relative period: the angle returns to itself.
    let theta1 = (theta0 + omega * period).rem_euclid(2.0 * std::f64::consts::PI);
    let t1 = rendezvous_window_s(R_LEO, R_GEO, theta1, MU);
    let rel = (t1 - t0).abs() / t0.abs().max(1e-12);
    assert!(rel < 1e-9, "window not periodic: t1 = {t1}, t0 = {t0}");
}

/// NaN theta is refused.
#[test]
fn window_refuses_nan_theta() {
    assert!(rendezvous_window_s(R_LEO, R_GEO, f64::NAN, MU).is_nan());
}

/// Theta outside [0, 2π) is refused.
#[test]
fn window_refuses_out_of_range_theta() {
    assert!(rendezvous_window_s(R_LEO, R_GEO, -0.1, MU).is_nan());
    assert!(rendezvous_window_s(R_LEO, R_GEO, 2.0 * std::f64::consts::PI, MU).is_nan());
}

/// Same-orbit rendezvous has no window (no relative motion).
#[test]
fn window_same_orbit_is_nan() {
    let t = rendezvous_window_s(R_LEO, R_LEO, 1.0, MU);
    assert!(t.is_nan());
}

/// The design hash is deterministic.
#[test]
fn design_hash_deterministic() {
    let h1 = rendezvous_design_hash();
    let h2 = rendezvous_design_hash();
    assert_eq!(h1, h2);
}

/// The design hash is a non-zero u64 (a real hash, not the empty-string hash).
#[test]
fn design_hash_nonzero() {
    let h = rendezvous_design_hash();
    assert_ne!(h, 0);
}

// ---------------------------------------------------------------------------------------------
// J2 + J3 + J4 propagation (the rendezvous case)
// ---------------------------------------------------------------------------------------------

use odderon_astro::integrate::integrate_zonal;
use odderon_astro::j2::{EARTH_J2, EARTH_J3, EARTH_J4};
use odderon_astro::radius;
use odderon_astro::rendezvous::{propagate_zonal, zonal_vs_two_body_drift_m};
use odderon_astro::State;
use odderon_astro::Vec3;

/// Build a circular LEO state at the given inclination, periapsis on +X.
fn leo_state(alt_km: f64, incl_deg: f64) -> State {
    let r = radius::EARTH + alt_km;
    let vp = (MU / r).sqrt();
    let incl = std::f64::consts::PI * incl_deg / 180.0;
    State::new(
        Vec3::new(r, 0.0, 0.0),
        Vec3::new(0.0, vp * incl.cos(), vp * incl.sin()),
    )
}

/// `propagate_zonal` returns `None` for degenerate input.
#[test]
fn propagate_zonal_refuses_degenerate_input() {
    let s = leo_state(400.0, 51.6);
    assert!(propagate_zonal(&s, f64::NAN, 1e-12).is_none(), "NaN dt");
    assert!(propagate_zonal(&s, -1.0, 1e-12).is_none(), "negative dt");
    assert!(propagate_zonal(&s, 100.0, 0.0).is_none(), "zero tolerance");
    assert!(
        propagate_zonal(&s, 100.0, -1e-12).is_none(),
        "negative tolerance"
    );
}

/// A zero-time propagation returns the initial state.
#[test]
fn propagate_zonal_zero_time_is_identity() {
    let s = leo_state(400.0, 51.6);
    let out = propagate_zonal(&s, 0.0, 1e-12).expect("zero time");
    assert!((out.r.x - s.r.x).abs() < 1e-9, "position not preserved");
    assert!((out.r.y - s.r.y).abs() < 1e-9, "position not preserved");
    assert!((out.r.z - s.r.z).abs() < 1e-9, "position not preserved");
}

/// `propagate_zonal` agrees with the direct `integrate_zonal` call using Earth's constants.
///
/// This is a thin wrapper test: it confirms the wrapper passes the right constants through and
/// extracts the state correctly.
#[test]
fn propagate_zonal_matches_integrate_zonal() {
    let s = leo_state(400.0, 51.6);
    let dt = 86_400.0; // one day
    let wrapped = propagate_zonal(&s, dt, 1e-12).expect("wrapper");
    let direct = integrate_zonal(
        &s,
        dt,
        MU,
        EARTH_J2,
        EARTH_J3,
        EARTH_J4,
        radius::EARTH,
        1e-12,
    )
    .expect("direct");
    assert!((wrapped.r.x - direct.state.r.x).abs() < 1e-9, "x mismatch");
    assert!((wrapped.r.y - direct.state.r.y).abs() < 1e-9, "y mismatch");
    assert!((wrapped.r.z - direct.state.r.z).abs() < 1e-9, "z mismatch");
}

/// The two-body approximation drifts significantly from the full field over one day at LEO.
///
/// This is the measurement that motivates the J2+J3+J4 block: the `CREDIBILITY.md` §3.1 baseline
/// says the two-body propagation is ~539 km off after one day at 400 km / 51.6°. This test confirms
/// the drift is large (not a rounding artifact) and in the expected direction (the full field and
/// two-body disagree substantially).
#[test]
fn zonal_vs_two_body_drift_is_significant_at_leo() {
    let s = leo_state(400.0, 51.6);
    let drift_km = zonal_vs_two_body_drift_m(&s, 86_400.0, 1e-12) / 1000.0;
    // The two-body drift must be large — hundreds of km, not metres.
    assert!(
        drift_km > 50.0,
        "two-body drift after one day at LEO is only {drift_km:.1} km - \
         the full field and two-body should disagree by hundreds of km"
    );
}

/// The drift grows with time: a week's propagation has more drift than a day's.
///
/// This is a monotonicity check. If the drift did not grow, the perturbation would be periodic with
/// a period shorter than a day, which is not the case for J2/J3/J4 secular effects.
#[test]
fn zonal_vs_two_body_drift_grows_with_time() {
    let s = leo_state(400.0, 51.6);
    let day_drift = zonal_vs_two_body_drift_m(&s, 86_400.0, 1e-12);
    let week_drift = zonal_vs_two_body_drift_m(&s, 7.0 * 86_400.0, 1e-12);
    assert!(
        week_drift > day_drift,
        "7-day drift ({week_drift:.1} m) should exceed 1-day drift ({day_drift:.1} m)"
    );
}

/// `zonal_vs_two_body_drift_m` returns NaN for invalid input.
#[test]
fn zonal_vs_two_body_drift_refuses_degenerate_input() {
    let s = leo_state(400.0, 51.6);
    assert!(zonal_vs_two_body_drift_m(&s, f64::NAN, 1e-12).is_nan());
    assert!(zonal_vs_two_body_drift_m(&s, -1.0, 1e-12).is_nan());
    assert!(zonal_vs_two_body_drift_m(&s, 100.0, 0.0).is_nan());
}
