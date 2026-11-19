//! Contract grading, ported from `evaluate` and `scoreFlight` in
//! `src/core/mission.ts`.
//!
//! The composite is returned as a 0..1 fraction; the caller rounds it to the
//! 0..1000 score so rounding semantics stay in one place.

fn clamp01(v: f64) -> f64 {
    if v < 0.0 {
        0.0
    } else if v > 1.0 {
        1.0
    } else {
        v
    }
}

/// Apsis accuracy in 0..1. Both errors are signed metres.
pub fn accuracy(pe_error: f64, ap_error: f64, tolerance: f64) -> f64 {
    clamp01(1.0 - (pe_error.abs() + ap_error.abs()) / (4.0 * tolerance))
}

/// Completion progress in 0..1, used by the HUD ring.
pub fn progress(pe_error: f64, ap_error: f64, tolerance: f64) -> f64 {
    clamp01(1.0 - (pe_error.abs() + ap_error.abs()) / (tolerance * 12.0).max(1.0))
}

/// 0.62 accuracy, 0.24 propellant reserve, 0.14 time in window.
pub fn composite(pe_error: f64, ap_error: f64, tolerance: f64, fuel_left: f64, time_frac: f64) -> f64 {
    let acc = accuracy(pe_error, ap_error, tolerance);
    0.62 * acc + 0.24 * clamp01(fuel_left) + 0.14 * clamp01(time_frac)
}

pub fn grade(score: f64) -> &'static str {
    if score >= 950.0 {
        "S"
    } else if score >= 850.0 {
        "A"
    } else if score >= 700.0 {
        "B"
    } else if score >= 550.0 {
        "C"
    } else {
        "D"
    }
}

pub fn in_tolerance(pe_error: f64, ap_error: f64, inc_error: f64, tolerance: f64) -> bool {
    pe_error.abs() <= tolerance
        && ap_error.abs() <= tolerance
        && inc_error < 3.0_f64.to_radians()
}
