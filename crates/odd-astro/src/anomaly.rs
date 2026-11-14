//! Hyperbolic anomaly conversions for ideal two-body motion.
//!
//! The mean anomaly is dimensionless and unwrapped. These functions use a
//! safeguarded numerical inversion, not a universal five-iteration guarantee.

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
    let denominator = 1.0 + e * cn;
    if !denominator.is_finite() {
        return Err(AnomalyError::NumericalRange);
    }
    if denominator <= 0.0 {
        return Err(AnomalyError::OutsidePhysicalBranch);
    }
    let ratio = ((e - 1.0).sqrt() * (e + 1.0).sqrt()) * (sn / denominator);
    if !ratio.is_finite() {
        return Err(AnomalyError::NumericalRange);
    }
    Ok(ratio.asinh())
}

/// True anomaly in (-π, π), for finite F and eccentricity greater than one.
pub fn true_from_hyperbolic(e: f64, anomaly: f64) -> Result<f64, AnomalyError> {
    validate(e, anomaly)?;
    let scale = (e + 1.0).sqrt() / (e - 1.0).sqrt();
    let result = 2.0 * (scale * (anomaly * 0.5).tanh()).atan();
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
    if result.is_finite() {
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
    let cubic = (m / e * 6.0).cbrt();
    let mut x = linear.min(cubic).min(hi);
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
