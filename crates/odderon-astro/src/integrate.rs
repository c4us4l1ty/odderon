//! A high-precision zonal-harmonic numerical integrator, used as the *reference* for the secular theory.
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
//! Acceleration under point-mass gravity plus the J2, J3, and J4 zonal harmonics. The potential is
//! `U = −μ/r · [1 − J2(Rₑ/r)²P₂(s) − J3(Rₑ/r)³P₃(s) − J4(Rₑ/r)⁴P₄(s)]` where `s = z/r` is the sine of
//! the geocentric latitude and `P₂, P₃, P₄` are Legendre polynomials. The acceleration is `a = −∇U`:
//! the central `−μr̂/r²` plus the gradient of the perturbation potential. The J2 term (Curtis eq. 12.30,
//! with `J2` as a positive magnitude so the sign convention matches [`crate::j2::EARTH_J2`]) is:
//!
//! ```text
//! a_x = -(3/2) J2 μ Rₑ² / r⁵ · (1 − 5z²/r²) · x      and likewise for y
//! a_z = -(3/2) J2 μ Rₑ² / r⁵ · (3 − 5z²/r²) · z
//! ```
//!
//! The J3 and J4 terms follow the same structure with their own Legendre polynomials and powers of
//! `(Rₑ/r)`. All three are applied together by [`accel_zonal`]; setting `j3 = 0, j4 = 0` reproduces
//! the J2-only field exactly, which is what [`accel_j2`] does.
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

/// Gravitational acceleration from a central body plus its J2, J3, and J4 zonal harmonics, km/s².
///
/// This is the general form. The potential is:
///
/// ```text
/// U = −μ/r · [1 − J2(Rₑ/r)²P₂(s) − J3(Rₑ/r)³P₃(s) − J4(Rₑ/r)⁴P₄(s)]
/// ```
///
/// where `s = z/r` is the sine of the geocentric latitude, `P₂, P₃, P₄` are Legendre polynomials,
/// and `J2, J3, J4` are the signed harmonic coefficients. The acceleration is `a = −∇U`, computed
/// by taking the gradient of the perturbation potential in Cartesian coordinates.
///
/// **Sign convention:** `J2` is a positive magnitude (`EARTH_J2 = +1.0826e-3`), while `J3` and `J4`
/// are signed (`EARTH_J3 = −2.5327e-6`, `EARTH_J4 = −1.6197e-6` for Earth). The formula carries the
/// physical signs; the constants are the measured values. Passing `j3 = 0, j4 = 0` reproduces the
/// J2-only field exactly — see [`accel_j2`] for the convenience wrapper.
///
/// Returns a zero vector for a non-finite or non-positive position magnitude, matching [`accel_j2`].
pub fn accel_zonal(
    r: Vec3,
    mu: f64,
    j2: f64,
    j3: f64,
    j4: f64,
    equatorial_radius: f64,
) -> Vec3 {
    let rn = r.norm();
    if !is_positive_finite(rn) {
        return Vec3::new(0.0, 0.0, 0.0);
    }
    let r2 = rn * rn;
    let rn3 = r2 * rn;
    let rn5 = r2 * r2 * rn;

    // s = sin(geocentric latitude) = z/r
    let s = r.z / rn;
    let s2 = s * s;
    let s4 = s2 * s2;

    // Point-mass acceleration
    let g = -mu / rn3;

    // Common factors: (Rₑ/r)^n for each harmonic
    let re2 = equatorial_radius * equatorial_radius;
    let re3 = re2 * equatorial_radius;
    let re4 = re3 * equatorial_radius;

    let mut ax = g * r.x;
    let mut ay = g * r.y;
    let mut az = g * r.z;

    // --- J2 term: P₂(s) = (3s² − 1)/2 ---
    // a_J2 = −(3/2) J2 μ Rₑ² / r⁵ · [(1 − 5s²) x̂, (1 − 5s²) ŷ, (3 − 5s²) ẑ]
    if j2 != 0.0 {
        let k2 = -1.5 * j2 * mu * re2 / rn5;
        ax += k2 * (1.0 - 5.0 * s2) * r.x;
        ay += k2 * (1.0 - 5.0 * s2) * r.y;
        az += k2 * (3.0 - 5.0 * s2) * r.z;
    }

    // --- J3 term: P₃(s) = (5s³ − 3s)/2 ---
    // The gradient of −μ J3 Rₑ³ P₃(s) / r⁴ gives a J3 acceleration with components:
    // a_x, a_y ∝ (15s² − 3) and a_z ∝ (15s⁴ − 12s² + 1)  (derived from −∇U_J3)
    if j3 != 0.0 {
        // U_J3 = −μ J3 Rₑ³ / r⁴ · P₃(s) = −μ J3 Rₑ³ / (2r⁴) · (5s³ − 3s)
        // ∂U/∂x: the gradient in Cartesian coordinates, with s = z/r.
        // The full derivation (Curtis §12.3 / Vallado §8.5) gives:
        // a_x = −(5/2) J3 μ Rₑ³ / r⁶ · (15s² − 3) · x
        // a_y = same with y
        // a_z = −(5/2) J3 μ Rₑ³ / r⁶ · (15s⁴ − 12s² + 1) · r  ... but this needs the r̂ component
        // Actually: a_J3 = −∇U_J3 where U_J3 = −μ J3 Rₑ³ (5s³−3s) / (2r⁴)
        // The cleanest form: compute the gradient directly.
        // ∂s/∂x = −zx/r³ = −sx/r²,  ∂s/∂z = (r²−z²)/r³ = (1−s²)/r
        // ∂(1/r⁴)/∂x = −4x/r⁶,  ∂(1/r⁴)/∂z = −4z/r⁶
        // U_J3 = −μ J3 Rₑ³ (5s³−3s) / (2r⁴)
        // ∂U/∂x = −μ J3 Rₑ³ / 2 · [∂(5s³−3s)/∂x / r⁴ + (5s³−3s) · ∂(1/r⁴)/∂x]
        //        = −μ J3 Rₑ³ / 2 · [(15s²−3)·(−sx/r²) / r⁴ + (5s³−3s)·(−4x/r⁶)]
        //        = μ J3 Rₑ³ x / (2r⁶) · [(15s²−3)·s + 4(5s³−3s)]
        //        = μ J3 Rₑ³ x / (2r⁶) · [15s³−3s + 20s³−12s]
        //        = μ J3 Rₑ³ x / (2r⁶) · [35s³ − 15s]
        //        = μ J3 Rₑ³ x · 5s(7s²−3) / (2r⁶)
        // a_x = −∂U/∂x = −μ J3 Rₑ³ x · 5s(7s²−3) / (2r⁶)
        //
        // Similarly for y.
        // ∂U/∂z = −μ J3 Rₑ³ / 2 · [(15s²−3)·(1−s²)/r / r⁴ + (5s³−3s)·(−4z/r⁶)]
        //        = −μ J3 Rₑ³ / 2 · [(15s²−3)(1−s²) / r⁵ − 4z(5s³−3s) / r⁶]
        //        = −μ J3 Rₑ³ / (2r⁶) · [r(15s²−3)(1−s²) − 4z(5s³−3s)]
        //        = −μ J3 Rₑ³ / (2r⁶) · [r(15s²−3)(1−s²) − 4sr(5s³−3s)]
        //        = −μ J3 Rₑ³ / (2r⁵) · [(15s²−3)(1−s²) − 4s(5s³−3s)]
        //        = −μ J3 Rₑ³ / (2r⁵) · [15s²−3−15s⁴+3s² − 20s⁴+12s²]
        //        = −μ J3 Rₑ³ / (2r⁵) · [−35s⁴+30s²−3]
        //        = μ J3 Rₑ³ (35s⁴−30s²+3) / (2r⁵)
        // a_z = −∂U/∂z = −μ J3 Rₑ³ (35s⁴−30s²+3) / (2r⁵)
        //
        // The gradient in Cartesian coordinates, with s = z/r and ∂s/∂x = −sx/r²,
        // ∂s/∂z = (1−s²)/r:
        //   ∂U/∂x = μ J3 Rₑ³ x · 5s(7s²−3) / (2r⁶)   →  a_x = −∂U/∂x
        //   ∂U/∂z = −μ J3 Rₑ³ (35s⁴−30s²+3) / (2r⁵)  →  a_z = −∂U/∂z
        let fx = 5.0 * s * (7.0 * s2 - 3.0); // 5s(7s²−3)
        let fz = 35.0 * s4 - 30.0 * s2 + 3.0; // 35s⁴−30s²+3
        let rn6 = rn5 * rn;
        ax += mu * j3 * re3 * r.x * fx / (2.0 * rn6);
        ay += mu * j3 * re3 * r.y * fx / (2.0 * rn6);
        az += mu * j3 * re3 * fz / (2.0 * rn5);
    }

    // --- J4 term: P₄(s) = (35s⁴ − 30s² + 3)/8 ---
    // U_J4 = −μ J4 Rₑ⁴ / r⁵ · P₄(s) = −μ J4 Rₑ⁴ (35s⁴−30s²+3) / (8r⁵)
    // Gradient: ∂s/∂x = −sx/r², ∂s/∂z = (1−s²)/r
    // ∂U/∂x = −μ J4 Rₑ⁴ / 8 · [∂(35s⁴−30s²+3)/∂x / r⁵ + (35s⁴−30s²+3) · ∂(1/r⁵)/∂x]
    //        = −μ J4 Rₑ⁴ / 8 · [(140s³−60s)(−sx/r²) / r⁵ + (35s⁴−30s²+3)(−5x/r⁷)]
    //        = μ J4 Rₑ⁴ x / (8r⁷) · [(140s³−60s)·s + 5(35s⁴−30s²+3)]
    //        = μ J4 Rₑ⁴ x / (8r⁷) · [140s⁴−60s² + 175s⁴−150s²+15]
    //        = μ J4 Rₑ⁴ x / (8r⁷) · [315s⁴ − 210s² + 15]
    //        = μ J4 Rₑ⁴ x · 15(21s⁴−14s²+1) / (8r⁷)
    // a_x = −∂U/∂x = −μ J4 Rₑ⁴ x · 15(21s⁴−14s²+1) / (8r⁷)
    //
    // ∂U/∂z = −μ J4 Rₑ⁴ / 8 · [(140s³−60s)(1−s²)/r / r⁵ + (35s⁴−30s²+3)(−5z/r⁷)]
    //        = −μ J4 Rₑ⁴ / 8 · [(140s³−60s)(1−s²) / r⁶ − 5z(35s⁴−30s²+3) / r⁷]
    //        = −μ J4 Rₑ⁴ / (8r⁷) · [r(140s³−60s)(1−s²) − 5sr(35s⁴−30s²+3)]
    //        = −μ J4 Rₑ⁴ / (8r⁶) · [(140s³−60s)(1−s²) − 5s(35s⁴−30s²+3)]
    //        = −μ J4 Rₑ⁴ / (8r⁶) · [140s³−60s−140s⁵+60s³ − 175s⁵+150s³−15s]
    //        = −μ J4 Rₑ⁴ / (8r⁶) · [−315s⁵ + 350s³ − 75s]
    //        = μ J4 Rₑ⁴ / (8r⁶) · [315s⁵ − 350s³ + 75s]
    //        = μ J4 Rₑ⁴ · 5s(63s⁴−70s²+15) / (8r⁶)
    // a_z = −∂U/∂z = −μ J4 Rₑ⁴ · 5s(63s⁴−70s²+15) / (8r⁶)
    //
    // Note: the sign convention here is that the acceleration is −∇U. Since U_J4 is
    // −μ J4 Rₑ⁴ P₄(s) / r⁵ (negative J4 gives positive potential contribution), the
    // gradient ∂U/∂x is negative, making −∂U/∂x positive — hence the `+` in the code.
    if j4 != 0.0 {
        let rn7 = rn5 * r2;
        let rn6 = rn5 * rn;
        let fx4 = 15.0 * (21.0 * s4 - 14.0 * s2 + 1.0); // 15(21s⁴−14s²+1)
        let fz4 = 5.0 * s * (63.0 * s4 - 70.0 * s2 + 15.0); // 5s(63s⁴−70s²+15)
        ax += mu * j4 * re4 * r.x * fx4 / (8.0 * rn7);
        ay += mu * j4 * re4 * r.y * fx4 / (8.0 * rn7);
        az += mu * j4 * re4 * fz4 / (8.0 * rn6);
    }

    Vec3::new(ax, ay, az)
}

/// Gravitational acceleration from a central body plus its J2 oblateness term, km/s².
///
/// `j2` is the **positive magnitude** `1.08263e-3`, matching [`crate::j2::EARTH_J2`]. The sign of the
/// oblateness is carried by the formula, not by the constant — see that constant's documentation for why
/// that choice cost a PR.
///
/// This is a convenience wrapper around [`accel_zonal`] with `j3 = 0, j4 = 0`.
pub fn accel_j2(r: Vec3, mu: f64, j2: f64, equatorial_radius: f64) -> Vec3 {
    accel_zonal(r, mu, j2, 0.0, 0.0, equatorial_radius)
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

/// The J2+J3+J4 contribution to the specific potential energy, km²/s².
///
/// ```text
/// U_zonal = μ Rₑ² / (2r³) · [J2(3s²−1) + J3·(Rₑ/r)·(5s³−3s)/2·2 + J4·(Rₑ/r)²·(35s⁴−30s²+3)/4·2]
/// ```
///
/// More precisely, the perturbation potential for the signed harmonics is:
///
/// ```text
/// U_J2 =  μ J2 Rₑ² / (2r³) · (3s² − 1)
/// U_J3 =  μ J3 Rₑ³ / (2r⁴) · (5s³ − 3s)
/// U_J4 =  μ J4 Rₑ⁴ / (8r⁵) · (35s⁴ − 30s² + 3)
/// ```
///
/// where `s = z/r`. The total conserved energy under the full zonal field is
/// `v²/2 − μ/r + U_J2 + U_J3 + U_J4`. This is the quantity a test asserts is constant when the
/// integrator runs with all three harmonics on — the same check [`potential_j2`] enables for J2 alone,
/// extended to the full field this module integrates.
///
/// Verified by finite-differencing against [`accel_zonal`] in the test file: `−∇U_zonal` reproduces the
/// full zonal acceleration (J2+J3+J4) to 1e-9 relative.
pub fn potential_zonal(
    r: Vec3,
    mu: f64,
    j2: f64,
    j3: f64,
    j4: f64,
    equatorial_radius: f64,
) -> f64 {
    let rn = r.norm();
    if !is_positive_finite(rn) {
        return 0.0;
    }
    let s = r.z / rn;
    let s2 = s * s;
    let s3 = s2 * s;
    let s4 = s2 * s2;
    let re2 = equatorial_radius * equatorial_radius;
    let re3 = re2 * equatorial_radius;
    let re4 = re3 * equatorial_radius;

    let u2 = mu * j2 * re2 / (2.0 * rn * rn * rn) * (3.0 * s2 - 1.0);
    let u3 = mu * j3 * re3 / (2.0 * rn * rn * rn * rn) * (5.0 * s3 - 3.0 * s);
    let u4 = mu * j4 * re4 / (8.0 * rn * rn * rn * rn * rn) * (35.0 * s4 - 30.0 * s2 + 3.0);
    u2 + u3 + u4
}

/// The state derivative: `(v, a)`.
///
/// Uses [`accel_zonal`] so the integrator can step under the full J2+J3+J4 field. The J2-only
/// variant [`integrate_j2`] is a wrapper that passes `j3 = 0, j4 = 0`.
fn deriv_zonal(s: [f64; 6], mu: f64, j2: f64, j3: f64, j4: f64, re: f64) -> [f64; 6] {
    let a = accel_zonal(Vec3::new(s[0], s[1], s[2]), mu, j2, j3, j4, re);
    [s[3], s[4], s[5], a.x, a.y, a.z]
}

/// The state derivative for J2 only. Kept so [`integrate_j2`] does not need to thread `j3, j4`
/// through every stage.
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

    let mut s = [
        state.r.x, state.r.y, state.r.z, state.v.x, state.v.y, state.v.z,
    ];

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
        state: State::new(Vec3::new(s[0], s[1], s[2]), Vec3::new(s[3], s[4], s[5])),
        accepted,
        rejected,
        min_step,
        max_step,
    })
}

/// Integrate from `state` for `dt` seconds under two-body + J2 + J3 + J4, adaptively.
///
/// This is the same Dormand–Prince 5(4) integrator as [`integrate_j2`], but the acceleration
/// function is [`accel_zonal`] — the full zonal field with all three harmonics. The step controller,
/// initial-step heuristic, and step-count cap are identical, so the two functions differ only in
/// which acceleration they call at each stage.
///
/// `j2` is the **positive magnitude** (`EARTH_J2`); `j3` and `j4` are **signed**
/// (`EARTH_J3`, `EARTH_J4`). Passing `j3 = 0, j4 = 0` reproduces [`integrate_j2`] exactly.
///
/// `rel_tol` is the per-step relative error target on the *state vector*, same as [`integrate_j2`].
/// Returns `None` for a non-finite or non-positive `dt`, or if the integrator cannot make progress.
#[allow(clippy::needless_range_loop)]
pub fn integrate_zonal(
    state: &State,
    dt: f64,
    mu: f64,
    j2: f64,
    j3: f64,
    j4: f64,
    equatorial_radius: f64,
    rel_tol: f64,
) -> Option<Integrated> {
    if !dt.is_finite() || dt < 0.0 || !is_positive_finite(rel_tol) {
        return None;
    }

    let mut s = [
        state.r.x, state.r.y, state.r.z, state.v.x, state.v.y, state.v.z,
    ];

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

    const MAX_STEPS: u32 = 4_000_000;

    while t < dt {
        if accepted + rejected > MAX_STEPS {
            return None;
        }
        let remaining = dt - t;
        if h > remaining {
            h = remaining;
        }

        let mut k = [[0.0f64; 6]; 7];
        k[0] = deriv_zonal(s, mu, j2, j3, j4, equatorial_radius);
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
            k[i] = deriv_zonal(si, mu, j2, j3, j4, equatorial_radius);
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

        let mut err = 0.0;
        for m in 0..6 {
            err = fmax(err, fabs(s5[m] - s4[m]));
        }
        let scale = fmax(1.0, sqrt(s5[0] * s5[0] + s5[1] * s5[1] + s5[2] * s5[2]));

        if err > rel_tol * scale {
            rejected += 1;
            h *= fmax(0.2, 0.9 * pow(rel_tol * scale / err, 0.2));
            if h < 1e-6 {
                return None;
            }
            continue;
        }

        s = s5;
        t += h;
        accepted += 1;
        min_step = if h < min_step { h } else { min_step };
        max_step = if h > max_step { h } else { max_step };

        let factor = if err > 0.0 {
            0.9 * pow(rel_tol * scale / err, 0.2)
        } else {
            5.0
        };
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
        state: State::new(Vec3::new(s[0], s[1], s[2]), Vec3::new(s[3], s[4], s[5])),
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
