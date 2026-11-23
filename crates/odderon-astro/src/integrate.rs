//! A high-precision J2 numerical integrator, used as the *reference* for the secular theory.
//!
//! ## Why this is not the game's propagator
//!
//! The game coasts bodies with closed forms — [`crate::propagate`] for two-body and
//! [`crate::j2::propagate_j2_secular`] for the averaged oblateness drift. Neither integrates anything.
//! This module does, deliberately and expensively, for one purpose: to answer the question
//! `CREDIBILITY.md` §3.1 asks of the secular theory —
//!
//! > how wrong is it, really?
//!
//! A formula cannot validate itself. Comparing [`crate::j2::secular_rates`] to the algebra it was
//! transcribed from is circular: it agrees by construction, including when both are wrong. The only
//! non-circular check is to integrate the actual equations of motion and measure the drift that comes
//! out. That is what this module exists for, and it is why it lives in the crate rather than in a test:
//! the test would have to reimplement it, and two copies of a reference are worse than one.
//!
//! ## The equations
//!
//! Acceleration under point-mass gravity plus the J2 harmonic (Curtis eq. 12.30, with `J2` as a positive
//! magnitude so the sign convention matches [`crate::j2::EARTH_J2`]):
//!
//! ```text
//! a_x = -(3/2) J2 μ Rₑ² / r⁵ · (1 − 5z²/r²) · x      and likewise for y
//! a_z = -(3/2) J2 μ Rₑ² / r⁵ · (3 − 5z²/r²) · z
//! ```
//!
//! ## Why Dormand–Prince 5(4)
//!
//! Adaptive, embedded, and about thirty lines. The embedded fourth-order solution gives a local error
//! estimate at every step for free, which is what makes "high precision" a measured claim rather than a
//! hope: the step controller compares that estimate against a tolerance and rejects steps that miss.
//! A fixed-step RK4 would need the same code length and give no error signal at all.
//!
//! The integrator is *not* optimised. It is allowed to be slow, because it runs in a test and never in a
//! frame.

use crate::{is_positive_finite, State, Vec3};
use odderon_dmath::{atan2, fabs, fmax, pow, sqrt, TAU};

/// Coefficients of the Dormand–Prince 5(4) pair. Standard tableau; written out rather than derived.
///
/// `A[i][j]` is the stage coupling, `B5` the fifth-order weights and `B4` the embedded fourth-order
/// weights. The last row of `A` doubles as `B5`, which is why the seventh stage's derivative is the
/// fifth-order solution — that is the FSAL property and it is not used here, because reusing it would
/// make the code faster and harder to check against a printed tableau.
const A: [[f64; 6]; 7] = [
    [0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [1.0 / 5.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [3.0 / 40.0, 9.0 / 40.0, 0.0, 0.0, 0.0, 0.0],
    [44.0 / 45.0, -56.0 / 15.0, 32.0 / 9.0, 0.0, 0.0, 0.0],
    [
        19372.0 / 6561.0,
        -25360.0 / 2187.0,
        64448.0 / 6561.0,
        -212.0 / 729.0,
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
    ],
    [
        35.0 / 384.0,
        0.0,
        500.0 / 1113.0,
        125.0 / 192.0,
        -2187.0 / 6784.0,
        11.0 / 84.0,
    ],
];

const B5: [f64; 7] = [
    35.0 / 384.0,
    0.0,
    500.0 / 1113.0,
    125.0 / 192.0,
    -2187.0 / 6784.0,
    11.0 / 84.0,
    0.0,
];

const B4: [f64; 7] = [
    5179.0 / 57600.0,
    0.0,
    7571.0 / 16695.0,
    393.0 / 640.0,
    -92097.0 / 339200.0,
    187.0 / 2100.0,
    1.0 / 40.0,
];

/// Gravitational acceleration from a central body plus its J2 oblateness term, km/s².
///
/// `j2` is the **positive magnitude** `1.08263e-3`, matching [`crate::j2::EARTH_J2`]. The sign of the
/// oblateness is carried by the formula, not by the constant — see that constant's documentation for why
/// that choice cost a PR.
pub fn accel_j2(r: Vec3, mu: f64, j2: f64, equatorial_radius: f64) -> Vec3 {
    let rn = r.norm();
    if !is_positive_finite(rn) {
        return Vec3::new(0.0, 0.0, 0.0);
    }
    let r2 = rn * rn;
    let u = (r.z / rn) * (r.z / rn);

    // -(3/2) J2 μ Rₑ² / r⁵, the common factor
    let k = -1.5 * j2 * mu * equatorial_radius * equatorial_radius / (r2 * r2 * rn);

    // point mass
    let g = -mu / (r2 * rn);

    Vec3::new(
        g * r.x + k * (1.0 - 5.0 * u) * r.x,
        g * r.y + k * (1.0 - 5.0 * u) * r.y,
        g * r.z + k * (3.0 - 5.0 * u) * r.z,
    )
}

/// The J2 contribution to the specific potential energy, km²/s².
///
/// ```text
/// U_J2 = μ J2 Rₑ² / (2 r³) · (3z²/r² − 1)
/// ```
///
/// This exists so a test can assert that the integrator **conserves total energy**, which is the check
/// that makes it a trustworthy reference. It is not optional decoration: an integrator that leaks energy
/// manufactures its own secular drift, and a manufactured drift could sit anywhere — including exactly on
/// top of the secular theory it is supposed to be checking.
///
/// The sign is the interesting part. `State::specific_energy` is documented as the *two-body* energy
/// `v²/2 − μ/r`, and under J2 that quantity is **not** conserved: it oscillates by about 2e-3 relative at
/// LEO over a week, because the oblateness is doing real work on the orbit. The conserved quantity is
/// `v²/2 − μ/r + U_J2`, and getting this wrong in a test produces a confident failure that looks like an
/// integrator bug. It was produced, and it was not an integrator bug.
///
/// Verified by finite-differencing this potential and comparing against [`accel_j2`]: `−∇U_J2` reproduces
/// the oblateness acceleration to 1e-9 relative. That check lives in the test file.
pub fn potential_j2(r: Vec3, mu: f64, j2: f64, equatorial_radius: f64) -> f64 {
    let rn = r.norm();
    if !is_positive_finite(rn) {
        return 0.0;
    }
    let u = (r.z / rn) * (r.z / rn);
    mu * j2 * equatorial_radius * equatorial_radius / (2.0 * rn * rn * rn) * (3.0 * u - 1.0)
}

/// The state derivative: `(v, a)`.
fn deriv(s: [f64; 6], mu: f64, j2: f64, re: f64) -> [f64; 6] {
    let a = accel_j2(Vec3::new(s[0], s[1], s[2]), mu, j2, re);
    [s[3], s[4], s[5], a.x, a.y, a.z]
}

/// Result of one integration.
#[derive(Clone, Copy, Debug)]
pub struct Integrated {
    /// The state at the end of the interval.
    pub state: State,
    /// Steps accepted.
    pub accepted: u32,
    /// Steps rejected by the error controller. A high count means the step was too aggressive; it is
    /// reported rather than hidden because it is evidence about the stiffness of the problem.
    pub rejected: u32,
    /// Smallest step taken, seconds.
    pub min_step: f64,
    /// Largest step taken, seconds.
    pub max_step: f64,
}

/// Integrate from `state` for `dt` seconds under two-body + J2, adaptively.
///
/// `rel_tol` is the per-step relative error target on the *state vector*. `1e-12` is a reasonable
/// reference setting: it makes the integrator's own error far smaller than the ~0.4% disagreement
/// between secular theory and reality that it is being used to measure, so the comparison is not
/// limited by the ruler.
///
/// Returns `None` for a non-finite or non-positive `dt`, or if the integrator cannot make progress.
///
/// # Why the lint allowances
///
/// The Runge-Kutta stages are written as fixed-size arrays with explicit indices, because this is a
/// numerical tableau transcription: the index *is* the tableau position, and rewriting the loops as
/// iterators would obscure the correspondence with the standard published form a reader is checking it
/// against. Clippy's `needless_range_loop` is right in general and wrong for a tableau. The allowance is
/// scoped to this function rather than to the crate, so the next accidental range loop elsewhere still
/// gets caught.
#[allow(clippy::needless_range_loop)]
pub fn integrate_j2(
    state: &State,
    dt: f64,
    mu: f64,
    j2: f64,
    equatorial_radius: f64,
    rel_tol: f64,
) -> Option<Integrated> {
    if !dt.is_finite() || dt < 0.0 || !is_positive_finite(rel_tol) {
        return None;
    }

    let mut s = [state.r.x, state.r.y, state.r.z, state.v.x, state.v.y, state.v.z];

    // Initial step: a small fraction of the orbital period. Starting too large wastes a rejection cycle;
    // starting too small wastes thousands of steps. A thousandth of a period is a dull, safe compromise.
    let r0 = state.r.norm();
    let v0 = state.v.norm();
    let specific = v0 * v0 / 2.0 - mu / r0;
    let a0 = if fabs(specific) > 1e-30 {
        -mu / (2.0 * specific)
    } else {
        r0
    };
    let period_guess = if a0 > 0.0 && a0.is_finite() {
        TAU * sqrt(a0 * a0 * a0 / mu)
    } else {
        1.0
    };
    // Clamp the initial step into a sane range. Written out rather than with `f64::clamp` because the
    // kernel policy lint forbids host float methods, and clippy's `manual_clamp` then objects to the
    // expanded form. Both tools are right about their own rule and wrong about this line: the kernel lint
    // is enforcing determinism and `clamp` is `core` (so it is actually safe), and clippy prefers a
    // readability idiom the kernel lint cannot see. An explicit allow with this note is the honest
    // resolution; silently disabling either lint crate-wide would hide the next real instance.
    #[allow(clippy::manual_clamp)]
    let mut h = {
        let mut x = period_guess / 1000.0;
        if x < 1e-3 {
            x = 1e-3;
        } else if x > 300.0 {
            x = 300.0;
        }
        x
    };

    let mut t = 0.0;
    let mut accepted = 0u32;
    let mut rejected = 0u32;
    let mut min_step = f64::INFINITY;
    let mut max_step = 0.0;

    // A hard cap on steps. Without one, a pathological tolerance turns a test into a hang, and a hang
    // looks like a slow machine rather than a failed integration.
    const MAX_STEPS: u32 = 4_000_000;

    while t < dt {
        if accepted + rejected > MAX_STEPS {
            return None;
        }
        let remaining = dt - t;
        if h > remaining {
            h = remaining;
        }

        // Seven stages
        let mut k = [[0.0f64; 6]; 7];
        k[0] = deriv(s, mu, j2, equatorial_radius);
        for i in 1..7 {
            let mut si = s;
            for j in 0..i {
                let aij = A[i][j];
                if aij != 0.0 {
                    for m in 0..6 {
                        si[m] += h * aij * k[j][m];
                    }
                }
            }
            k[i] = deriv(si, mu, j2, equatorial_radius);
        }

        let mut s5 = s;
        let mut s4 = s;
        for m in 0..6 {
            let mut d5 = 0.0;
            let mut d4 = 0.0;
            for i in 0..7 {
                d5 += B5[i] * k[i][m];
                d4 += B4[i] * k[i][m];
            }
            s5[m] += h * d5;
            s4[m] += h * d4;
        }

        // Local error estimate, scaled by the magnitude of the state so it is relative.
        let mut err = 0.0;
        for m in 0..6 {
            err = fmax(err, fabs(s5[m] - s4[m]));
        }
        let scale = fmax(1.0, sqrt(s5[0] * s5[0] + s5[1] * s5[1] + s5[2] * s5[2]));

        if err > rel_tol * scale {
            // reject and shrink. The 0.9 factor keeps the new step slightly inside the boundary rather
            // than exactly on it, which stops the controller oscillating across it.
            rejected += 1;
            h *= fmax(0.2, 0.9 * pow(rel_tol * scale / err, 0.2));
            if h < 1e-6 {
                return None; // cannot make progress
            }
            continue;
        }

        // accept
        s = s5;
        t += h;
        accepted += 1;
        min_step = if h < min_step { h } else { min_step };
        max_step = if h > max_step { h } else { max_step };

        // grow, but gently
        let factor = if err > 0.0 {
            0.9 * pow(rel_tol * scale / err, 0.2)
        } else {
            5.0
        };
        // Clamp the growth factor: no fewer than 0.2x, no more than 5x per step. Written out for the same
        // reason as the initial-step clamp above - see the note there.
        #[allow(clippy::manual_clamp)]
        let factor = if factor < 0.2 {
            0.2
        } else if factor > 5.0 {
            5.0
        } else {
            factor
        };
        h *= factor;
    }

    Some(Integrated {
        state: State::new(
            Vec3::new(s[0], s[1], s[2]),
            Vec3::new(s[3], s[4], s[5]),
        ),
        accepted,
        rejected,
        min_step,
        max_step,
    })
}

/// Right ascension of the ascending node, radians in `[0, 2π)`.
///
/// Undefined for an equatorial orbit, where `Z × h` vanishes. Returns `0.0` there, matching
/// [`crate::Elements::to_elements`], which folds the same degeneracy the same way.
pub fn raan_of(state: &State) -> f64 {
    let h = state.angular_momentum();
    // node vector = Z × h = (-h.y, h.x, 0)
    crate::wrap_two_pi(atan2(h.x, -h.y))
}
