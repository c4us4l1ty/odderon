//! The Anomaly Drawer and the logic rack — the failure-design and session-architecture layer
//! (bible §7.3, §7.4, §5.5, §4, §9.2).
//!
//! [`crate::mission`] scores a single prediction; [`crate::essence`] aggregates a career into the
//! `E_cal` calibration rating. This module is the *third* Esse axis: **E_an**, the Anomaly rating
//! (§4: "Blind, ranked, clocked root-cause resolution. Duel-style Elo per mission; weighted by how
//! deep the fault was hidden and how little telemetry you were given"). It is built on three
//! mechanisms the bible names, and each is a pure function so it is recomputable from a verified
//! replay:
//!
//! * **The Anomaly Drawer (§7.3).** A mission that diverged — the committed prediction was wrong by
//!   more than a pinned threshold — *opens an anomaly*: a specific, small, real discrepancy that
//!   lives in the drawer until it is root-caused. The drawer holds **at most [`DRAWER_CAP`]** open
//!   anomalies; when full, [`drawer_can_open`] refuses a sixth until one is closed. This is the
//!   Zeigarnik effect with a budget: enough to pull the player back tomorrow, never enough to become
//!   dread.
//! * **Rescue over restart (§7.4).** A bad situation is resolved, not reloaded. Resolving an
//!   anomaly — root-causing it — is a [`elo_update`]: the player's `E_an` rating moves by a
//!   duel-style Elo delta, weighted by the fault's depth and the telemetry scarcity. The anomaly is
//!   closed and removed from the drawer. Nothing is lost by closing the tab: the drawer is an input
//!   log (§9.2), so it always exists.
//! * **The logic rack (§5.5).** Automation is a physical object, not a setting. A rule
//!   `IF sensor > threshold THEN action` is **compiled** by [`rack_compile`] into a deterministic
//!   predicate and **fired** by [`rack_fires`] against a sensor reading. It compiles into the same
//!   deterministic tick loop: the same rule and the same reading produce the same fire on every
//!   platform, every run. Authored automation is permitted; the kernel only says whether the rule
//!   fires — the page owns the tick loop and the side effect.
//!
//! Every function is a pure function of its arguments: the same inputs give the same output on every
//! platform, every run. That determinism is what makes the drawer and the rack **recomputable** from
//! a verified replay (§4.3 rule 4) — a verifier replays the input log and gets the same anomalies,
//! the same cap refusal, the same Elo delta, the same fire. The rating is a function of the replay,
//! not a server's word.
//!
//! This crate is `no_std` with no allocator, so the anomaly and the rack rule are fixed-size
//! structs, not `Vec`: the drawer cap and the sensor/action ids are pinned constants, and a career
//! ledger is bounded by them. No allocation, no hidden state, no clock — the same rules every kernel
//! module follows.

use odderon_dmath::{fabs, fmax, fmin, is_finite, pow};

// =============================================================================================
// The pinned constants
// =============================================================================================

/// The maximum number of anomalies the drawer holds open at once (§7.3: "at most five"). When the
/// drawer is full, [`drawer_can_open`] refuses a sixth until one is resolved. Pinned, and folded
/// into [`anomaly_design_hash`].
pub const DRAWER_CAP: usize = 5;

/// The Elo K-factor (the standard 32). The effective K in [`elo_update`] is weighted above this by
/// the fault's depth and the telemetry scarcity, so a deeper fault resolved with less telemetry
/// moves the rating more. Pinned, and folded into [`anomaly_design_hash`].
pub const ELO_K: f64 = 32.0;

/// The starting `E_an` rating. A new player begins at a neutral 1000, the standard Elo origin.
/// Pinned, and folded into [`anomaly_design_hash`].
pub const ELO_BASE: f64 = 1000.0;

/// The divergence below which a mission does **not** open an anomaly. A 5% discrepancy between the
/// committed prediction and the outcome is the floor of "a real discrepancy you cannot explain yet"
// (§7.3's example is a 3% thrust discrepancy); anything smaller is rounding, not an anomaly. Pinned,
// and folded into [`anomaly_design_hash`].
pub const DIVERGENCE_THRESHOLD: f64 = 0.05;

/// The scale that maps a fault's depth + telemetry scarcity onto the anomaly's Elo strength. A
/// deeper fault hidden behind less telemetry is a stronger opponent in the duel. Pinned, and folded
/// into [`anomaly_design_hash`].
pub const ELO_SCALE: f64 = 200.0;

/// The mission's reference window (days), reused from the economy kernel so the telemetry a mission
// provides is measured against the same reference the price model uses. Pinned, and folded into
// [`anomaly_design_hash`].
pub const WINDOW_REF_DAYS: f64 = crate::mission::WINDOW_REF_DAYS;

/// How much mission risk contributes to a fault's depth. A riskier mission hides faults deeper.
/// Pinned, and folded into [`anomaly_design_hash`].
pub const RISK_DEPTH_WEIGHT: f64 = 0.5;

// =============================================================================================
// The anomaly
// =============================================================================================

/// An anomaly: a specific, small, real discrepancy opened by a mission that diverged, living in the
/// drawer until it is root-caused (§7.3).
///
/// The `depth` is how deep the fault was hidden and the `telemetry` is how much telemetry the
/// mission provided (`1.0` = full). Both are normalized to `[0, 1]` and both feed the anomaly's
/// Elo strength and the weight of its resolution — the §4 weighting: "how deep the fault was hidden
/// and how little telemetry you were given."
#[derive(Clone, Copy)]
pub struct Anomaly {
    /// The id of the mission that opened this anomaly, so a resolver can point back at the divergent
    /// mission and a verifier can replay it.
    pub source_id: u32,
    /// How deep the fault was hidden, normalized to `[0, 1]`. Higher means a deeper fault.
    pub depth: f64,
    /// How much telemetry the mission provided, normalized to `[0, 1]`. `1.0` = full telemetry.
    pub telemetry: f64,
    /// The anomaly's Elo strength — the opponent rating in the duel. Derived from `depth` and the
    /// telemetry scarcity (`1 - telemetry`): a deeper fault behind less telemetry is stronger.
    pub elo: f64,
    /// Whether the anomaly has been resolved (root-caused). An open anomaly is `false`.
    pub resolved: bool,
}

// =============================================================================================
// The logic rack
// =============================================================================================

/// The comparison operator of a logic-rack rule (§5.5: `IF pump_housing_temp > 92 K THEN ...`).
///
/// Encoded as a `u8` so it crosses the wasm ABI as a plain integer; the kernel maps it back to the
/// comparison in [`rack_fires`]. Pinned, and folded into [`anomaly_design_hash`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Op {
    /// `>` — strictly greater.
    Gt = 0,
    /// `<` — strictly less.
    Lt = 1,
    /// `>=` — greater or equal.
    Ge = 2,
    /// `<=` — less or equal.
    Le = 3,
    /// `==` — equal.
    Eq = 4,
    /// `!=` — not equal.
    Ne = 5,
}

impl Op {
    /// The largest valid operator code. Used by [`rack_compile`] to refuse an unknown op.
    pub const MAX: u8 = 5;

    /// Decode an operator code, or `None` if it is not a valid [`Op`].
    ///
    /// Public so the wasm ABI can reconstruct an [`Op`] from the `u8` code a host wrote into linear
    /// memory (the `RackRule` crosses the ABI as a flat buffer of f64, and the op is one of them).
    pub fn from_code(code: u8) -> Option<Op> {
        match code {
            0 => Some(Op::Gt),
            1 => Some(Op::Lt),
            2 => Some(Op::Ge),
            3 => Some(Op::Le),
            4 => Some(Op::Eq),
            5 => Some(Op::Ne),
            _ => None,
        }
    }
}

/// A compiled logic-rack rule (§5.5): `IF <sensor> <op> <threshold> THEN <action>`.
///
/// This is the *compiled* form. [`rack_compile`] validates the raw inputs and produces this struct;
/// [`rack_fires`] evaluates it against a single sensor reading. The rule is a pure function of its
/// fields and the reading, so it compiles into the deterministic tick loop: the same rule and the
/// same reading fire the same action on every platform, every run. The `action` is an id the page
/// interprets — the kernel only says whether the rule fires.
#[derive(Clone, Copy)]
pub struct RackRule {
    /// Which sensor the rule reads. An index into the page's sensor set; the kernel does not name
    /// sensors, it evaluates the predicate against the reading the page provides.
    pub sensor: u32,
    /// The comparison operator.
    pub op: Op,
    /// The threshold the sensor is compared against.
    pub threshold: f64,
    /// The action id to take when the rule fires. Interpreted by the page's tick loop.
    pub action: u32,
}

// =============================================================================================
// The drawer and the anomaly
// =============================================================================================

/// Derive an anomaly from a mission that diverged, per §7.3 and §4.
///
/// Each input is a raw mission fact, not an aggregate: `predicted_p` is the probability the player
/// committed before execution, `occurred` is whether the outcome actually happened (the same pair
/// [`crate::mission::log_score`] scores), `risk` is the mission's risk factor and `window_days` is
/// its window. The kernel derives everything else:
///
/// * the **divergence** is `|predicted_p − occurred|`, the absolute prediction error. Below
///   [`DIVERGENCE_THRESHOLD`] it is rounding, not an anomaly — the mission did not diverge enough to
///   open one, and `None` is returned.
/// * the **depth** is the divergence plus the mission's risk (a riskier mission hides faults
///   deeper), clamped to `[0, 1]`.
/// * the **telemetry** is the window against [`WINDOW_REF_DAYS`] (a longer window gives more
///   observation time), clamped to `[0, 1]`.
/// * the **elo** strength is [`ELO_BASE`] plus `(depth + scarcity) · [`ELO_SCALE`]`, where
///   `scarcity = 1 − telemetry`: a deeper fault behind less telemetry is a stronger opponent.
///
/// **Refusals.** `None` is returned — the mission does not open an anomaly — if `predicted_p` is
/// non-finite or outside `[0, 1]`, if `occurred` is not `0` or `1`, if `risk` is non-finite or
/// negative, if `window_days` is non-finite or non-positive, or if the divergence is below
/// [`DIVERGENCE_THRESHOLD`]. A mission that did not diverge is not an anomaly; an invalid mission is
/// not a rating. A refusal is `None`, never a fabricated anomaly.
pub fn anomaly_from_mission(
    source_id: u32,
    predicted_p: f64,
    occurred: u32,
    risk: f64,
    window_days: f64,
) -> Option<Anomaly> {
    // Validate every input before computing anything: one invalid fact means the mission is not a
    // replay the drawer can be computed from.
    if !is_finite(predicted_p) || !(0.0..=1.0).contains(&predicted_p) {
        return None;
    }
    if occurred != 0 && occurred != 1 {
        return None;
    }
    if !is_finite(risk) || risk < 0.0 {
        return None;
    }
    if !is_finite(window_days) || window_days <= 0.0 {
        return None;
    }

    // The divergence: the absolute prediction error. `occurred` as a float, then the absolute
    // difference. `fabs` from odderon_dmath (R1: no host transcendental).
    let occurred_f = if occurred == 1 { 1.0 } else { 0.0 };
    let divergence = fabs(predicted_p - occurred_f);

    // Below the threshold the mission did not diverge enough to open an anomaly: a 2% miss is
    // rounding, not a discrepancy worth a drawer slot.
    if divergence < DIVERGENCE_THRESHOLD {
        return None;
    }

    // The depth: the divergence plus the mission's risk, clamped to [0, 1]. `fmin`/`fmax` are free
    // functions from odderon_dmath (R1); host float methods are not available in the no_std kernel.
    let depth = fmin(fmax(divergence + risk * RISK_DEPTH_WEIGHT, 0.0), 1.0);
    // The telemetry: the window against the reference, clamped to [0, 1]. A longer window gives more
    // observation time, so more telemetry.
    let telemetry = fmin(fmax(window_days / WINDOW_REF_DAYS, 0.0), 1.0);
    // The scarcity is the complement: how little telemetry you were given.
    let scarcity = 1.0 - telemetry;

    // The anomaly's Elo strength: a deeper fault behind less telemetry is a stronger opponent.
    let elo = ELO_BASE + (depth + scarcity) * ELO_SCALE;

    Some(Anomaly { source_id, depth, telemetry, elo, resolved: false })
}

/// Whether the drawer can open another anomaly, per §7.3: "at most five."
///
/// The drawer holds at most [`DRAWER_CAP`] open anomalies. With `open_count` already open, a new
/// one can be opened only if `open_count < DRAWER_CAP`. When the drawer is full, the caller must
/// resolve (close) an anomaly before opening another — the Zeigarnik budget. A pure predicate: the
/// same count gives the same answer on every platform, every run.
pub fn drawer_can_open(open_count: usize) -> bool {
    open_count < DRAWER_CAP
}

// =============================================================================================
// The E_an Elo
// =============================================================================================

/// The Elo expected score of `rating` against `opponent`, per §4's "duel-style Elo."
///
/// The standard Elo expectation: `1 / (1 + 10^((opponent − rating) / 400))`. A rating equal to the
/// opponent scores `0.5`; a higher rating scores above `0.5`. Computed with [`pow`] from
/// `odderon_dmath` (R1: the only transcendental here is the kernel's own `pow`, so the expectation
/// is deterministic across platforms).
///
/// `NaN` if either rating is non-finite — a rating with no meaning is refused, not answered.
pub fn elo_expected(rating: f64, opponent: f64) -> f64 {
    if !is_finite(rating) || !is_finite(opponent) {
        return f64::NAN;
    }
    let exponent = (opponent - rating) / 400.0;
    1.0 / (1.0 + pow(10.0, exponent))
}

/// The weighted `E_an` update from resolving an anomaly, per §4.
///
/// Resolving an anomaly is a duel: the player's `E_an` rating (`rating`) faces the anomaly's strength
/// (`opponent`), and the outcome (`1` = correctly root-caused, `0` = not) moves the rating by the
/// standard Elo delta `K · (outcome − expected)`. The K-factor is **weighted** by the fault's depth
/// and the telemetry scarcity — "weighted by how deep the fault was hidden and how little telemetry
/// you were given":
///
/// ```text
/// scarcity  = 1 − telemetry
/// K_eff     = ELO_K · (1 + depth) · (1 + scarcity)
/// delta     = K_eff · (outcome − expected)
/// new_rating = rating + delta
/// ```
///
/// A deeper fault resolved with less telemetry moves the rating more, because it was harder. The
/// anomaly's `depth` and `telemetry` are the ones [`anomaly_from_mission`] derived, so the weighting
/// is the kernel's, not the page's.
///
/// **Refusals.** `NaN` if `rating` or `opponent` is non-finite, if `depth` or `telemetry` is
/// non-finite or outside `[0, 1]`, or if `outcome` is not `0` or `1`. A refusal is never a fabricated
/// rating — a rating with no valid inputs has no meaning.
pub fn elo_update(rating: f64, opponent: f64, outcome: u32, depth: f64, telemetry: f64) -> f64 {
    if !is_finite(rating) || !is_finite(opponent) {
        return f64::NAN;
    }
    if !is_finite(depth) || !(0.0..=1.0).contains(&depth) {
        return f64::NAN;
    }
    if !is_finite(telemetry) || !(0.0..=1.0).contains(&telemetry) {
        return f64::NAN;
    }
    if outcome != 0 && outcome != 1 {
        return f64::NAN;
    }

    let expected = elo_expected(rating, opponent);
    // The effective K, weighted by the fault's depth and the telemetry scarcity.
    let scarcity = 1.0 - telemetry;
    let k_eff = ELO_K * (1.0 + depth) * (1.0 + scarcity);
    let outcome_f = if outcome == 1 { 1.0 } else { 0.0 };
    rating + k_eff * (outcome_f - expected)
}

// =============================================================================================
// The logic rack
// =============================================================================================

/// Compile a logic-rack rule (§5.5: `IF sensor > threshold THEN action`).
///
/// Validates the raw inputs and produces the compiled [`RackRule`] the tick loop fires. The rule is
/// a pure function of its fields, so compiling is deterministic: the same inputs compile to the same
/// rule on every platform, every run.
///
/// **Refusals.** `None` is returned if `op` is not a valid [`Op`] code, if `threshold` is
/// non-finite, or if `sensor` or `action` exceeds the pinned `u32` id space the page uses. An
/// invalid rule is refused, never compiled into a predicate that fires on the wrong condition.
pub fn rack_compile(sensor: u32, op: u8, threshold: f64, action: u32) -> Option<RackRule> {
    let op = Op::from_code(op)?;
    if !is_finite(threshold) {
        return None;
    }
    // The sensor and action ids are plain u32s the page interprets; any u32 is a valid id, so there
    // is no range to refuse beyond the type itself. The rule is structurally valid once the op is
    // known and the threshold is finite.
    Some(RackRule { sensor, op, threshold, action })
}

/// Evaluate a compiled rack rule against a sensor reading, per §5.5.
///
/// Returns `true` if the rule fires — if `<reading> <op> <threshold>` holds — and the page's tick
/// loop takes the rule's `action`. The evaluation is a pure comparison: the same rule and the same
/// reading fire the same action on every platform, every run, which is what makes the rack
/// compile into the deterministic tick loop.
///
/// `NaN` readings are refused: a sensor with no reading has no value to compare, so the rule does
/// not fire (returns `false`) rather than answering with a fabricated comparison. A rule with a
/// `NaN` threshold was refused by [`rack_compile`] and never reaches here.
pub fn rack_fires(rule: &RackRule, reading: f64) -> bool {
    if !is_finite(reading) {
        return false;
    }
    match rule.op {
        Op::Gt => reading > rule.threshold,
        Op::Lt => reading < rule.threshold,
        Op::Ge => reading >= rule.threshold,
        Op::Le => reading <= rule.threshold,
        Op::Eq => reading == rule.threshold,
        Op::Ne => reading != rule.threshold,
    }
}

// =============================================================================================
// The design hash
// =============================================================================================

/// The deterministic design hash of the anomaly/drawer/rack constants.
///
/// Pins the drawer cap, the Elo constants, the divergence threshold, the Elo scale, the risk
/// weight, and the operator set, so a replay bundle or an Akashi entry can detect a stale
/// failure-design kernel — the same way [`crate::mission::mission_design_hash`] pins the economy
/// and [`crate::essence::essence_design_hash`] pins the aggregation. A pure function: the same
/// kernel produces the same hash on every platform, every run. Folding in every pinned constant
/// means changing any of them changes the hash, which is the point: the failure design is a
/// documented model, and the hash is how a verifier knows which model produced an anomaly or a
/// rule.
pub fn anomaly_design_hash() -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut mix = |v: f64| {
        let bits = v.to_bits();
        hash ^= bits;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    };
    mix(DRAWER_CAP as f64);
    mix(ELO_K);
    mix(ELO_BASE);
    mix(DIVERGENCE_THRESHOLD);
    mix(ELO_SCALE);
    mix(WINDOW_REF_DAYS);
    mix(RISK_DEPTH_WEIGHT);
    mix(Op::MAX as f64);
    hash
}
