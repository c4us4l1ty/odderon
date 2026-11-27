//! The mission economy and the Esse calibration score.
//!
//! This module is the career core of the kernel: the two things that turn a launch into a *mission*
//! rather than an experiment. It answers the questions the bible's mission loop (§3.2) asks at the
//! PREDICT and DEBRIEF beats, and the economy (§12.1) asks at the contract beat.
//!
//! Two families, both pure functions of their arguments:
//!
//! * **The economy (§12.1 — "money is physics, not a difficulty knob").** A contract's price is a
//!   function of the work it asks for: the Δv budget, the mass, the risk, and the window. The price
//!   is computed here, in the kernel, from pinned constants -- never in the page. The page holds no
//!   price formula and no price constant, which is what makes the claim "money is physics" testable:
//!   a verifier, given the contract's parameters, calls [`contract_price`] and gets the same number
//!   the panel shows. The coefficients are a *documented model*, not a difficulty knob: they are
//!   pinned by [`mission_design_hash`] so a stale kernel is detectable, exactly as the rocket's
//!   design hash pins the Kármán line.
//!
//! * **Esse, the rating (§4.3).** `E_cal` -- calibration -- is "predicted vs. actual, across all
//!   domains", scored with **proper scoring rules** and never with an accuracy percentage. A proper
//!   rule cannot be gamed by hedging or by claiming impossible certainty, which is the whole reason
//!   the bible forbids accuracy %. Two rules are provided, matching the two kinds of prediction a
//!   mission produces:
//!
//!   - [`log_score`] for a **categorical** call: the player commits a probability that the mission
//!     meets its Δv budget, and the score is the log score of that probability against the outcome.
//!   - [`crps`] for an **interval** prediction: the player commits a predicted Δv range, and the
//!     score is the continuous ranked probability score of that interval against the achieved Δv.
//!
//! Both are proper: the expected score is minimised by the true distribution, so a player cannot
//! improve their rating by hedging or by over-claiming. That property is what the gate tests -- a
//! rule that rewards hedging is not a proper rule and is not Esse.
//!
//! Every function is a pure function of its arguments: the same inputs give the same output on every
//! platform, every run. That determinism is the P3 replay claim and it is what makes the score
//! recomputable from a verified replay (§4.3 rule 4).

use odderon_dmath::{fmax, is_finite, ln, pow};

use crate::rocket::{rocket_flight, LaunchParams, RocketVerdict};

// =============================================================================================
// The economy (§12.1)
// =============================================================================================

/// The reference Δv, km/s, that the price model is calibrated against. A LEO-class budget: the
/// work a typical orbital contract asks for. Pinned, documented, and folded into
/// [`mission_design_hash`].
pub const DV_REF_KM_S: f64 = 7.8;

/// The reference payload mass, kg. Pinned and folded into the design hash.
pub const MASS_REF_KG: f64 = 500.0;

/// The reference mission window, days. Pinned and folded into the design hash.
pub const WINDOW_REF_DAYS: f64 = 30.0;

/// The base contract price, in mission credits (MCU), at the reference point
/// `(DV_REF_KM_S, MASS_REF_KG, risk = 0, WINDOW_REF_DAYS)`. Pinned and folded into the design hash.
pub const PRICE_BASE_MCU: f64 = 100_000.0;

/// The exponent on Δv. Δv is the dominant cost driver: the propellant to impart it scales
/// exponentially (the rocket equation), so the price does too, but damped so the model stays
/// finite across the practical range rather than exploding.
pub const PRICE_DV_EXP: f64 = 1.6;

/// The exponent on mass. Mass scales the work roughly linearly: more payload, more propellant.
pub const PRICE_MASS_EXP: f64 = 1.0;

/// The exponent on the window. A shorter window is a rush job: more cost, because the same work
/// has to be done in less time. `window_factor = (WINDOW_REF / window)^PRICE_TIME_EXP`.
pub const PRICE_TIME_EXP: f64 = 0.5;

/// The contract price for a mission, in mission credits (MCU).
///
/// A pure function of the work the contract asks for, per §12.1:
///
/// ```text
/// price = PRICE_BASE * (dv / DV_REF)^PRICE_DV_EXP
///              * (mass / MASS_REF)^PRICE_MASS_EXP
///              * (1 + risk)
///              * (WINDOW_REF / window)^PRICE_TIME_EXP
/// ```
///
/// `risk` is a dimensionless multiplier (`0` = routine, `1` = double the risk premium); it is added
/// to `1`, never multiplied into the Δv term, so a risky routine contract is not the same as a
/// routine risky one. `window` is the contract's window in days.
///
/// `NaN` if any input is non-finite, if `dv`, `mass`, or `window` is non-positive, or if `risk` is
/// negative -- a refusal, not an answer, the same rule every other module follows.
pub fn contract_price(dv_km_s: f64, mass_kg: f64, risk: f64, window_days: f64) -> f64 {
    if !is_finite(dv_km_s)
        || !is_finite(mass_kg)
        || !is_finite(risk)
        || !is_finite(window_days)
        || dv_km_s <= 0.0
        || mass_kg <= 0.0
        || window_days <= 0.0
        || risk < 0.0
    {
        return f64::NAN;
    }
    let dv_term = pow(dv_km_s / DV_REF_KM_S, PRICE_DV_EXP);
    let mass_term = pow(mass_kg / MASS_REF_KG, PRICE_MASS_EXP);
    let risk_term = 1.0 + risk;
    let window_term = pow(WINDOW_REF_DAYS / window_days, PRICE_TIME_EXP);
    PRICE_BASE_MCU * dv_term * mass_term * risk_term * window_term
}

/// The Δv a mission actually achieved, km/s, for an authored launch.
///
/// The achieved Δv is the burnout velocity the rocket imparts -- the velocity at the end of the
/// burn, which is exactly the Δv the mission spent. It is read through the rocket kernel so that a
/// refused launch (a degenerate parameter set) yields `NaN` rather than a number: a mission that
/// never flew has no achieved Δv, and the score of a prediction against it is a refusal.
///
/// `NaN` if the launch is refused (non-finite or non-positive parameters, or a state that produces
/// no orbit). A valid launch returns the burnout velocity, which is the input `v` validated by the
/// rocket's own verdict -- not an echo of the input, but the kernel's confirmation that the launch
/// is one that flew.
pub fn achieved_dv_km_s(lat: f64, az: f64, el: f64, v: f64) -> f64 {
    let params = LaunchParams {
        latitude_rad: lat,
        azimuth_rad: az,
        elevation_rad: el,
        burnout_velocity_km_s: v,
    };
    let flight = rocket_flight(&params);
    if matches!(flight.verdict, RocketVerdict::Refused) {
        return f64::NAN;
    }
    v
}

// =============================================================================================
// Esse — the calibration score (§4.3)
// =============================================================================================

/// The log score of a categorical prediction, per §4.3.
///
/// The player commits a probability `p ∈ [0,1]` that a binary outcome occurs (e.g. that the mission
/// meets its Δv budget), *before* execution. The log score is:
///
/// ```text
/// score = -ln(p)        if the outcome occurred
///        = -ln(1 - p)   if it did not
/// ```
///
/// **Lower is better**: `0` is a perfect, certain, correct call; the score grows without bound as
/// the prediction approaches certainty and is wrong. It is a **proper** scoring rule: the expected
/// score is minimised by reporting the true probability, so hedging (reporting `0.5` always) and
/// over-claiming (reporting `1.0` and being wrong) are both punished. That is the property the
/// bible demands and the property accuracy % lacks -- accuracy rewards hedging, the log score does
/// not.
///
/// `NaN` if `p` is non-finite or outside `[0,1]`, or if `occurred` is not `0` or `1` -- a refusal,
/// not a score.
pub fn log_score(predicted_p: f64, occurred: u32) -> f64 {
    if !is_finite(predicted_p) || !(0.0..=1.0).contains(&predicted_p) {
        return f64::NAN;
    }
    if occurred != 0 && occurred != 1 {
        return f64::NAN;
    }
    // Guard the degenerate certainties: p = 0 with occurred, or p = 1 without, is an infinite
    // penalty -- the honest score for claiming certainty and being wrong. Return it as infinity
    // rather than NaN, because it is a real (if extreme) score, not a refusal. The other
    // degenerate pair (p = 1 and occurred, p = 0 and not) is a perfect 0.
    let p_if = if occurred == 1 {
        predicted_p
    } else {
        1.0 - predicted_p
    };
    if p_if == 0.0 {
        return f64::INFINITY;
    }
    -ln(p_if)
}

/// The continuous ranked probability score (CRPS) of an interval prediction, per §4.3.
///
/// The player commits a predicted interval `[lo, hi]` for a continuous quantity (e.g. the achieved
/// Δv), *before* execution. CRPS is the integral of the absolute difference between the predictive
/// CDF and the observed step function -- a single number that rewards both sharpness and accuracy.
/// For a uniform predictive distribution on `[lo, hi]` it has the closed form:
///
/// ```text
///   w = hi - lo
///   y ∈ [lo, hi]:  CRPS = w/6 + ((y-lo)² + (hi-y)² + (y-lo)(hi-y)) / (3w)
///   y <  lo:       CRPS = (lo - y) + w/2
///   y >  hi:       CRPS = (y - hi) + w/2
/// ```
///
/// **Lower is better**: `0` is a degenerate point prediction exactly at the outcome. It is a
/// **proper** scoring rule for a continuous forecast: the expected score is minimised by the true
/// distribution, so widening the interval to hedge is punished, and a sharp wrong prediction is
/// punished more than a wide one that contains the outcome. Both properties are what the bible
/// requires and what accuracy % cannot express.
///
/// `NaN` if any input is non-finite, if `lo >= hi` (a non-positive width is a refusal, not a
/// point prediction), or if the width underflows to zero -- a refusal, not a score.
pub fn crps(predicted_lo: f64, predicted_hi: f64, actual: f64) -> f64 {
    if !is_finite(predicted_lo) || !is_finite(predicted_hi) || !is_finite(actual) {
        return f64::NAN;
    }
    let w = predicted_hi - predicted_lo;
    if w <= 0.0 {
        return f64::NAN;
    }
    if actual < predicted_lo {
        return (predicted_lo - actual) + w * 0.5;
    }
    if actual > predicted_hi {
        return (actual - predicted_hi) + w * 0.5;
    }
    // actual ∈ [lo, hi]: the closed form. Clamp the terms so a rounding slip at the endpoints
    // cannot produce a tiny negative width in the denominator.
    let yl = fmax(actual - predicted_lo, 0.0);
    let yh = fmax(predicted_hi - actual, 0.0);
    let w_safe = fmax(w, f64::MIN_POSITIVE);
    w * (1.0 / 6.0) + (yl * yl + yh * yh + yl * yh) / (3.0 * w_safe)
}

/// The deterministic design hash of the mission economy's constants.
///
/// Pins the price model's reference point, the exponents, and the base price, so a replay bundle or
/// an Akashi entry can detect a stale economy kernel. A pure function: the same kernel produces the
/// same hash on every platform, every run. Folds in every pinned constant above, so changing any of
/// them changes the hash -- which is the point: the economy is a documented model, and the hash is
/// how a verifier knows which model produced a price.
pub fn mission_design_hash() -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut mix = |v: f64| {
        let bits = v.to_bits();
        hash ^= bits;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    };
    mix(DV_REF_KM_S);
    mix(MASS_REF_KG);
    mix(WINDOW_REF_DAYS);
    mix(PRICE_BASE_MCU);
    mix(PRICE_DV_EXP);
    mix(PRICE_MASS_EXP);
    mix(PRICE_TIME_EXP);
    hash
}
