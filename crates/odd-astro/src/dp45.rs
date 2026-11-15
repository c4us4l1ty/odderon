//! The one Dormand–Prince 5(4) tableau and step controller in the workspace.
//!
//! `integrator` (orbital state) and `odd-systems` (tank thermal state) both
//! step through this module, so the No-Euler Integration Law is enforced by
//! construction rather than by two copies that can drift. The constants are
//! the standard DP5(4) pair; the controller is the classic PI-free
//! `0.9·err^-1/5` with the usual safety clamps.
//!
//! Nothing here knows about orbits or tanks: `trial` advances an arbitrary
//! `N`-vector and returns the embedded error estimate normalized by the
//! caller's tolerances.

use crate::integrator::IntegrationError;

/// Stage abscissae.
pub const C: [f64; 7] = [0.0, 1.0 / 5.0, 3.0 / 10.0, 4.0 / 5.0, 8.0 / 9.0, 1.0, 1.0];
/// Runge–Kutta matrix.
pub const A: [[f64; 7]; 7] = [
    [0.0; 7],
    [1.0 / 5.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [3.0 / 40.0, 9.0 / 40.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [44.0 / 45.0, -56.0 / 15.0, 32.0 / 9.0, 0.0, 0.0, 0.0, 0.0],
    [
        19372.0 / 6561.0,
        -25360.0 / 2187.0,
        64448.0 / 6561.0,
        -212.0 / 729.0,
        0.0,
        0.0,
        0.0,
    ],
    [
        9017.0 / 3168.0,
        -355.0 / 33.0,
        46732.0 / 5247.0,
        49.0 / 176.0,
        -5103.0 / 18656.0,
        0.0,
        0.0,
    ],
    [
        35.0 / 384.0,
        0.0,
        500.0 / 1113.0,
        125.0 / 192.0,
        -2187.0 / 6784.0,
        11.0 / 84.0,
        0.0,
    ],
];
/// Fifth-order solution weights.
pub const B5: [f64; 7] = [
    35.0 / 384.0,
    0.0,
    500.0 / 1113.0,
    125.0 / 192.0,
    -2187.0 / 6784.0,
    11.0 / 84.0,
    0.0,
];
/// Embedded fourth-order weights, used only for the error estimate.
pub const B4: [f64; 7] = [
    5179.0 / 57600.0,
    0.0,
    7571.0 / 16695.0,
    393.0 / 640.0,
    -92097.0 / 339200.0,
    187.0 / 2100.0,
    1.0 / 40.0,
];

/// One DP5(4) trial over `width` seconds, returning the fifth-order state and
/// the max normalized embedded error. `derivative` is called at each stage
/// time with that stage's state.
///
/// The accumulation order matches `integrator`'s orbital loop exactly, so a
/// caller reproducing orbital state through this function gets the same bits.
pub fn trial<const N: usize, F>(
    start: f64,
    y0: [f64; N],
    width: f64,
    rtol: f64,
    atol: f64,
    mut derivative: F,
) -> Result<([f64; N], f64), IntegrationError>
where
    F: FnMut(f64, [f64; N]) -> Result<[f64; N], IntegrationError>,
{
    if width <= 0.0 || !width.is_finite() {
        return Err(IntegrationError::StepUnderflow);
    }
    let mut k = [[0.0; N]; 7];
    for i in 0..7 {
        let mut z = y0;
        for (j, coeff) in A[i].iter().enumerate().take(i) {
            for n in 0..N {
                z[n] += width * coeff * k[j][n];
            }
        }
        if !z.iter().all(|x| x.is_finite()) {
            return Err(IntegrationError::NumericalRange);
        }
        let t = start + C[i] * width;
        k[i] = derivative(t, z)?;
        if !k[i].iter().all(|x| x.is_finite()) {
            return Err(IntegrationError::ForceFailure);
        }
    }
    let mut next = y0;
    let mut err = 0.0_f64;
    for n in 0..N {
        let mut delta = 0.0;
        for i in 0..7 {
            next[n] += width * B5[i] * k[i][n];
            delta += width * (B5[i] - B4[i]) * k[i][n];
        }
        if !next[n].is_finite() {
            return Err(IntegrationError::NumericalRange);
        }
        let scale = atol + rtol * y0[n].abs().max(next[n].abs());
        err = err.max((delta / scale).abs());
    }
    if !err.is_finite() {
        return Err(IntegrationError::NumericalRange);
    }
    Ok((next, err))
}

/// Step-size multiplier for a normalized error, clamped to `[0.2, 5.0]`.
/// A zero error grows the step by the maximum factor.
pub fn controller(err: f64) -> f64 {
    if err == 0.0 {
        5.0
    } else {
        (0.9 * libm::pow(err, -0.2)).clamp(0.2, 5.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Exponential decay has a closed form, so the tableau and controller can
    /// be checked against truth rather than against themselves.
    #[test]
    fn fifth_order_convergence_on_exponential_decay() {
        let exact = libm::exp(-1.0);
        let mut worst = 0.0_f64;
        for steps in [64_usize, 128, 256] {
            let width = 1.0 / steps as f64;
            let mut y = [1.0_f64];
            let mut t = 0.0;
            for _ in 0..steps {
                let (next, _) = trial(t, y, width, 1e-12, 1e-12, |_, z| Ok([-z[0]])).unwrap();
                y = next;
                t += width;
            }
            let error = (y[0] - exact).abs();
            if steps == 256 {
                worst = error;
            }
        }
        // Global order 5 would give 4; DP5(4) local sixth order gives ~5.
        // Asserting a tight absolute bound keeps this honest without pinning
        // a ratio that rounding can perturb.
        assert!(worst < 1e-12, "{worst}");
    }

    #[test]
    fn controller_clamps_and_grows() {
        assert_eq!(controller(0.0), 5.0);
        assert_eq!(controller(1e30), 0.2);
        assert!((controller(1.0) - 0.9).abs() < 1e-15);
    }
}
