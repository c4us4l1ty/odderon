//! Parity tests against the TypeScript reference implementation.
//!
//! The golden numbers below were sampled from the running game
//! (`window.odderon.vessel`) and solved by `src/core/orbit.ts`. They are the
//! contract between the two implementations: if these drift, the wasm core and
//! the fallback disagree about where the ship is.

use odderon_core::dynamics;
use odderon_core::elements::{self, Vec3};
use odderon_core::propagate;
use odderon_core::score;

const MU: f64 = odderon_core::EARTH_MU;
const R0: Vec3 = Vec3::new(
    -6_575_229.422_777_696,
    -3_346_948.151_655_526_4,
    1_038_359.366_558_675_1,
);
const V0: Vec3 = Vec3::new(
    2_182.750_070_866_793_5,
    -5_958.942_847_568_644_5,
    -3_197.413_134_061_305,
);

fn rel(actual: f64, expected: f64) -> f64 {
    ((actual - expected) / expected).abs()
}

#[test]
fn elements_match_typescript() {
    let el = elements::elements_from_state(R0, V0, MU);
    assert!(rel(el.a, 7_054_999.999_999_992) < 1e-12, "a = {}", el.a);
    assert!(rel(el.e, 0.070_588_235_294_118_31) < 1e-10, "e = {}", el.e);
    assert!((el.i - 0.497_418_836_818_383_9).abs() < 1e-12, "i = {}", el.i);
    assert!((el.raan - 0.733_038_285_837_618_4).abs() < 1e-12, "raan = {}", el.raan);
    assert!((el.argp - 5.969_026_041_820_617).abs() < 1e-10, "argp = {}", el.argp);
    assert!((el.nu - 2.531_045_009_370_307).abs() < 1e-10, "nu = {}", el.nu);
    assert!(rel(el.pe, 6_556_999.999_999_986) < 1e-12, "pe = {}", el.pe);
    assert!(rel(el.ap, 7_552_999.999_999_994) < 1e-12, "ap = {}", el.ap);
    assert!(rel(el.period, 5_897.344_626_441_641) < 1e-12, "period = {}", el.period);
}

#[test]
fn state_round_trip() {
    let el = elements::elements_from_state(R0, V0, MU);
    let (r, v) = elements::state_from_elements(el.p_hat, el.q_hat, el.p, el.e, el.nu, MU);
    assert!(
        (r.x - R0.x).abs() < 1e-6 && (r.y - R0.y).abs() < 1e-6 && (r.z - R0.z).abs() < 1e-6,
        "r = {r:?}"
    );
    assert!(
        (v.x - V0.x).abs() < 1e-6 && (v.y - V0.y).abs() < 1e-6 && (v.z - V0.z).abs() < 1e-6,
        "v = {v:?}"
    );
}

#[test]
fn kepler_returns_after_one_period() {
    let el = elements::elements_from_state(R0, V0, MU);
    let (r, v) = propagate::propagate_kepler(R0, V0, el.period, MU);
    assert!((r.x - R0.x).abs() < 0.5, "x drift {}", r.x - R0.x);
    assert!((r.y - R0.y).abs() < 0.5, "y drift {}", r.y - R0.y);
    assert!((r.z - R0.z).abs() < 0.5, "z drift {}", r.z - R0.z);
    assert!((v.x - V0.x).abs() < 1e-3, "vx drift {}", v.x - V0.x);
}

#[test]
fn kepler_conserves_energy_and_angular_momentum() {
    let e0 = dynamics::specific_energy(R0, V0, MU);
    let h0 = R0.cross(V0).len();
    let mut r = R0;
    let mut v = V0;
    for _ in 0..24 {
        let (nr, nv) = propagate::propagate_kepler(r, v, 240.0, MU);
        r = nr;
        v = nv;
        let e = dynamics::specific_energy(r, v, MU);
        assert!((e - e0).abs() / e0.abs() < 1e-9, "energy drift {}", e - e0);
        assert!((r.cross(v).len() - h0).abs() / h0 < 1e-9);
    }
}

#[test]
fn kepler_is_reversible() {
    let (r1, v1) = propagate::propagate_kepler(R0, V0, 900.0, MU);
    let (r2, v2) = propagate::propagate_kepler(r1, v1, -900.0, MU);
    assert!((r2.x - R0.x).abs() < 0.5, "x {}", r2.x - R0.x);
    assert!((r2.y - R0.y).abs() < 0.5, "y {}", r2.y - R0.y);
    assert!((v2.z - V0.z).abs() < 1e-2, "vz {}", v2.z - V0.z);
}

#[test]
fn rk4_tracks_the_conic_without_thrust() {
    // Zero thrust and no J2 must reproduce the analytical conic closely, and
    // J2 must stay a small perturbation over a single orbit.
    let mut r = R0;
    let mut v = V0;
    for _ in 0..6 {
        let (nr, nv) = dynamics::step_rk4(r, v, 5.0, Vec3::new(0.0, 0.0, 0.0), false, MU);
        r = nr;
        v = nv;
    }
    let (rk, _) = (r, v);
    let (kp, _) = propagate::propagate_kepler(R0, V0, 30.0, MU);
    let err = (rk.x - kp.x).abs() + (rk.y - kp.y).abs() + (rk.z - kp.z).abs();
    assert!(err < 1.0, "RK4 vs Kepler after 30 s: {} m", err);
}

#[test]
fn rk4_energy_drift_is_bounded() {
    let e0 = dynamics::specific_energy(R0, V0, MU);
    let mut r = R0;
    let mut v = V0;
    for _ in 0..400 {
        let (nr, nv) = dynamics::step_rk4(r, v, 0.5, Vec3::new(0.0, 0.0, 0.0), false, MU);
        r = nr;
        v = nv;
    }
    let e = dynamics::specific_energy(r, v, MU);
    assert!((e - e0).abs() / e0.abs() < 1e-9, "energy drift {}", e - e0);
}

#[test]
fn time_to_apoapsis_is_sane() {
    let el = elements::elements_from_state(R0, V0, MU);
    let t = elements::time_to_apsis(el.a, el.e, el.nu, true, MU);
    assert!(t > 0.0 && t < el.period, "t = {}", t);
    // 145 degrees of true anomaly before apoapsis is roughly a third of the
    // orbit for this eccentricity.
    assert!(t > 500.0 && t < 700.0, "t = {}", t);
    let at_pe = elements::time_to_apsis(el.a, el.e, 0.0, false, MU);
    assert!((at_pe - el.period).abs() < 1e-6);
}

#[test]
fn scoring_matches_the_type_script_contract() {
    // Perfect flight: both apsides on target, full tank, instant arrival.
    let perfect = score::composite(0.0, 0.0, 45_000.0, 1.0, 1.0);
    assert!((perfect - 1.0).abs() < 1e-12);
    // The Kestrel-9 outcome from the reference run: -18.8 km / -2.3 km.
    let flown = score::composite(-18_800.0, -2_300.0, 45_000.0, 0.874, 0.99);
    assert!((flown - 0.884).abs() < 0.02, "composite {}", flown);
    assert_eq!(score::grade(0.884 * 1000.0), "A");
    assert_eq!(score::grade(0.4 * 1000.0), "D");
    assert!(score::in_tolerance(-18_800.0, -2_300.0, 0.01, 45_000.0));
    assert!(!score::in_tolerance(-60_000.0, 0.0, 0.01, 45_000.0));
}

#[test]
fn self_test_export_answers() {
    assert_eq!(odderon_core::oc_self_test(), 1);
    assert_eq!(odderon_core::oc_version(), 2);
}
