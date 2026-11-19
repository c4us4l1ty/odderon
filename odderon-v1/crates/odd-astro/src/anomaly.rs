//! Elliptic and hyperbolic anomaly conversions for ideal two-body motion.
//!
//! The mean anomaly is dimensionless and unwrapped. These functions use a
//! safeguarded numerical inversion, not a universal five-iteration guarantee.
use crate::conic_denominator;
use std::f64::consts::{PI, TAU};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnomalyError {
    InvalidInput,
    OutsidePhysicalBranch,
    NumericalRange,
    NoConvergence,
}

fn validate(e: f64, value: f64) -> Result<(), AnomalyError> {
    if e.is_finite() && e > 1.0 && value.is_finite() {
        Ok(())
    } else {
        Err(AnomalyError::InvalidInput)
    }
}

/// Hyperbolic eccentric anomaly F from true anomaly ν, in radians.
/// Rejects directions at or beyond the hyperbola's asymptotes.
pub fn hyperbolic_from_true(e: f64, true_anomaly_rad: f64) -> Result<f64, AnomalyError> {
    validate(e, true_anomaly_rad)?;
    let (sn, cn) = true_anomaly_rad.sin_cos();
    let denominator = conic_denominator(e, true_anomaly_rad, cn);
    if !denominator.is_finite() {
        return Err(AnomalyError::NumericalRange);
    }
    if denominator <= 0.0 {
        return Err(AnomalyError::OutsidePhysicalBranch);
    }
    let ratio = (((e - 1.0).sqrt() * (e + 1.0).sqrt()) / denominator) * sn;
    if !ratio.is_finite() || (ratio == 0.0 && sn != 0.0) {
        return Err(AnomalyError::NumericalRange);
    }
    Ok(ratio.asinh())
}

/// True anomaly in (-π, π), for finite F and eccentricity greater than one.
pub fn true_from_hyperbolic(e: f64, anomaly: f64) -> Result<f64, AnomalyError> {
    validate(e, anomaly)?;
    let scale = (e + 1.0).sqrt() / (e - 1.0).sqrt();
    // Halving a subnormal F can erase it before scaling. The linear limit
    // is accurate here even for the largest representable scale.
    let result = if anomaly.abs() < 1e-160 {
        scale * anomaly
    } else {
        2.0 * (scale * (anomaly * 0.5).tanh()).atan()
    };
    if result.is_finite() {
        Ok(result)
    } else {
        Err(AnomalyError::NumericalRange)
    }
}

// Avoid cancellation in e*sinh(F)-F for tiny F and e close to one.
fn mean_unchecked(e: f64, f: f64) -> f64 {
    let remainder = if f.abs() < 0.1 {
        let f2 = f * f;
        f * f2 * (1.0 / 6.0 + f2 * (1.0 / 120.0 + f2 * (1.0 / 5040.0 + f2 / 362880.0)))
    } else {
        f.sinh() - f
    };
    (e - 1.0) * f + e * remainder
}

/// Hyperbolic mean anomaly M = e sinh(F) - F.
pub fn hyperbolic_mean(e: f64, anomaly: f64) -> Result<f64, AnomalyError> {
    validate(e, anomaly)?;
    let result = mean_unchecked(e, anomaly);
    if result.is_finite() && (result != 0.0 || anomaly == 0.0) {
        Ok(result)
    } else {
        Err(AnomalyError::NumericalRange)
    }
}

/// Solve M = e sinh(F) - F, using bracketed Newton/bisection. The bracket
/// prevents divergence for high eccentricity and near-parabolic orbits.
pub fn hyperbolic_from_mean(e: f64, mean: f64) -> Result<f64, AnomalyError> {
    validate(e, mean)?;
    if mean == 0.0 {
        return Ok(0.0);
    }
    let m = mean.abs();
    let mut lo = 0.0;
    let mut hi = (m / e).asinh() + 1.0;
    while mean_unchecked(e, hi) < m {
        hi *= 2.0;
        if !hi.is_finite() {
            return Err(AnomalyError::NumericalRange);
        }
    }
    // Both estimates are upper bounds near F=0, and remain finite for small M.
    let linear = m / (e - 1.0);
    // The true solution is no larger than this linear estimate; if it has
    // underflowed, no nonzero result can be represented.
    if linear == 0.0 {
        return Err(AnomalyError::NumericalRange);
    }
    let cubic = (m / e * 6.0).cbrt();
    let mut x = if cubic == 0.0 {
        linear.min(hi)
    } else {
        linear.min(cubic).min(hi)
    };
    if x <= 0.0 || !x.is_finite() {
        x = hi * 0.5;
    }
    for _ in 0..160 {
        let value = mean_unchecked(e, x);
        let residual = value - m;
        if residual.abs() <= m * 2e-14 || (value == m) {
            return Ok(x.copysign(mean));
        }
        if residual > 0.0 {
            hi = x;
        } else {
            lo = x;
        }
        let derivative = (e - 1.0) + 2.0 * e * (x * 0.5).sinh().powi(2);
        let candidate = x - residual / derivative;
        let next = if candidate.is_finite() && candidate > lo && candidate < hi {
            candidate
        } else {
            lo + (hi - lo) * 0.5
        };
        if next == x {
            return Ok(x.copysign(mean));
        }
        x = next;
    }
    Err(AnomalyError::NoConvergence)
}

fn validate_ellipse(e: f64, value: f64) -> Result<(), AnomalyError> {
    if e.is_finite() && (0.0..1.0).contains(&e) && value.is_finite() {
        Ok(())
    } else {
        Err(AnomalyError::InvalidInput)
    }
}

// Preserve small negative angles (rem_euclid would round them toward 2π).
fn principal(value: f64) -> f64 {
    let mut v = value % TAU;
    if v > PI {
        v -= TAU;
    } else if v < -PI {
        v += TAU;
    }
    v
}

fn elliptic_mean_unchecked(e: f64, anomaly: f64) -> f64 {
    let rem = if anomaly.abs() < 0.1 {
        let a2 = anomaly * anomaly;
        anomaly * a2 * (1.0 / 6.0 - a2 * (1.0 / 120.0 - a2 * (1.0 / 5040.0 - a2 / 362880.0)))
    } else {
        anomaly - anomaly.sin()
    };
    (1.0 - e) * anomaly + e * rem
}

/// Elliptic mean anomaly M = E - e sin(E), unwrapped.
pub fn elliptic_mean(e: f64, anomaly: f64) -> Result<f64, AnomalyError> {
    validate_ellipse(e, anomaly)?;
    let result = elliptic_mean_unchecked(e, anomaly);
    if result.is_finite() && (result != 0.0 || anomaly == 0.0) {
        Ok(result)
    } else {
        Err(AnomalyError::NumericalRange)
    }
}

/// Solve the elliptic Kepler equation. Returns the principal eccentric
/// anomaly in [-π, π]; integer revolutions are deliberately not returned.
/// For very large M, the input float may already have lost phase precision.
pub fn elliptic_from_mean(e: f64, mean: f64) -> Result<f64, AnomalyError> {
    validate_ellipse(e, mean)?;
    let m = principal(mean);
    if m == 0.0 {
        return Ok(m);
    }
    let target = m.abs();
    let mut lo = 0.0;
    let mut hi = PI;
    let linear = target / (1.0 - e);
    let cubic = (target * 6.0 / e).cbrt();
    let mut x = linear.min(cubic).min(hi);
    if x <= 0.0 || !x.is_finite() {
        x = target;
    }
    for _ in 0..160 {
        let value = elliptic_mean_unchecked(e, x);
        let residual = value - target;
        if residual.abs() <= target * 2e-14 || value == target {
            return Ok(x.copysign(m));
        }
        if residual > 0.0 {
            hi = x;
        } else {
            lo = x;
        }
        let derivative = (1.0 - e) + 2.0 * e * (x * 0.5).sin().powi(2);
        let candidate = x - residual / derivative;
        let next = if candidate.is_finite() && candidate > lo && candidate < hi {
            candidate
        } else {
            lo + (hi - lo) * 0.5
        };
        if next == x {
            return Ok(x.copysign(m));
        }
        x = next;
    }
    Err(AnomalyError::NoConvergence)
}

// The half-angle forms avoid subtracting cos(E)-e near periapsis when
// eccentricity approaches one. Scale tiny subnormal angles before halving.
fn elliptic_angle(value: f64, numerator: f64, denominator: f64) -> Result<f64, AnomalyError> {
    let v = principal(value);
    let scale = numerator / denominator;
    let result = if v.abs() < 1e-160 {
        scale * v
    } else {
        let (s, c) = (0.5 * v).sin_cos();
        2.0 * (numerator * s).atan2(denominator * c)
    };
    if result.is_finite() && (result != 0.0 || v == 0.0) {
        Ok(result)
    } else {
        Err(AnomalyError::NumericalRange)
    }
}

/// Eccentric anomaly E from true anomaly, in [-π, π].
pub fn elliptic_from_true(e: f64, true_anomaly_rad: f64) -> Result<f64, AnomalyError> {
    validate_ellipse(e, true_anomaly_rad)?;
    elliptic_angle(true_anomaly_rad, (1.0 - e).sqrt(), (1.0 + e).sqrt())
}

/// True anomaly from eccentric anomaly E, in [-π, π].
pub fn true_from_elliptic(e: f64, anomaly: f64) -> Result<f64, AnomalyError> {
    validate_ellipse(e, anomaly)?;
    elliptic_angle(anomaly, (1.0 + e).sqrt(), (1.0 - e).sqrt())
}
