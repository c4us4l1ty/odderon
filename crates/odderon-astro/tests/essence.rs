//! The Esse career ledger, as measurable claims.
//!
//! `docs/BUILD-PLAN.md` §5 names P5's job: the career core — the mission loop (`GAME-DESIGN-BIBLE.md`
//! §3.2), the economy (§12.1), and the Esse rating (§4.3). [`odderon_astro::mission`] scores a single
//! prediction; this module aggregates a career's predictions into the `E_cal` rating. The claims, each
//! falsifiable, grouped by which part they can falsify:
//!
//! 1. **Esse is a reliability curve, not an average (§4.3 rule 1).** The rating carries a binned
//!    curve of predicted probability against observed frequency, with N, a 95% interval, and the
//!    §2.2(a) bias. An average cannot distinguish a calibrated forecaster from a lucky one; the curve
//!    can. This is the claim the per-mission running mean in Block 8 could not make.
//! 2. **The aggregate is proper.** The mean log score is re-derived through the same proper rule the
//!    per-mission score uses (`mission::log_score`), never an accuracy percentage. Cross-checked
//!    against the platform's own `ln` so a sign or branch error in the kernel is caught.
//! 3. **The curve is the diagonal for a calibrated forecaster.** When the predicted probability equals
//!    the observed frequency in every bin, the curve sits on the diagonal and the bias is zero. A
//!    biased forecaster is off the diagonal, with a signed bias.
//! 4. **Provisional until N ≥ 30 (§4.3).** The rating is flagged as a claim below the threshold and a
//!    measurement at or above it.
//! 5. **The refusals.** An empty ledger, or any poisoned prediction, refuses the whole rating with
//!    `NaN` — never a fabricated number.
//! 6. **Determinism and recomputability.** The same predictions give the same rating every call, and
//!    the design hash pins the aggregation's constants.
//!
//! Every number quoted in a comment was computed against the same constants before being written
//! down, so a failure says which of the two disagreed.

use odderon_astro::essence::{essence_cal, essence_design_hash, BIN_COUNT, CI95_Z, PROVISIONAL_N};

/// The platform's own log score, for cross-checking the kernel's aggregate against an independent
/// computation. Lower is better; `-ln(p)` if the outcome occurred, `-ln(1-p)` if it did not.
fn platform_log_score(p: f64, occurred: u32) -> f64 {
    let target = if occurred == 1 { p } else { 1.0 - p };
    -target.ln()
}

/// The expected scalar summary, computed independently on the platform. Returns
/// `(mean_log, brier, bias)`.
fn expected_summary(preds: &[(f64, u32)]) -> (f64, f64, f64) {
    let n = preds.len() as f64;
    let mut sum_log = 0.0;
    let mut sum_sq = 0.0;
    let mut sum_p = 0.0;
    let mut sum_occ = 0.0;
    for &(p, occ) in preds {
        let occ_f = if occ == 1 { 1.0 } else { 0.0 };
        sum_log += platform_log_score(p, occ);
        sum_sq += (p - occ_f) * (p - occ_f);
        sum_p += p;
        sum_occ += occ_f;
    }
    (sum_log / n, sum_sq / n, sum_p / n - sum_occ / n)
}

// ---------------------------------------------------------------------------------------------
// 1. The refusals
// ---------------------------------------------------------------------------------------------

#[test]
fn empty_ledger_is_a_refusal() {
    let cal = essence_cal(&[]);
    assert_eq!(cal.n, 0);
    assert!(cal.mean_log_score.is_nan(), "an empty ledger has no mean log score");
    assert!(cal.brier.is_nan());
    assert!(cal.bias.is_nan());
    assert!(cal.ci95_lo.is_nan() && cal.ci95_hi.is_nan());
    assert!(cal.provisional, "an empty ledger is provisional");
    // Every bin is an empty hole, not a zero that reads as a measurement.
    for b in cal.bins {
        assert_eq!(b.count, 0);
        assert!(b.predicted.is_nan());
        assert!(b.observed.is_nan());
    }
}

#[test]
fn a_poisoned_prediction_refuses_the_whole_rating() {
    // One valid prediction and one with p outside [0,1]: the rating is refused, not averaged over
    // the valid half. A rating with a poisoned entry is not a rating.
    let preds = [(0.8, 1), (1.5, 1)];
    let cal = essence_cal(&preds);
    assert_eq!(cal.n, 2, "n echoes the input length so the refusal is visible");
    assert!(cal.mean_log_score.is_nan(), "a poisoned entry refuses the mean");
    assert!(cal.brier.is_nan());
    assert!(cal.bias.is_nan());
    assert!(cal.provisional, "n=2 is below the provisional threshold");
}

#[test]
fn a_non_binary_outcome_refuses_the_rating() {
    let preds = [(0.8, 1), (0.7, 2)];
    let cal = essence_cal(&preds);
    assert!(cal.mean_log_score.is_nan(), "occurred must be 0 or 1");
}

#[test]
fn a_non_finite_probability_refuses_the_rating() {
    let preds = [(f64::NAN, 1), (0.8, 1)];
    let cal = essence_cal(&preds);
    assert!(cal.mean_log_score.is_nan());
}

// ---------------------------------------------------------------------------------------------
// 2. The aggregate is proper and matches an independent computation
// ---------------------------------------------------------------------------------------------

#[test]
fn the_mean_brier_and_bias_match_an_independent_computation() {
    // A mixed, realistic career: a spread of probabilities and outcomes, computed against the
    // platform's own ln so a kernel transcendental error is caught, not just the shape.
    let preds: [(f64, u32); 8] = [
        (0.9, 1), (0.8, 1), (0.7, 0), (0.6, 1),
        (0.5, 0), (0.4, 0), (0.3, 1), (0.2, 0),
    ];
    let cal = essence_cal(&preds);
    let (mean, brier, bias) = expected_summary(&preds);
    assert_eq!(cal.n, 8);
    assert!((cal.mean_log_score - mean).abs() < 1e-12, "mean log score");
    assert!((cal.brier - brier).abs() < 1e-12, "brier score");
    assert!((cal.bias - bias).abs() < 1e-12, "bias");
    assert!(cal.provisional, "n=8 is below the threshold, so it is provisional");
}

#[test]
fn a_single_prediction_has_a_degenerate_interval() {
    let cal = essence_cal(&[(0.8, 1)]);
    assert_eq!(cal.n, 1);
    assert!((cal.mean_log_score - (-0.8f64.ln())).abs() < 1e-12);
    // One prediction has no sample spread, so the 95% interval collapses to the mean itself.
    assert!((cal.ci95_lo - cal.mean_log_score).abs() < 1e-12);
    assert!((cal.ci95_hi - cal.mean_log_score).abs() < 1e-12);
    assert!(cal.provisional, "n=1 is provisional");
}

#[test]
fn the_interval_brackets_the_mean_and_widens_with_variance() {
    // A tight, confident, mostly-correct set: a narrow interval.
    let tight = essence_cal(&[(0.95, 1), (0.9, 1), (0.85, 1), (0.95, 1)]);
    assert!(tight.ci95_lo <= tight.mean_log_score);
    assert!(tight.ci95_hi >= tight.mean_log_score);
    // A wide, uncertain set: the log scores spread far apart (a near-certain correct call scores
    // near 0, a coin-flip scores ~0.69), so the interval widens for the same N. Note the log score
    // is symmetric -- -ln(p) correct equals -ln(1-p) wrong -- so the spread must come from differing
    // p, not from mixing correct and wrong at the same p.
    let wide = essence_cal(&[(0.99, 1), (0.5, 0), (0.5, 1), (0.01, 0)]);
    let tight_width = tight.ci95_hi - tight.ci95_lo;
    let wide_width = wide.ci95_hi - wide.ci95_lo;
    assert!(
        wide_width > tight_width,
        "more variance must widen the interval (tight {tight_width}, wide {wide_width})"
    );
    // The interval is mean ± z * standard error, so it is centred on the mean.
    let mid = (wide.ci95_lo + wide.ci95_hi) / 2.0;
    assert!((mid - wide.mean_log_score).abs() < 1e-9, "the interval is centred on the mean");
    assert!((CI95_Z - 1.96).abs() < 1e-12);
}

// ---------------------------------------------------------------------------------------------
// 3. The curve is the diagonal for a calibrated forecaster, off it for a biased one
// ---------------------------------------------------------------------------------------------

/// A calibrated career: in each bin the predicted probability equals the observed frequency, so the
/// curve sits exactly on the diagonal and the bias is zero. 20 predictions per bin at the bin's
/// centre, with `(2i+1)` of them occurring — so observed = `(2i+1)/20` = centre = predicted.
fn calibrated_career() -> Vec<(f64, u32)> {
    let mut preds = Vec::new();
    for i in 0..BIN_COUNT {
        let c = (2 * i + 1) as f64 / (2.0 * BIN_COUNT as f64);
        let occur = 2 * i + 1; // the number that occur, so observed = occur/20 = c
        for _ in 0..occur {
            preds.push((c, 1));
        }
        for _ in 0..(20 - occur) {
            preds.push((c, 0));
        }
    }
    preds
}

#[test]
fn a_calibrated_forecaster_sits_on_the_diagonal() {
    let preds = calibrated_career();
    let cal = essence_cal(&preds);
    assert_eq!(cal.n, BIN_COUNT * 20);
    assert!(!cal.provisional, "n=200 is a measurement, not a claim");
    assert!(cal.bias.abs() < 1e-12, "a calibrated forecaster has zero bias, got {}", cal.bias);
    for b in cal.bins {
        assert_eq!(b.count, 20, "each bin holds its 20 predictions");
        assert!(
            (b.predicted - b.observed).abs() < 1e-12,
            "bin at {} is off the diagonal: predicted {} observed {}",
            b.center, b.predicted, b.observed
        );
        assert!(
            (b.predicted - b.center).abs() < 1e-12,
            "the calibrated forecaster's predicted probability equals the bin centre"
        );
    }
    // The mean log score matches an independent computation over the same calibrated career.
    let (mean, brier, bias) = expected_summary(&preds);
    assert!((cal.mean_log_score - mean).abs() < 1e-9);
    assert!((cal.brier - brier).abs() < 1e-9);
    assert!((cal.bias - bias).abs() < 1e-9);
}

#[test]
fn a_biased_forecaster_is_off_the_diagonal_with_a_signed_bias() {
    // Ten predictions all at p = 0.9, five of which occur: the forecaster over-predicts the outcome.
    let mut preds = vec![(0.9, 1); 5];
    preds.extend(std::iter::repeat((0.9, 0)).take(5));
    let cal = essence_cal(&preds);
    assert_eq!(cal.n, 10);
    // predicted 0.9, observed 0.5: bias = +0.4 (over-predicts).
    assert!((cal.bias - 0.4).abs() < 1e-12, "bias should be +0.4, got {}", cal.bias);
    // All ten land in the top bin (p=0.9 → bin 9); its observed frequency is 0.5, far below the
    // predicted 0.9 — off the diagonal, below it.
    let top = cal.bins[BIN_COUNT - 1];
    assert_eq!(top.count, 10);
    assert!((top.predicted - 0.9).abs() < 1e-12);
    assert!((top.observed - 0.5).abs() < 1e-12);
    assert!(top.observed < top.predicted, "the biased curve sits below the diagonal");
}

#[test]
fn the_curve_is_not_a_single_average() {
    // The §4.3 claim in one assertion: the rating carries a binned curve, not one scalar. Two
    // forecasters with the same N but different calibration produce different curves — which a mean
    // alone could not distinguish if their means happened to coincide. Here the means differ too, but
    // the point is that the curve is present and binned: ten centres, each with its own
    // predicted/observed pair.
    let preds = calibrated_career();
    let cal = essence_cal(&preds);
    let centers: Vec<f64> = cal.bins.iter().map(|b| b.center).collect();
    assert_eq!(centers.len(), BIN_COUNT);
    // The centres are evenly spaced over [0,1]: the (2i+1)/(2*BIN_COUNT) formula.
    for (i, &c) in centers.iter().enumerate() {
        let expected = (2 * i + 1) as f64 / (2.0 * BIN_COUNT as f64);
        assert!((c - expected).abs() < 1e-12, "bin {} centre", i);
    }
    assert!(centers[0] > 0.0 && centers[BIN_COUNT - 1] < 1.0, "centres span the open interval");
}

// ---------------------------------------------------------------------------------------------
// 4. The provisional threshold
// ---------------------------------------------------------------------------------------------

#[test]
fn provisional_until_thirty() {
    assert_eq!(PROVISIONAL_N, 30);
    let below = essence_cal(&[(0.8, 1); PROVISIONAL_N - 1]);
    assert!(below.provisional, "n=29 is provisional");
    let at = essence_cal(&[(0.8, 1); PROVISIONAL_N]);
    assert!(!at.provisional, "n=30 is a measurement");
    assert_eq!(at.n, PROVISIONAL_N);
}

// ---------------------------------------------------------------------------------------------
// 5. Determinism and the design hash
// ---------------------------------------------------------------------------------------------

#[test]
fn the_rating_is_deterministic() {
    let preds = calibrated_career();
    let a = essence_cal(&preds);
    let b = essence_cal(&preds);
    assert_eq!(a.n, b.n);
    assert_eq!(a.mean_log_score, b.mean_log_score);
    assert_eq!(a.brier, b.brier);
    assert_eq!(a.bias, b.bias);
    assert_eq!(a.provisional, b.provisional);
    for (ba, bb) in a.bins.iter().zip(b.bins.iter()) {
        assert_eq!(ba.center, bb.center);
        assert_eq!(ba.predicted, bb.predicted);
        assert_eq!(ba.observed, bb.observed);
        assert_eq!(ba.count, bb.count);
    }
}

#[test]
fn the_design_hash_is_stable_and_distinct_from_the_economy_hash() {
    let h = essence_design_hash();
    assert_eq!(h, essence_design_hash(), "the design hash is deterministic");
    // A different constant set must produce a different hash. The economy's hash folds in the price
    // constants; the essence hash folds in the aggregation constants. They are different models, so
    // their hashes differ — a verifier can tell which produced a rating.
    assert_ne!(
        h,
        odderon_astro::mission::mission_design_hash(),
        "the essence and economy design hashes must differ"
    );
}
