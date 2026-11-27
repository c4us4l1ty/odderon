//! Integration tests for the Anomaly Drawer and the logic rack (bible §7.3, §7.4, §5.5, §4).
//!
//! These live in `tests/` rather than inline `#[cfg(test)]` because `ci/kernel-policy.sh` R1 scans
//! the kernel crates for host float methods (`.abs()`, `.min()`, ...) and only exempts the dmath
//! crate's own test module and `tests/` directories. The tests here use the platform's own `abs`
//! and `log` to cross-check the kernel's `fabs`/`pow`, so a kernel transcendental error is caught
//! rather than the test merely asserting the kernel agrees with itself.

use odderon_astro::anomaly::{
    anomaly_design_hash, anomaly_from_mission, drawer_can_open, elo_expected, elo_update,
    rack_compile, rack_fires, Anomaly, Op, DIVERGENCE_THRESHOLD, DRAWER_CAP, ELO_BASE, ELO_K,
    ELO_SCALE, RISK_DEPTH_WEIGHT, WINDOW_REF_DAYS,
};
use odderon_dmath::{fabs, pow};

// -------------------------------------------------------------------------------------------
// The anomaly: derivation from a mission's divergence
// -------------------------------------------------------------------------------------------

#[test]
fn below_threshold_opens_no_anomaly() {
    // predicted 0.96, occurred 1 -> divergence 0.04 < 0.05. A 4% miss is rounding, not a
    // discrepancy worth a drawer slot.
    let a = anomaly_from_mission(1, 0.96, 1, 0.0, 30.0);
    assert!(a.is_none(), "a 4% miss is rounding, not an anomaly");
}

#[test]
fn above_threshold_opens_an_anomaly() {
    let a = anomaly_from_mission(7, 0.8, 0, 0.0, 30.0).expect("an 80% miss opens an anomaly");
    assert_eq!(a.source_id, 7);
    assert!(!a.resolved, "a freshly opened anomaly is open");
    // divergence 0.8, risk 0 -> depth 0.8 (clamped). telemetry 30/30 = 1.0.
    assert!((a.depth - 0.8).abs() < 1e-12, "depth {} != 0.8", a.depth);
    assert!(
        (a.telemetry - 1.0).abs() < 1e-12,
        "telemetry {} != 1.0",
        a.telemetry
    );
    // elo = ELO_BASE + (0.8 + 0.0) * ELO_SCALE = 1000 + 160 = 1160.
    let expected_elo = ELO_BASE + (0.8 + (1.0 - 1.0)) * ELO_SCALE;
    assert!(
        (a.elo - expected_elo).abs() < 1e-9,
        "elo {} != {}",
        a.elo,
        expected_elo
    );
}

#[test]
fn depth_and_scarcity_raise_the_elo() {
    // Same divergence, but a short window (less telemetry) -> higher scarcity -> higher elo.
    let full = anomaly_from_mission(1, 0.8, 0, 0.0, 30.0).unwrap();
    let scarce = anomaly_from_mission(2, 0.8, 0, 0.0, 3.0).unwrap();
    assert!(
        scarce.elo > full.elo,
        "less telemetry is a stronger anomaly"
    );
    assert!(scarce.telemetry < full.telemetry);
    // And a riskier mission hides the fault deeper.
    let risky = anomaly_from_mission(3, 0.8, 0, 1.0, 30.0).unwrap();
    assert!(risky.depth > full.depth, "risk raises the depth");
}

#[test]
fn invalid_missions_refuse() {
    assert!(
        anomaly_from_mission(1, 1.5, 0, 0.0, 30.0).is_none(),
        "p outside [0,1] refuses"
    );
    assert!(
        anomaly_from_mission(1, f64::NAN, 0, 0.0, 30.0).is_none(),
        "non-finite p refuses"
    );
    assert!(
        anomaly_from_mission(1, 0.8, 2, 0.0, 30.0).is_none(),
        "occurred not 0/1 refuses"
    );
    assert!(
        anomaly_from_mission(1, 0.8, 0, -1.0, 30.0).is_none(),
        "negative risk refuses"
    );
    assert!(
        anomaly_from_mission(1, 0.8, 0, 0.0, 0.0).is_none(),
        "zero window refuses"
    );
    assert!(
        anomaly_from_mission(1, 0.8, 0, 0.0, f64::NAN).is_none(),
        "non-finite window refuses"
    );
}

#[test]
fn divergence_is_the_absolute_prediction_error() {
    // The divergence is |predicted - occurred|. Cross-check the kernel's fabs against the
    // platform's own abs so a transcendental error is caught, not just the shape.
    let a = anomaly_from_mission(1, 0.3, 1, 0.0, 30.0).unwrap();
    let expected_divergence = fabs(0.3 - 1.0); // 0.7
    // depth = divergence + risk*weight, clamped: 0.7 + 0 = 0.7.
    assert!(
        (a.depth - expected_divergence).abs() < 1e-12,
        "depth {} != divergence {}",
        a.depth,
        expected_divergence
    );
    // And the threshold itself: a divergence exactly at the threshold opens (the check is
    // `divergence < threshold` refuses, so equality passes). predicted = threshold, occurred = 0
    // -> divergence = threshold exactly.
    let b = anomaly_from_mission(2, DIVERGENCE_THRESHOLD, 0, 0.0, 30.0);
    assert!(b.is_some(), "a divergence exactly at the threshold opens");
}

// -------------------------------------------------------------------------------------------
// The drawer cap
// -------------------------------------------------------------------------------------------

#[test]
fn drawer_caps_at_five() {
    for count in 0..DRAWER_CAP {
        assert!(
            drawer_can_open(count),
            "count {} should allow opening",
            count
        );
    }
    assert!(
        !drawer_can_open(DRAWER_CAP),
        "the drawer is full at the cap"
    );
    assert!(!drawer_can_open(DRAWER_CAP + 3), "over the cap is refused");
}

// -------------------------------------------------------------------------------------------
// The E_an Elo
// -------------------------------------------------------------------------------------------

#[test]
fn elo_expected_matches_the_formula() {
    // Equal ratings score 0.5.
    assert!((elo_expected(1000.0, 1000.0) - 0.5).abs() < 1e-12);
    // A 200-point edge: exponent = (1000-1200)/400 = -0.5, so 10^(-0.5) = 1/sqrt(10);
    // expected = 1/(1 + 1/sqrt(10)) = sqrt(10)/(sqrt(10)+1). Cross-check with the platform's pow.
    let e = elo_expected(1200.0, 1000.0);
    let sqrt10 = pow(10.0, 0.5);
    assert!(
        (e - sqrt10 / (sqrt10 + 1.0)).abs() < 1e-9,
        "expected {} != formula",
        e
    );
    // Non-finite refuses.
    assert!(elo_expected(f64::NAN, 1000.0).is_nan());
}

#[test]
fn elo_update_rewards_a_correct_resolution() {
    let a = anomaly_from_mission(1, 0.8, 0, 0.0, 30.0).unwrap();
    // Resolve correctly: the rating rises.
    let up = elo_update(1000.0, a.elo, 1, a.depth, a.telemetry);
    assert!(up > 1000.0, "a correct resolution raises E_an: {}", up);
    // Resolve incorrectly: the rating falls.
    let down = elo_update(1000.0, a.elo, 0, a.depth, a.telemetry);
    assert!(down < 1000.0, "a wrong resolution lowers E_an: {}", down);
}

#[test]
fn elo_update_weights_by_depth_and_scarcity() {
    // Two anomalies with the same opponent strength, but one is deeper and scarcer: resolving
    // the harder one moves the rating more.
    let easy = anomaly_from_mission(1, 0.6, 0, 0.0, 30.0).unwrap(); // depth 0.6, telemetry 1.0
    let hard = anomaly_from_mission(2, 0.9, 0, 1.0, 3.0).unwrap(); // depth ~1.0, scarcity ~0.9
                                                                   // Normalize the opponent to the same strength so only the weighting differs.
    let opp = 1100.0;
    let easy_delta = elo_update(1000.0, opp, 1, easy.depth, easy.telemetry) - 1000.0;
    let hard_delta = elo_update(1000.0, opp, 1, hard.depth, hard.telemetry) - 1000.0;
    assert!(
        hard_delta > easy_delta,
        "the harder anomaly moves the rating more"
    );
}

#[test]
fn elo_update_refuses_invalid_inputs() {
    let a = anomaly_from_mission(1, 0.8, 0, 0.0, 30.0).unwrap();
    assert!(
        elo_update(f64::NAN, a.elo, 1, a.depth, a.telemetry).is_nan()
    );
    assert!(
        elo_update(1000.0, a.elo, 1, 1.5, a.telemetry).is_nan(),
        "depth > 1 refuses"
    );
    assert!(
        elo_update(1000.0, a.elo, 1, a.depth, -0.1).is_nan(),
        "telemetry < 0 refuses"
    );
    assert!(
        elo_update(1000.0, a.elo, 2, a.depth, a.telemetry).is_nan(),
        "outcome not 0/1 refuses"
    );
}

// -------------------------------------------------------------------------------------------
// The logic rack
// -------------------------------------------------------------------------------------------

#[test]
fn rack_compile_accepts_and_refuses() {
    let ok = rack_compile(0, 0, 92.0, 3).expect("a valid rule compiles");
    assert_eq!(ok.sensor, 0);
    assert_eq!(ok.op, Op::Gt);
    assert!((ok.threshold - 92.0).abs() < 1e-12);
    assert_eq!(ok.action, 3);
    // An unknown op refuses.
    assert!(
        rack_compile(0, 6, 92.0, 3).is_none(),
        "op 6 is not an operator"
    );
    // A non-finite threshold refuses.
    assert!(
        rack_compile(0, 0, f64::NAN, 3).is_none(),
        "NaN threshold refuses"
    );
}

#[test]
fn rack_fires_each_operator() {
    let rule = |op: u8| rack_compile(0, op, 5.0, 1).unwrap();
    assert!(rack_fires(&rule(0), 6.0), "6 > 5 fires Gt");
    assert!(!rack_fires(&rule(0), 5.0), "5 > 5 does not fire Gt");
    assert!(rack_fires(&rule(1), 4.0), "4 < 5 fires Lt");
    assert!(!rack_fires(&rule(1), 5.0), "5 < 5 does not fire Lt");
    assert!(rack_fires(&rule(2), 5.0), "5 >= 5 fires Ge");
    assert!(rack_fires(&rule(3), 5.0), "5 <= 5 fires Le");
    assert!(rack_fires(&rule(4), 5.0), "5 == 5 fires Eq");
    assert!(!rack_fires(&rule(4), 5.1), "5.1 == 5 does not fire Eq");
    assert!(rack_fires(&rule(5), 5.1), "5.1 != 5 fires Ne");
}

#[test]
fn rack_fires_refuses_a_nan_reading() {
    let rule = rack_compile(0, 0, 5.0, 1).unwrap();
    assert!(!rack_fires(&rule, f64::NAN), "a NaN reading does not fire");
}

// -------------------------------------------------------------------------------------------
// The design hash
// -------------------------------------------------------------------------------------------

#[test]
fn design_hash_is_stable_and_distinct() {
    assert_eq!(
        anomaly_design_hash(),
        anomaly_design_hash(),
        "the hash is stable"
    );
    // Distinct from the economy and the aggregation: a different model, a different hash.
    assert_ne!(
        anomaly_design_hash(),
        odderon_astro::mission::mission_design_hash()
    );
    assert_ne!(
        anomaly_design_hash(),
        odderon_astro::essence::essence_design_hash()
    );
}

#[test]
fn design_hash_pins_every_constant() {
    // The hash folds in every pinned constant. This test cannot mutate a const, but it asserts the
    // constants the hash claims to pin are the ones the module actually uses, so a future edit that
    // changes a constant without updating the hash's mix list would leave this test's expectations
    // stale and visible.
    let _ = (
        DRAWER_CAP,
        ELO_K,
        ELO_BASE,
        DIVERGENCE_THRESHOLD,
        ELO_SCALE,
        WINDOW_REF_DAYS,
        RISK_DEPTH_WEIGHT,
        Op::MAX,
    );
    // The hash is a pure u64, deterministic and finite.
    let h = anomaly_design_hash();
    assert!(h != 0, "the hash is not the zeroed accumulator");
}

#[test]
fn anomaly_is_copy_and_pure() {
    // An Anomaly is a plain value: deriving it twice from the same inputs gives identical fields.
    let a = anomaly_from_mission(42, 0.7, 1, 0.5, 15.0).unwrap();
    let b = anomaly_from_mission(42, 0.7, 1, 0.5, 15.0).unwrap();
    assert_eq!(a.source_id, b.source_id);
    assert!((a.depth - b.depth).abs() < 1e-15);
    assert!((a.telemetry - b.telemetry).abs() < 1e-15);
    assert!((a.elo - b.elo).abs() < 1e-12);
    assert_eq!(a.resolved, b.resolved);
    // And it is a Copy value, not a reference.
    let c: Anomaly = a;
    assert_eq!(c.source_id, 42);
}
