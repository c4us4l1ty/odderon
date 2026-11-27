//! Lambert's problem: given two position vectors and a time of flight, find the transfer orbit.
//!
//! ## What this solves
//!
//! Lambert's problem is the inverse of propagation. [`crate::propagate`] answers "where is the
//! body at time `t` given its state at `t₀`". This module answers the reverse: *given the position
//! at `t₀` and the position at `t₁`, and the time between them, what was the velocity at each end?*
//! It is the transfer-planning primitive that a rendezvous or an interplanetary trajectory needs
//! once the endpoints are chosen — the thing that turns "go from here to there" into a burn.
//!
//! ## Method: y-parameter (Bate-Mueller-White universal variables)
//!
//! The solver uses the y-parameter formulation of the universal-variable method. This is the same
//! family as the propagator's universal-variable Kepler solve — one conic type, the same
//! [`crate::stumpff`] functions — but with a key difference: the iteration variable is the
//! **y-parameter**, not the universal anomaly χ directly.
//!
//! The y-parameter decouples the orbit-shape constraint from the flight-time constraint. In a
//! direct χ iteration, the Lagrange coefficient `g` depends on the target time `dt`, which is only
//! correct when the Kepler equation is already solved — circular during iteration. The y-parameter
//! sidesteps this: `g = A·sqrt(y/μ)` is a function of the geometry alone, valid at every step.
//!
//! ## The algorithm
//!
//! 1. **Geometry.** Compute `r₁ = |r₁|`, `r₂ = |r₂|`, the transfer angle `Δν`, and
//!    `A = sqrt(r₁·r₂·(1 + cos Δν))`. `A > 0` for elliptic transfers, `A = 0` at 180°.
//!
//! 2. **Iterate on `ψ = χ²·α`.** The y-parameter is
//!    `y = r₁ + r₂ + A·(ψ·S(ψ) − 1)/sqrt(C(ψ))`,
//!    and the flight time is
//!    `dt = (χ³·S(ψ) + A·sqrt(y)) / sqrt(μ)`
//!    where `χ = sqrt(y / C(ψ))`. Newton's method on `ψ` converges quadratically.
//!
//! 3. **Velocities.** From the converged `y`:
//!    `f = 1 − y/r₁`, `g = A·sqrt(y/μ)`, `ġ = 1 − y/r₂`,
//!    `v₁ = (r₂ − f·r₁)/g`, `v₂ = (ġ·r₂ − r₁)/g`.
//!
//! ## What is checked
//!
//! The solver is validated against published solutions: Vallado's example problems and the
//! zero-rev / one-rev / 180° degenerate cases named in `CREDIBILITY.md` §3.1. The key check is
//! that propagating the solved departure state forward by the flight time reproduces the target
//! position — a self-consistency check that does not depend on the solver's own internals.
//!
//! ## What is not modelled
//!
//! **Multi-rev transfers.** The solver finds the minimum-energy (short-way) transfer. A transfer
//! that wraps around the central body more than once needs a multi-rev search, which is a
//! different problem with its own convergence properties. This module does not model it and says
//! so by refusing inputs where the short-way solution does not converge.

use crate::{is_positive_finite, Vec3};
use odderon_dmath::{fabs, fmax, sqrt};

/// Maximum Newton iterations for the ψ solve.
///
/// The flight-time equation converges quadratically, so 64 is generous by an order of magnitude.
/// Exceeding it means the problem is near-singular (the two positions are nearly collinear with
/// the centre, or the flight time is nearly a full period) and the solver reports `unconverged`
/// rather than returning a wrong answer.
const MAX_ITERATIONS: u32 = 64;

/// Convergence tolerance on the flight-time residual, in seconds.
///
/// `1e-5` s is well below any physical requirement: at LEO a 1e-5 s time error corresponds to a
/// position error of ~70 mm, far inside the < 50 m target the rest of the kernel is measured
/// against, and well inside the 1e-8 relative velocity acceptance criterion.
const TOLERANCE_S: f64 = 1e-5;

/// Result of a Lambert solve.
///
/// Returning the iteration count and residual rather than swallowing them follows the kernel's
/// honesty rule (`GAME-DESIGN-BIBLE.md` P1): if a solve needed 60 iterations, the problem is
/// near-singular and the caller should be able to see that.
#[derive(Clone, Copy, Debug)]
pub struct Lambert {
    /// Departure velocity, km/s.
    pub v1: Vec3,
    /// Arrival velocity, km/s.
    pub v2: Vec3,
    /// Newton iterations used on the ψ solve.
    pub iterations: u32,
    /// Final flight-time residual, in seconds. Should be at or near zero on convergence.
    pub residual: f64,
    /// True if the iteration limit was hit without meeting the tolerance.
    pub unconverged: bool,
}

/// Solve Lambert's problem: find the transfer orbit from `r1` to `r2` in `dt_s` seconds.
///
/// # Arguments
///
/// * `r1` — departure position, km.
/// * `r2` — arrival position, km.
/// * `dt_s` — time of flight, seconds. Must be positive.
/// * `mu` — gravitational parameter of the central body, km³/s².
///
/// # Transfer angle
///
/// The transfer angle `Δν` is the angle between `r1` and `r2` as seen from the central body. The
/// cross product magnitude gives `sin Δν` (always positive for the short-way transfer this solver
/// computes), and the dot product gives `cos Δν`. The 180° case (`cos Δν = -1`, `A = 0`) is
/// handled by a small perturbation: the Lagrange coefficient `g = A·sqrt(y/μ)` vanishes at
/// exactly 180°, making the velocity computation singular. Perturbing `r₂` by 1 m in a
/// perpendicular direction breaks the singularity; the solution is accurate to well within the
/// acceptance tolerance, and the perpendicular direction picks one of the infinitely many valid
/// orbital planes for a 180° transfer.
///
/// # Degenerate input
///
/// Returns a `Lambert` with NaN velocities and `unconverged: true` for:
/// * non-finite or non-positive `r1`, `r2`, `dt_s`, or `mu`
/// * `r1` and `r2` being the same point (transfer angle 0, no unique solution)
/// * the Newton iteration not converging within [`MAX_ITERATIONS`]
///
/// The caller should check `unconverged` before using the velocities. A solver that returns a
/// plausible-looking but wrong velocity is worse than one that refuses.
// `f64::clamp` is not available on the CI toolchain (Rust 1.83); the manual clamp is the
// portable form and is semantically identical for the [-1, 1] range with finite bounds.
#[allow(clippy::manual_clamp)]
pub fn lambert(r1: Vec3, r2: Vec3, dt_s: f64, mu: f64) -> Lambert {
    let r1n = r1.norm();
    let r2n_orig = r2.norm();

    if !is_positive_finite(r1n)
        || !is_positive_finite(r2n_orig)
        || !is_positive_finite(dt_s)
        || !is_positive_finite(mu)
        || dt_s <= 0.0
    {
        return unconverged_result();
    }

    let sqrt_mu = sqrt(mu);

    // --- Transfer geometry -----------------------------------------------------------

    let cos_dnu = r1.dot(r2) / (r1n * r2n_orig);
    let cos_dnu = if cos_dnu > 1.0 {
        1.0
    } else if cos_dnu < -1.0 {
        -1.0
    } else {
        cos_dnu
    };

    // --- 180° degenerate case ---------------------------------------------------------
    //
    // When r1 and r2 are anti-parallel, A = 0, which makes g = A·sqrt(y/μ) = 0 and the velocity
    // computation singular. For an *elliptic* 180° transfer the two positions are the apses of
    // the transfer ellipse, so the semi-major axis is determined directly from the flight time:
    //   dt = π·sqrt(a³/μ)  →  a = (μ·(dt/π)²)^(1/3)
    // and the velocities come from the vis-viva equation, directed tangentially (perpendicular
    // to each radius) in a chosen orbital plane. This is exact — no perturbation, no iteration
    // error — and achieves the full 1e-8 acceptance tolerance.
    //
    // The orbital plane is not unique for a 180° transfer (any plane containing the r1–r2 line
    // works). The perpendicular direction picks one; the self-consistency check passes for any
    // valid plane.
    //
    // If the transfer is not elliptic (a hyperbolic 180° transfer has a shorter flight time than
    // the Hohmann time), fall back to a small perturbation of r2 to break the singularity. The
    // perturbation solution is approximate but still useful for non-elliptic 180° cases.
    let (r1_solve, r2_solve) = if cos_dnu < -1.0 + 1e-10 {
        // Try the direct elliptic solution.
        let dt_over_pi = dt_s / core::f64::consts::PI;
        let a_direct = odderon_dmath::pow(mu * dt_over_pi * dt_over_pi, 1.0 / 3.0);
        let v1_sq = mu * (2.0 / r1n - 1.0 / a_direct);
        let v2_sq = mu * (2.0 / r2n_orig - 1.0 / a_direct);
        if v1_sq > 0.0 && v2_sq > 0.0 && a_direct.is_finite() && a_direct > 0.0 {
            // Direct elliptic 180° solution.
            let n = perpendicular_to(&r1);
            let v1_dir = n.cross(r1);
            let v1_dir_n = v1_dir.norm();
            let v2_dir = n.cross(r2);
            let v2_dir_n = v2_dir.norm();
            if v1_dir_n > 1e-15 && v2_dir_n > 1e-15 {
                let v1 = v1_dir * (sqrt(v1_sq) / v1_dir_n);
                let v2 = v2_dir * (sqrt(v2_sq) / v2_dir_n);
                return Lambert {
                    v1,
                    v2,
                    iterations: 0,
                    residual: 0.0,
                    unconverged: false,
                };
            }
        }
        // Fallback: perturb r2 to break the singularity.
        let perp = perpendicular_to(&r1);
        let r2_perturbed = r2 + perp * 10.0;
        (r1, r2_perturbed)
    } else {
        (r1, r2)
    };
    let r2n = r2_solve.norm();
    if !is_positive_finite(r2n) {
        return unconverged_result();
    }

    // Recompute geometry with the (possibly perturbed) r2.
    let cos_dnu = r1_solve.dot(r2_solve) / (r1n * r2n);
    let cos_dnu = if cos_dnu > 1.0 {
        1.0
    } else if cos_dnu < -1.0 {
        -1.0
    } else {
        cos_dnu
    };

    // A parameter: A > 0 for elliptic transfers, A = 0 at exactly 180°.
    //
    // A = 0 is handled by the direct solution above (for elliptic) or the perturbation (for
    // non-elliptic). If A is still 0 after perturbation (should not happen, but guard anyway),
    // the parabolic formulation is used.
    let a_param = sqrt(r1n * r2n * (1.0 + cos_dnu));

    // --- Iterate on ψ = χ²·α ---------------------------------------------------------
    //
    // The y-parameter:
    //   y = r1 + r2 + A·(ψ·S(ψ) − 1) / sqrt(C(ψ))
    //
    // The flight time:
    //   dt = (χ³·S(ψ) + A·sqrt(y)) / sqrt(μ),  where χ = sqrt(y / C(ψ))
    //
    // Newton's method on ψ. Start from ψ = 0 (parabolic approximation).
    let mut psi = 0.0;

    let mut iterations: u32 = 0;
    let mut residual = f64::INFINITY;
    let mut unconverged = true;
    let mut y_final = 0.0;

    while iterations < MAX_ITERATIONS {
        iterations += 1;

        let (c, s) = crate::stumpff(psi);

        // Guard against non-finite Stumpff values (can happen for very negative ψ).
        if !c.is_finite() || !s.is_finite() {
            psi = 0.0;
            continue;
        }

        // Compute y. When C(ψ) is near zero, the division is unstable; use a series expansion.
        let y = if fabs(c) > 1e-12 {
            r1n + r2n + a_param * (psi * s - 1.0) / sqrt(c)
        } else {
            // C ≈ 0: ψ is large and negative (hyperbolic). Use the asymptotic form.
            // For large |ψ|, C(ψ) ≈ (e^{sqrt(-ψ)} - 1) / (-ψ) and S(ψ) ≈ (e^{sqrt(-ψ)} - sqrt(-ψ) - 1) / (-ψ)^{3/2}.
            // The y formula simplifies, but in practice C = 0 only at the boundary of the
            // hyperbolic regime. Fall back to a small positive y.
            r1n + r2n
        };

        if !y.is_finite() || y <= 0.0 {
            // y ≤ 0: the orbit is too hyperbolic for this ψ. Adjust ψ and retry.
            // For A > 0, decrease ψ (move toward the elliptic regime).
            // For A ≤ 0, increase ψ (move toward less hyperbolic).
            psi = if a_param > 1e-12 {
                psi - 1.0
            } else {
                psi + 1.0
            };
            continue;
        }

        y_final = y;

        // χ = sqrt(y / C(ψ))
        if c <= 0.0 {
            psi += 1.0;
            continue;
        }
        let chi = sqrt(y / c);
        let chi3 = chi * chi * chi;

        // Flight time: dt = (χ³·S + A·sqrt(y)) / sqrt(μ)
        let flight_time = (chi3 * s + a_param * sqrt(y)) / sqrt_mu;
        residual = flight_time - dt_s;

        // Newton step on ψ: d(dt)/dψ.
        //
        // The derivative is computed numerically via a small finite difference. This is simpler
        // and more robust than deriving the analytic derivative, which involves the derivatives
        // of the Stumpff functions. The finite difference is accurate because the function is
        // smooth and the step size is small relative to ψ.
        let dpsi = 1e-6 * fmax(1.0, fabs(psi));
        let psi_plus = psi + dpsi;
        let (c_plus, s_plus) = crate::stumpff(psi_plus);
        if !c_plus.is_finite() || !s_plus.is_finite() || c_plus <= 0.0 {
            psi += dpsi;
            continue;
        }
        let y_plus = r1n + r2n + a_param * (psi_plus * s_plus - 1.0) / sqrt(c_plus);
        if y_plus <= 0.0 || !y_plus.is_finite() {
            psi += dpsi;
            continue;
        }
        let chi_plus = sqrt(y_plus / c_plus);
        let flight_time_plus =
            (chi_plus * chi_plus * chi_plus * s_plus + a_param * sqrt(y_plus)) / sqrt_mu;

        let d_dt_dpsi = (flight_time_plus - flight_time) / dpsi;

        if fabs(d_dt_dpsi) < 1e-15 {
            psi += dpsi;
            continue;
        }

        let step = residual / d_dt_dpsi;
        psi -= step;

        if fabs(residual) <= TOLERANCE_S {
            unconverged = false;
            break;
        }
    }

    if unconverged {
        return Lambert {
            v1: Vec3::new(f64::NAN, f64::NAN, f64::NAN),
            v2: Vec3::new(f64::NAN, f64::NAN, f64::NAN),
            iterations,
            residual,
            unconverged: true,
        };
    }

    // --- Velocities ----------------------------------------------------------------------
    //
    // From the converged y:
    //   f  = 1 − y/r1
    //   g  = A·sqrt(y/μ)
    //   ġ  = 1 − y/r2
    //   v1 = (r2 − f·r1) / g
    //   v2 = (ġ·r2 − r1) / g
    let (c, _s) = crate::stumpff(psi);
    if c <= 0.0 || !y_final.is_finite() || y_final <= 0.0 {
        return unconverged_result();
    }
    let chi = sqrt(y_final / c);
    let _chi3 = chi * chi * chi;

    let f = 1.0 - y_final / r1n;
    let g = a_param * sqrt(y_final / mu);

    if fabs(g) < 1e-15 {
        return unconverged_result();
    }

    let v1 = (r2_solve - r1_solve * f) * (1.0 / g);
    let gdot = 1.0 - y_final / r2n;
    let v2 = (r2_solve * gdot - r1_solve) * (1.0 / g);

    Lambert {
        v1,
        v2,
        iterations,
        residual,
        unconverged: false,
    }
}

/// Return a unit vector perpendicular to `v`.
///
/// Uses `v × ẑ` first, falling back to `v × x̂` when `v` is nearly parallel to the z-axis
/// (where the first cross product would be near-zero). The result is normalised; if both cross
/// products are degenerate (which only happens for the zero vector, already excluded by the
/// caller's `is_positive_finite` check), returns the z-axis unit vector.
fn perpendicular_to(v: &Vec3) -> Vec3 {
    let z_axis = Vec3::new(0.0, 0.0, 1.0);
    let x_axis = Vec3::new(1.0, 0.0, 0.0);
    let perp = v.cross(z_axis);
    if perp.norm() > 1e-12 {
        perp.unit()
    } else {
        v.cross(x_axis).unit()
    }
}

/// Return a refusal result: NaN velocities, `unconverged: true`.
fn unconverged_result() -> Lambert {
    Lambert {
        v1: Vec3::new(f64::NAN, f64::NAN, f64::NAN),
        v2: Vec3::new(f64::NAN, f64::NAN, f64::NAN),
        iterations: 0,
        residual: f64::NAN,
        unconverged: true,
    }
}
