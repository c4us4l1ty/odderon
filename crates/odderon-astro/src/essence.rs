//! The Esse career ledger — the calibration rating aggregated across a career (§4.3).
//!
//! [`crate::mission`] scores a *single* prediction with a proper rule ([`crate::mission::log_score`],
//! [`crate::mission::crps`]). This module aggregates a *sequence* of scored predictions into the
//! `E_cal` rating §4.3 defines: "Predicted vs. actual, across all domains... Aggregated into a
//! reliability curve, **not an average**. Reported with N and a 95% interval. Provisional until
//! N ≥ 30."
//!
//! The aggregation is the point, and it is deliberately **not** a running mean. §4.3 rule 1 forbids
//! collapsing calibration into a single average, because an average cannot tell a *calibrated*
//! forecaster (predicted ≈ observed in every bin) from a merely *lucky* one — both can post the same
//! mean. So `E_cal` is reported as:
//!
//! * the **reliability curve** — the predicted probability binned against the observed frequency in
//!   each bin. A calibrated forecaster sits on the diagonal; a biased one is systematically above or
//!   below it. "Aggregated into a reliability curve, not an average" is exactly this curve.
//! * **N**, the number of scored predictions.
//! * the **mean log score** — the proper scalar summary (lower is better), computed by re-scoring
//!   every prediction through [`crate::mission::log_score`] so the aggregate is the same proper rule
//!   the per-mission score uses, never an accuracy percentage.
//! * the **brier score** — a second proper scalar, the mean squared error of the predictions.
//! * the **bias** — mean predicted probability minus observed frequency. This is the §2.2(a)
//!   competence-mirror number: "your bias is +0.8%... that bias is your next lesson."
//! * a **95% interval** on the mean log score, from the standard error of the mean.
//! * **provisional** — true until N ≥ [`PROVISIONAL_N`], because a rating from too few samples is a
//!   claim, not a measurement.
//!
//! Every function is a pure function of its arguments: the same predictions give the same rating on
//! every platform, every run. That determinism is what makes the rating **recomputable** from a
//! verified replay (§4.3 rule 4) — a verifier downloads the input log and re-simulates, and gets the
//! same curve, the same N, the same interval. The rating is a function of the replay, not a server's
//! word.
//!
//! This crate is `no_std` with no allocator, so the reliability curve is a fixed-size array
//! [`ReliabilityBin; BIN_COUNT`], not a `Vec`: the bin count is a pinned constant, and a career ledger
//! is bounded by it. No allocation, no hidden state, no clock — the same rules every kernel module
//! follows.

use odderon_dmath::{fmax, is_finite, sqrt};

use crate::mission::log_score;

// =============================================================================================
// The pinned aggregation constants
// =============================================================================================

/// The number of bins the reliability curve is drawn in, over the predicted-probability range
/// `[0, 1]`. Ten bins is the standard calibration resolution: fine enough to expose a bias that an
/// average would hide, coarse enough that a career's N fills each bin rather than leaving them empty.
/// Pinned, and folded into [`essence_design_hash`].
pub const BIN_COUNT: usize = 10;

/// The minimum N before a calibration rating is *measured* rather than *provisional*, per §4.3:
/// "Provisional until N ≥ 30." A rating from fewer than this many predictions is reported, but flagged
/// as a claim rather than a measurement. Pinned, and folded into [`essence_design_hash`].
pub const PROVISIONAL_N: usize = 30;

/// The two-sided 95% normal critical value, pinned so the interval is deterministic across platforms
/// and targets. It is a constant, not a transcendental; the only transcendental the interval needs is
/// the [`sqrt`] in the standard error, which comes from `odderon_dmath` (R1). Pinned, and folded into
/// [`essence_design_hash`].
pub const CI95_Z: f64 = 1.96;

/// One bin of the reliability curve: the predicted-probability interval and what was actually
/// observed inside it.
#[derive(Clone, Copy)]
pub struct ReliabilityBin {
    /// The bin's midpoint, the predicted probability the bin represents. `(2i + 1) / (2 * BIN_COUNT)`.
    pub center: f64,
    /// The mean predicted probability of the predictions that fell in the bin. `NaN` if the bin is
    /// empty, so an empty bin is visibly a hole rather than a zero that reads as a measurement.
    pub predicted: f64,
    /// The observed frequency in the bin — the fraction of the bin's predictions whose outcome
    /// occurred. `NaN` if the bin is empty. A calibrated forecaster has `predicted ≈ observed`.
    pub observed: f64,
    /// The number of predictions that fell in the bin.
    pub count: usize,
}

/// The `E_cal` calibration rating aggregated from a career's scored predictions, per §4.3.
///
/// A refusal is expressed as `NaN` in every computed field (`mean_log_score`, `brier`, `bias`, the
/// interval bounds, and each bin's `predicted`/`observed`), with `n` echoing the input length and
/// `provisional` reflecting it. The same rule every other module follows: a degenerate or invalid
/// input is refused, never answered with a fabricated number.
#[derive(Clone, Copy)]
pub struct EsseCal {
    /// The number of scored predictions the rating was computed from.
    pub n: usize,
    /// The mean log score across the predictions (lower is better). `NaN` if the input is empty or
    /// invalid.
    pub mean_log_score: f64,
    /// The brier score — the mean squared error of the predictions (lower is better). `NaN` if the
    /// input is empty or invalid.
    pub brier: f64,
    /// The calibration bias: mean predicted probability minus observed frequency. Positive means the
    /// forecaster over-predicts the outcome. `NaN` if the input is empty or invalid.
    pub bias: f64,
    /// The reliability curve, one bin per entry, in order from the lowest predicted probability.
    pub bins: [ReliabilityBin; BIN_COUNT],
    /// True until N ≥ [`PROVISIONAL_N`]: the rating is a claim, not yet a measurement.
    pub provisional: bool,
    /// The lower bound of the 95% interval on the mean log score. `NaN` if the input is empty or
    /// invalid, or if N is too small for an interval (see [`essence_cal`]).
    pub ci95_lo: f64,
    /// The upper bound of the 95% interval on the mean log score.
    pub ci95_hi: f64,
}

/// Aggregate a career's scored predictions into the `E_cal` calibration rating, per §4.3.
///
/// Each prediction is a `(predicted_p, occurred)` pair — the probability the player committed before
/// execution and whether the outcome actually occurred — exactly the pair [`crate::mission::log_score`]
/// scores. The rating is a **pure function of the slice**: the same predictions give the same rating
/// on every platform, every run, which is what makes it recomputable from a verified replay.
///
/// The reliability curve bins the predicted probabilities into [`BIN_COUNT`] equal intervals over
/// `[0, 1]` and compares each bin's mean predicted probability against its observed frequency. The
/// mean log score and the brier score are the two proper scalar summaries; the bias is the §2.2(a)
/// competence-mirror number. The 95% interval is `mean ± CI95_Z * (σ / √N)`, with `σ` the sample
/// standard deviation of the log scores.
///
/// **Refusals.** An empty slice, or any prediction whose `predicted_p` is non-finite or outside
/// `[0, 1]` or whose `occurred` is not `0` or `1`, refuses the whole rating: every computed field is
/// `NaN`. A rating with a poisoned entry is not a rating — a verified replay has only valid entries,
/// and an invalid one means the input log is not a replay the rating can be computed from. `n` echoes
/// the input length and `provisional` reflects it, so the refusal is visible rather than silent.
pub fn essence_cal(predictions: &[(f64, u32)]) -> EsseCal {
    let n = predictions.len();
    let provisional = n < PROVISIONAL_N;

    // A refusal: every computed field NaN, the curve all empty bins, n echoed, provisional set.
    let refusal = EsseCal {
        n,
        mean_log_score: f64::NAN,
        brier: f64::NAN,
        bias: f64::NAN,
        bins: empty_bins(),
        provisional,
        ci95_lo: f64::NAN,
        ci95_hi: f64::NAN,
    };
    if n == 0 {
        return refusal;
    }

    // Validate every prediction before computing anything: one poisoned entry refuses the rating.
    for &(p, occ) in predictions {
        if !is_finite(p) || !(0.0..=1.0).contains(&p) || (occ != 0 && occ != 1) {
            return refusal;
        }
    }

    // Per-bin accumulators for the reliability curve. Indexed by bin, deterministic, no hash order.
    let mut bin_count = [0usize; BIN_COUNT];
    let mut bin_sum_p = [0.0f64; BIN_COUNT];
    let mut bin_sum_occ = [0.0f64; BIN_COUNT];

    let mut sum_log = 0.0f64;
    let mut sum_log_sq = 0.0f64;
    let mut sum_p = 0.0f64;
    let mut sum_occ = 0.0f64;
    let mut sum_sq_err = 0.0f64;

    for &(p, occ) in predictions {
        let occ_f = if occ == 1 { 1.0 } else { 0.0 };
        // The per-mission score, re-derived through the same proper rule — never an accuracy %.
        let score = log_score(p, occ);
        sum_log += score;
        sum_log_sq += score * score;
        sum_p += p;
        sum_occ += occ_f;
        sum_sq_err += (p - occ_f) * (p - occ_f);

        // Bin the predicted probability. `p ∈ [0,1]`, so `p * BIN_COUNT ∈ [0, BIN_COUNT]`; the cast
        // truncates toward zero (floor for non-negative values) and the clamp folds `p = 1.0` (which
        // lands exactly on `BIN_COUNT`) back into the last bin. No `floor`/`min` method — those are
        // host float methods (R1) and `min` is a method the lint flags.
        let mut idx = (p * BIN_COUNT as f64) as usize;
        if idx > BIN_COUNT - 1 {
            idx = BIN_COUNT - 1;
        }
        bin_count[idx] += 1;
        bin_sum_p[idx] += p;
        bin_sum_occ[idx] += occ_f;
    }

    let mean_log = sum_log / n as f64;
    let brier = sum_sq_err / n as f64;
    let bias = sum_p / n as f64 - sum_occ / n as f64;

    // The reliability curve: each bin's mean predicted probability against its observed frequency.
    let mut bins = empty_bins();
    for i in 0..BIN_COUNT {
        bins[i].center = (2 * i + 1) as f64 / (2.0 * BIN_COUNT as f64);
        bins[i].count = bin_count[i];
        if bin_count[i] > 0 {
            bins[i].predicted = bin_sum_p[i] / bin_count[i] as f64;
            bins[i].observed = bin_sum_occ[i] / bin_count[i] as f64;
        }
    }

    // The 95% interval on the mean log score: `mean ± z * σ/√N`, with σ the sample standard deviation
    // (n-1 denominator). For N = 1 there is no sample variance, so the interval collapses to the mean
    // itself — a single prediction has no spread to bound. `fmax` guards the sqrt against a tiny
    // negative variance from rounding at the endpoints.
    let (ci_lo, ci_hi) = if n > 1 {
        let var = (sum_log_sq - sum_log * sum_log / n as f64) / (n as f64 - 1.0);
        let std = sqrt(fmax(var, 0.0));
        let se = std / sqrt(fmax(n as f64, f64::MIN_POSITIVE));
        let margin = CI95_Z * se;
        (mean_log - margin, mean_log + margin)
    } else {
        (mean_log, mean_log)
    };

    EsseCal {
        n,
        mean_log_score: mean_log,
        brier,
        bias,
        bins,
        provisional,
        ci95_lo: ci_lo,
        ci95_hi: ci_hi,
    }
}

/// The empty reliability curve: every bin present, with `NaN` predicted/observed and a zero count.
fn empty_bins() -> [ReliabilityBin; BIN_COUNT] {
    [ReliabilityBin {
        center: 0.0,
        predicted: f64::NAN,
        observed: f64::NAN,
        count: 0,
    }; BIN_COUNT]
}

/// The deterministic design hash of the Esse aggregation's constants.
///
/// Pins the bin count, the provisional threshold, and the interval's critical value, so a replay
/// bundle or an Akashi entry can detect a stale aggregation kernel — the same way
/// [`crate::mission::mission_design_hash`] pins the economy. A pure function: the same kernel
/// produces the same hash on every platform, every run. Folding in every pinned constant means
/// changing any of them changes the hash, which is the point: the aggregation is a documented model,
/// and the hash is how a verifier knows which model produced a rating.
pub fn essence_design_hash() -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut mix = |v: f64| {
        let bits = v.to_bits();
        hash ^= bits;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    };
    mix(BIN_COUNT as f64);
    mix(PROVISIONAL_N as f64);
    mix(CI95_Z);
    hash
}
