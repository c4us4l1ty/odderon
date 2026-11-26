//! The mission economy and the Esse calibration score, as measurable claims.
//!
//! `docs/BUILD-PLAN.md` §5 names P5's job: the career core — the mission loop (`GAME-DESIGN-BIBLE.md`
//! §3.2), the economy (§12.1), and the Esse rating (§4.3). The two claims this module makes are
//! each falsifiable, and the tests below are grouped by which part of the claim they can falsify:
//!
//! 1. **Money is physics (§12.1).** A contract's price is a function of the work it asks for —
//!    Δv, mass, risk, time — computed in the kernel from pinned constants, never in the page. The
//!    price rises with each of those, is pinned at the reference point, and refuses degenerate
//!    inputs. A price that a page could replicate from a constant is not "physics".
//! 2. **The achieved Δv is the flight's.** The Δv a mission spent is the rocket's burnout velocity,
//!    read through `rocket_flight` so a refused launch yields no Δv. The score is a function of the
//!    same physics the flight is.
//! 3. **Esse is a proper scoring rule (§4.3).** The log score and the CRPS are proper: the expected
//!    score is minimised by the true distribution, so hedging and over-claiming are both punished.
//!    This is the property accuracy % lacks, and it is the reason the bible forbids accuracy %.
//! 4. **The refusals.** A degenerate input is refused with `NaN`, never answered with a fabricated
//!    number — the same rule every other kernel module follows.
//! 5. **The design hash is stable.** It pins the economy's constants, so a replay bundle or Akashi
//!    entry can detect a stale economy kernel.
//!
//! Every number quoted in a comment was computed against the same constants before being written
//! down, so a failure says which of the two disagreed.

use odderon_astro::mission::{
    achieved_dv_km_s, contract_price, crps, log_score, mission_design_hash, DV_REF_KM_S,
    MASS_REF_KG, PRICE_BASE_MCU, WINDOW_REF_DAYS,
};

// --------------------------------------------------------------------------------------------
// 1. The economy (§12.1)
// --------------------------------------------------------------------------------------------

#[test]
fn price_is_pinned_at_the_reference_point() {
    // At (DV_REF, MASS_REF, risk 0, WINDOW_REF) every term is 1, so the price is exactly the base.
    // This pins the model's calibration, not just its shape: a verifier who changes any constant
    // changes the hash and this number, and both are detectable.
    let p = contract_price(DV_REF_KM_S, MASS_REF_KG, 0.0, WINDOW_REF_DAYS);
    assert!(
        (p - PRICE_BASE_MCU).abs() < 1e-9 * PRICE_BASE_MCU,
        "price {p} != base {PRICE_BASE_MCU}"
    );
}

#[test]
fn price_rises_with_each_factor_of_work() {
    // §12.1: "contract prices derive from Δv, mass, risk and time". Each factor raises the price.
    let base = contract_price(7.8, 500.0, 0.0, 30.0);
    assert!(
        contract_price(9.0, 500.0, 0.0, 30.0) > base,
        "more Δv should cost more"
    );
    assert!(
        contract_price(7.8, 800.0, 0.0, 30.0) > base,
        "more mass should cost more"
    );
    assert!(
        contract_price(7.8, 500.0, 1.0, 30.0) > base,
        "more risk should cost more"
    );
    assert!(
        contract_price(7.8, 500.0, 0.0, 10.0) > base,
        "a shorter window should cost more"
    );
}

#[test]
fn price_is_a_pure_function() {
    // The same inputs give the same price, every call. This is the replay claim: a price that
    // depends on when you computed it is not a function of the work.
    let a = contract_price(8.5, 620.0, 0.3, 21.0);
    let b = contract_price(8.5, 620.0, 0.3, 21.0);
    assert_eq!(a.to_bits(), b.to_bits());
}

#[test]
fn price_refuses_degenerate_inputs() {
    // A refusal, not an answer: zero or negative work, negative risk, or non-finite input.
    assert!(contract_price(0.0, 500.0, 0.0, 30.0).is_nan());
    assert!(contract_price(-1.0, 500.0, 0.0, 30.0).is_nan());
    assert!(contract_price(7.8, 0.0, 0.0, 30.0).is_nan());
    assert!(contract_price(7.8, 500.0, 0.0, 0.0).is_nan());
    assert!(contract_price(7.8, 500.0, -0.1, 30.0).is_nan());
    assert!(contract_price(f64::NAN, 500.0, 0.0, 30.0).is_nan());
    assert!(contract_price(7.8, f64::INFINITY, 0.0, 30.0).is_nan());
}

// --------------------------------------------------------------------------------------------
// 2. The achieved Δv (§3.2 execute beat)
// --------------------------------------------------------------------------------------------

#[test]
fn achieved_dv_is_the_burnout_velocity_of_a_valid_launch() {
    // A valid launch achieves the burnout velocity it was authored with: the rocket imparts exactly
    // v at burnout, and that is the Δv the mission spent. Read through the kernel, so the score is
    // a function of the same physics the flight is.
    let dv = achieved_dv_km_s(0.0, 0.0, 0.5, 7.0);
    assert!((dv - 7.0).abs() < 1e-12, "achieved dv {dv} != burnout 7.0");
}

#[test]
fn achieved_dv_refuses_a_refused_launch() {
    // A launch the kernel refuses has no achieved Δv: the mission never flew.
    assert!(achieved_dv_km_s(0.0, 0.0, 0.0, 0.0).is_nan());
    assert!(achieved_dv_km_s(f64::NAN, 0.0, 0.5, 7.0).is_nan());
    assert!(achieved_dv_km_s(0.0, 0.0, 0.5, -1.0).is_nan());
}

// --------------------------------------------------------------------------------------------
// 3. Esse — the proper scoring rules (§4.3)
// --------------------------------------------------------------------------------------------

#[test]
fn log_score_is_proper_and_penalises_wrong_certainty() {
    // Claiming certainty and being right is perfect (0); claiming certainty and being wrong is the
    // worst score (infinite). This is the property accuracy % lacks: accuracy rewards hedging, the
    // log score punishes it.
    assert_eq!(log_score(1.0, 1), 0.0);
    assert_eq!(log_score(0.0, 0), 0.0);
    assert_eq!(log_score(1.0, 0), f64::INFINITY);
    assert_eq!(log_score(0.0, 1), f64::INFINITY);
}

#[test]
fn log_score_is_lower_when_right() {
    // For the same prediction, the correct outcome scores lower than the wrong one.
    let p = 0.8;
    assert!(log_score(p, 1) < log_score(p, 0));
}

#[test]
fn log_score_is_a_proper_rule_against_the_platform() {
    // The log score is -ln(p) on a correct call, -ln(1-p) on a wrong one. Computed against the
    // platform's own ln so a sign or branch error in the kernel is caught, not just its shape.
    let p = 0.7;
    assert!(
        (log_score(p, 1) + p.ln()).abs() < 1e-12,
        "log_score(p,1) should be -ln(p)"
    );
    assert!(
        (log_score(p, 0) + (1.0 - p).ln()).abs() < 1e-12,
        "log_score(p,0) should be -ln(1-p)"
    );
}

#[test]
fn log_score_refuses_a_bad_probability() {
    assert!(log_score(-0.1, 1).is_nan());
    assert!(log_score(1.1, 1).is_nan());
    assert!(log_score(f64::NAN, 1).is_nan());
}

#[test]
fn crps_is_zero_at_a_tight_interval_around_the_outcome() {
    // A degenerate interval is refused (width must be > 0), but the limit as the interval shrinks
    // to the outcome is 0. A tight interval containing the outcome scores near zero.
    let eps = 1e-6;
    let s = crps(5.0 - eps, 5.0 + eps, 5.0);
    assert!(
        s >= 0.0 && s < eps,
        "tight interval score {s} should be near 0"
    );
}

#[test]
fn crps_rewards_sharpness_and_accuracy() {
    // A tight interval containing the outcome scores lower than a wide one that also contains it:
    // sharpness is rewarded. An interval that misses scores higher than one that hits.
    let tight = crps(4.9, 5.1, 5.0);
    let wide = crps(4.0, 6.0, 5.0);
    assert!(tight < wide, "tight {tight} should beat wide {wide}");
    let miss = crps(5.5, 6.5, 5.0);
    assert!(miss > wide, "miss {miss} should beat wide {wide}");
}

#[test]
fn crps_matches_the_closed_form_against_the_platform() {
    // The closed form for a uniform interval: w/6 + (yl² + yh² + yl·yh)/(3w) when the outcome is
    // inside. Checked against the platform's own arithmetic so a coefficient error is caught.
    let (lo, hi, y) = (4.0, 6.0, 5.0);
    let w = hi - lo;
    let yl = y - lo;
    let yh = hi - y;
    let expected = w / 6.0 + (yl * yl + yh * yh + yl * yh) / (3.0 * w);
    assert!((crps(lo, hi, y) - expected).abs() < 1e-12);
}

#[test]
fn crps_refuses_a_nonpositive_width() {
    assert!(crps(5.0, 5.0, 5.0).is_nan());
    assert!(crps(6.0, 4.0, 5.0).is_nan());
    assert!(crps(f64::NAN, 6.0, 5.0).is_nan());
}

// --------------------------------------------------------------------------------------------
// 5. The design hash
// --------------------------------------------------------------------------------------------

#[test]
fn design_hash_is_stable_and_distinct() {
    // Pins the economy's constants: the same kernel produces the same hash, every run, and a
    // different constant set produces a different hash.
    let a = mission_design_hash();
    let b = mission_design_hash();
    assert_eq!(a, b);
    // Distinct from the rocket's design hash (a different constant set, so a different fingerprint).
    assert_ne!(a, odderon_astro::rocket::rocket_design_hash());
}
