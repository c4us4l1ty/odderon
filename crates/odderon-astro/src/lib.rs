//! # `odderon-astro` — the two-body kernel
//!
//! Propagation that never runs a loop unless something is thrusting. The universe is a *function of
//! time* (`VISION.md` §5.1): to find where a coasting spacecraft is at time `t`, the kernel evaluates a
//! closed form. Mars does not move at 60 Hz, and neither does anything else.
//!
//! ## What is here
//!
//! * [`Vec3`] — positions and velocities in a local frame (`f64`, kilometres and kilometres/second).
//! * [`State`] — a state vector `(r, v)` at an epoch.
//! * [`Elements`] — classical orbital elements, the form in which a coasting orbit is *stored*.
//! * [`propagate`] — the universal-variable Kepler propagator: one closed form that handles circular,
//!   elliptic, parabolic and hyperbolic orbits without branching on eccentricity.
//!
//! ## Units, chosen once and stated everywhere
//!
//! * length **km**, time **s**, so `mu` is **km³/s²**.
//! * angles in **radians** internally; degrees only at a display boundary.
//! * epochs are **µs since J2000 TDB** as `i64`, per `VISION.md` §5.2 — enough for ±292,000 years.
//!   This crate does not interpret epochs; it propagates by a `dt` in seconds, so it cannot get time
//!   systems wrong by accident. That is deliberate: leap seconds belong to exactly one module, later.
//!
//! ## Why universal variables
//!
//! The classical formulation needs a different solver per conic type (Kepler's equation for ellipses,
//! Barker's for parabolas, hyperbolic Kepler for hyperbolas), and the boundaries between them are
//! where propagators break. Universal variables replace all three with one equation in one variable,
//! which is why `e = 0`, `e = 1` and `e = 1 + ε` need no special cases. The catalogue is full of
//! near-parabolic comets and near-circular satellite orbits, i.e. exactly the two regimes that punish
//! the classical approach.
//!
//! ## Accuracy
//!
//! Measured, not asserted. `tests/validation.rs` compares against analytic invariants (energy,
//! angular momentum, period) and against published test vectors; the tolerance is written before the
//! test. The propagator is Newton's method on the universal Kepler equation, so it converges
//! quadratically — typically to machine precision in 3–6 iterations.

#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

use odderon_dmath::{acos, atan2, fabs, fmax, sinh, sqrt};

/// Gravitational parameters, km³/s².
///
/// Values from the IAU/JPL published constants. They are *data*, not physics: a body's `mu` is a
/// measured quantity with a stated uncertainty, and in the full kernel it will arrive as a
/// `Provenance` node rather than a bare `f64`. For now the constants are marked with their source so
/// that the eventual upgrade is a mechanical change rather than an archaeology exercise.
pub mod mu {
    /// Earth, from the C00 value `GM = 398600.4418 km³/s²` (EGM/IAU 2009).
    pub const EARTH: f64 = 398_600.441_8;
    /// Moon.
    pub const MOON: f64 = 4_902.800_066;
    /// Mars system barycentre is not a useful `mu` for local orbits; this is Mars itself.
    pub const MARS: f64 = 42_828.37;
    /// Sun.
    pub const SUN: f64 = 132_712_440_041.938_5;
    /// Jupiter.
    pub const JUPITER: f64 = 126_686_534.0;
}

/// Mean equatorial radii, km. Also data, also to become `Provenance` later.
pub mod radius {
    /// Earth mean equatorial radius.
    pub const EARTH: f64 = 6_378.137;
    /// Moon mean radius.
    pub const MOON: f64 = 1_737.4;
    /// Mars mean equatorial radius.
    pub const MARS: f64 = 3_396.19;
    /// Sun mean radius.
    pub const SUN: f64 = 695_700.0;
}

// ---------------------------------------------------------------------------------------------
// vectors
// ---------------------------------------------------------------------------------------------

/// A vector in three dimensions: a position (km) or a velocity (km/s), depending on context.
///
/// Deliberately not a generic vector type. The kernel does not need one, and a concrete type keeps the
/// arithmetic legible when someone is checking a formula against a textbook.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vec3 {
    /// x component.
    pub x: f64,
    /// y component.
    pub y: f64,
    /// z component.
    pub z: f64,
}

impl Vec3 {
    /// The zero vector.
    pub const ZERO: Vec3 = Vec3 { x: 0.0, y: 0.0, z: 0.0 };

    /// Construct from components.
    #[inline]
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Vec3 { x, y, z }
    }

    /// Component-wise addition.
    #[inline]
    pub fn add(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }

    /// Component-wise subtraction.
    #[inline]
    pub fn sub(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }

    /// Scalar multiplication.
    #[inline]
    pub fn scale(self, k: f64) -> Vec3 {
        Vec3::new(self.x * k, self.y * k, self.z * k)
    }

    /// Dot product.
    #[inline]
    pub fn dot(self, o: Vec3) -> f64 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }

    /// Cross product.
    #[inline]
    pub fn cross(self, o: Vec3) -> Vec3 {
        Vec3::new(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }

    /// Euclidean norm (magnitude).
    #[inline]
    pub fn norm(self) -> f64 {
        sqrt(self.dot(self))
    }

    /// Unit vector, or the zero vector if the norm is zero.
    ///
    /// Returning zero rather than NaN keeps a degenerate input from poisoning a whole trajectory; the
    /// caller can test the norm if it needs to know.
    #[inline]
    pub fn unit(self) -> Vec3 {
        let n = self.norm();
        if n == 0.0 {
            Vec3::ZERO
        } else {
            self.scale(1.0 / n)
        }
    }
}

// ---------------------------------------------------------------------------------------------
// state
// ---------------------------------------------------------------------------------------------

/// A state vector: where something is and how fast, at one epoch.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct State {
    /// Position, km.
    pub r: Vec3,
    /// Velocity, km/s.
    pub v: Vec3,
}

impl State {
    /// Construct a state.
    #[inline]
    pub const fn new(r: Vec3, v: Vec3) -> Self {
        State { r, v }
    }

    /// Specific orbital energy, km²/s². Negative for elliptic, zero for parabolic, positive for
    /// hyperbolic.
    #[inline]
    pub fn specific_energy(&self, mu: f64) -> f64 {
        0.5 * self.v.dot(self.v) - mu / self.r.norm()
    }

    /// Specific angular momentum vector, km²/s.
    #[inline]
    pub fn angular_momentum(&self) -> Vec3 {
        self.r.cross(self.v)
    }

    /// Semi-major axis from the vis-viva equation. Infinite for a parabolic orbit.
    #[inline]
    pub fn semi_major_axis(&self, mu: f64) -> f64 {
        let a_inv = 2.0 / self.r.norm() - self.v.dot(self.v) / mu;
        if a_inv == 0.0 {
            f64::INFINITY
        } else {
            1.0 / a_inv
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Stumpff functions
// ---------------------------------------------------------------------------------------------

/// The Stumpff functions `C(z)` and `S(z)`, which absorb the elliptic/parabolic/hyperbolic distinction.
///
/// `C(z) = (1 − cos √z)/z` for `z > 0`, `1/2` for `z = 0`, and `(cosh √−z − 1)/(−z)` for `z < 0`;
/// `S(z)` analogously with `sin`/`sinh`. They are the reason one propagator covers every conic.
///
/// The threshold `|z| < 1e-6` uses the series because the closed forms lose precision to cancellation
/// as `z → 0`; the first omitted terms are `z⁴/8!` and `z⁴/9!`, so four terms are ample.
pub fn stumpff(z: f64) -> (f64, f64) {
    if z > 1e-6 {
        let s = sqrt(z);
        let (sin_s, cos_s) = odderon_dmath::sin_cos(s);
        ((1.0 - cos_s) / z, (s - sin_s) / (s * s * s))
    } else if z < -1e-6 {
        let s = sqrt(-z);
        let ch = odderon_dmath::cosh(s);
        ((ch - 1.0) / -z, (sinh(s) - s) / (s * s * s))
    } else {
        // Series in z. C = 1/2! − z/4! + z²/6! − z³/8!; S = 1/3! − z/5! + z²/7! − z³/9!.
        let c = 1.0 / 2.0 - z / 24.0 + z * z / 720.0 - z * z * z / 40_320.0;
        let s = 1.0 / 6.0 - z / 120.0 + z * z / 5_040.0 - z * z * z / 362_880.0;
        (c, s)
    }
}

// ---------------------------------------------------------------------------------------------
// propagation
// ---------------------------------------------------------------------------------------------

/// Result of a propagation, including how the solve went.
///
/// Returning the iteration count and residual rather than swallowing them is a deliberate choice: the
/// kernel's honesty rule (`GAME-DESIGN-BIBLE.md` P1) applies to its own solvers too. If a propagation
/// needed 40 iterations, something is near-singular and the caller should be able to see that.
#[derive(Clone, Copy, Debug)]
pub struct Propagation {
    /// State after `dt`.
    pub state: State,
    /// Newton iterations used.
    pub iterations: u32,
    /// Final `|Δχ|`, i.e. how converged the solution is. Should be at or near machine epsilon.
    pub residual: f64,
    /// True if the iteration limit was hit without meeting the tolerance. A caller that ignores this
    /// is taking a number on trust, which is exactly what this project refuses to do.
    pub unconverged: bool,
}

/// Newton iteration limit. Universal-variable Kepler converges quadratically, so this is generous by
/// an order of magnitude; exceeding it means the problem is near-parabolic *and* badly scaled.
const MAX_ITERATIONS: u32 = 64;

/// Convergence tolerance on the step in `χ`, dimensionless.
const TOLERANCE: f64 = 1e-12;

/// Propagate a two-body state by `dt` seconds using universal variables.
///
/// One closed form for every conic type. No special cases for `e = 0`, `e = 1`, or hyperbolic orbits.
///
/// # Arguments
///
/// * `state` — position (km) and velocity (km/s) at the epoch.
/// * `dt` — elapsed time in seconds. May be negative: the propagator is time-reversible, and a
///   `dt` of `-P` returns the original state to machine precision.
/// * `mu` — gravitational parameter of the central body, km³/s².
///
/// # Degenerate input
///
/// A zero-radius state has no orbit; the result is a state of NaNs rather than a panic, because a
/// simulator must survive being handed nonsense by a player mid-design.
pub fn propagate(state: &State, dt: f64, mu: f64) -> Propagation {
    let r0 = state.r.norm();
    if !(r0 > 0.0) || !(mu > 0.0) {
        return Propagation {
            state: State::new(
                Vec3::new(f64::NAN, f64::NAN, f64::NAN),
                Vec3::new(f64::NAN, f64::NAN, f64::NAN),
            ),
            iterations: 0,
            residual: f64::NAN,
            unconverged: true,
        };
    }

    let sqrt_mu = sqrt(mu);
    let rv = state.r.dot(state.v);
    // alpha = 1/a. Its sign selects the conic, but the solver never branches on it.
    let alpha = 2.0 / r0 - state.v.dot(state.v) / mu;

    // Initial guess for chi. A poor guess costs iterations, not accuracy, but a *wrong-signed* guess
    // for hyperbolic orbits can send Newton the wrong way, so each case is seeded separately.
    let mut chi = if alpha > 1e-12 {
        // Elliptic: chi ≈ sqrt(mu)·dt·alpha, which is exact in the limit of small dt.
        sqrt_mu * dt * alpha
    } else if alpha < -1e-12 {
        // Hyperbolic: the closed-form seed from the hyperbolic Kepler equation.
        let a = 1.0 / alpha;
        let s = if dt < 0.0 { -1.0 } else { 1.0 };
        let denom = rv + s * sqrt(-mu * a) * (1.0 - r0 * alpha);
        s * sqrt(-a) * odderon_dmath::ln(-2.0 * mu * alpha * dt / denom)
    } else {
        // Parabolic: chi ≈ sqrt(mu)·dt/r0.
        sqrt_mu * dt / r0
    };
    // The hyperbolic seed can be non-finite when dt is tiny; fall back to the parabolic form.
    if !chi.is_finite() {
        chi = sqrt_mu * dt / r0;
    }

    let mut iterations: u32 = 0;
    let mut residual = f64::INFINITY;
    let mut unconverged = true;
    let mut c = 0.5;
    let mut s = 1.0 / 6.0;

    while iterations < MAX_ITERATIONS {
        iterations += 1;
        let psi = chi * chi * alpha;
        let (cc, ss) = stumpff(psi);
        c = cc;
        s = ss;

        let chi2 = chi * chi;
        // F(chi) = the universal Kepler equation, expressed in the numerically stable form.
        let f = rv / sqrt_mu * chi2 * c
            + (1.0 - r0 * alpha) * chi2 * chi * s
            + r0 * chi
            - sqrt_mu * dt;
        // dF/dchi = r, the current radius.
        let r = chi2 * c + rv / sqrt_mu * chi * (1.0 - psi * s) + r0 * (1.0 - psi * c);

        let step = f / r;
        chi -= step;
        residual = fabs(step);
        if !(residual > TOLERANCE) {
            unconverged = false;
            break;
        }
    }

    // Recompute C and S at the final chi (the loop's values are one step stale on the way out).
    let psi = chi * chi * alpha;
    let (c, s) = if unconverged {
        stumpff(psi)
    } else {
        (c, s)
    };
    let _ = psi;

    let chi2 = chi * chi;
    // Lagrange f and g coefficients.
    let f = 1.0 - chi2 / r0 * c;
    let g = dt - chi2 * chi / sqrt_mu * s;
    let r_new = state.r.scale(f).add(state.v.scale(g));
    let rn = r_new.norm();
    if !(rn > 0.0) {
        return Propagation {
            state: State::new(
                Vec3::new(f64::NAN, f64::NAN, f64::NAN),
                Vec3::new(f64::NAN, f64::NAN, f64::NAN),
            ),
            iterations,
            residual,
            unconverged: true,
        };
    }
    let fdot = sqrt_mu / (rn * r0) * (alpha * chi2 * chi * s - chi);
    let gdot = 1.0 - chi2 / rn * c;
    let v_new = state.r.scale(fdot).add(state.v.scale(gdot));

    Propagation {
        state: State::new(r_new, v_new),
        iterations,
        residual,
        unconverged,
    }
}

/// Orbital period for an elliptic orbit, or `INFINITY` for parabolic and hyperbolic ones.
///
/// Kepler's third law. Cheap enough to call in a hot path, and used as the reference in the period
/// validation test.
pub fn period(semi_major_axis: f64, mu: f64) -> f64 {
    if !(semi_major_axis > 0.0) {
        return f64::INFINITY;
    }
    odderon_dmath::TAU * sqrt(semi_major_axis * semi_major_axis * semi_major_axis / mu)
}

/// True anomaly ν, in `(−π, π]`, from the eccentricity vector.
///
/// Kept here rather than on [`Elements`] because the debrief asks "where in the orbit did this happen"
/// directly from a state vector, without a round trip through elements.
///
/// A circular orbit has no defined periapsis, so its true anomaly is degenerate. Rather than return a
/// misleading number, this returns the **argument of latitude** in that case, which is the quantity
/// that is actually defined — and it is the same convention flight dynamics software uses.
pub fn true_anomaly(state: &State, mu: f64) -> f64 {
    let h = state.angular_momentum();
    let h_norm = h.norm();
    let r_norm = state.r.norm();
    if !(h_norm > 0.0) || !(r_norm > 0.0) {
        return f64::NAN;
    }
    // e = ((v² − mu/r)·r − (r·v)·v) / mu
    let v2 = state.v.dot(state.v);
    let e_vec = state
        .r
        .scale(v2 - mu / r_norm)
        .sub(state.v.scale(state.r.dot(state.v)))
        .scale(1.0 / mu);
    let e = e_vec.norm();

    if e < 1e-10 {
        // Degenerate: no periapsis direction. Argue of latitude = angle from the node line.
        let node = Vec3::new(0.0, 0.0, 1.0).cross(h);
        if node.norm() < 1e-12 {
            // Equatorial circular: use the +x axis as the reference direction.
            let nu = atan2(state.r.y, state.r.x);
            return nu;
        }
        let n = node.unit();
        let cos_u = n.dot(state.r.unit());
        let sign = if state.r.cross(n).dot(h) < 0.0 { -1.0 } else { 1.0 };
        return sign * acos(fmax(-1.0, cos_u.min(1.0)));
    }

    let cos_nu = e_vec.dot(state.r) / (e * r_norm);
    let nu = acos(fmax(-1.0, cos_nu.min(1.0)));
    // acos loses the sign; recover it from the direction of motion.
    if state.r.dot(state.v) < 0.0 {
        -nu
    } else {
        nu
    }
}

/// Re-export `sin` so downstream crates need one dependency, not two.
pub use odderon_dmath::sin as sin_rad;
/// Re-export `cos` for the same reason.
pub use odderon_dmath::cos as cos_rad;
