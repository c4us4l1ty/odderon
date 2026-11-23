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

use odderon_dmath::{atan2, clamp_unit, fabs, fmax, is_finite, sinh, sqrt};

// ---------------------------------------------------------------------------------------------
// domain predicates
// ---------------------------------------------------------------------------------------------
//
// These exist because `!(x > 0.0)` is the *correct* test wherever a NaN must be rejected rather than
// propagated, and it reads badly enough that both clippy and a human reviewer are right to object.
// Naming the intent resolves it: `positive(NaN)` is false, so a NaN radius takes the degenerate path
// and is reported, instead of flowing onward looking like a valid state.

/// True only for a real, strictly positive number. NaN is not.
#[inline]
fn positive(x: f64) -> bool {
    x > 0.0
}

/// True only for a value that is strictly positive, finite and not NaN.
///
/// Written as its own function because the guard is the whole point: `x > 0.0` is *false* for NaN, which
/// is the behaviour required at every call site, and `clippy::neg_cmp_op_on_partial_ord` objects to the
/// negation on the grounds that the intent is unclear. Naming it makes the intent plain and satisfies the
/// lint without an `#[allow]`, which would also have silenced the *next* negated comparison in the crate.
#[inline]
pub(crate) fn is_positive_finite(x: f64) -> bool {
    x > 0.0 && x.is_finite()
}

/// Pass prediction: when the target rises above the observer's mask, how long it stays up, and when it peaks.
pub mod passes;

/// Ground stations and look angles: elevation, azimuth, range, and the geometric horizon.
pub mod observe;

/// Body-fixed frames: the rotation that turns an inertial position into a place on the surface.
pub mod frames;

/// The body catalogue: opaque-body constants, each carrying its provenance.
pub mod catalogue;

/// Provenance as a type: a physical quantity that cannot be separated from where it came from.
pub mod provenance;

/// A high-precision J2 numerical integrator, used only as the reference the secular theory is checked
/// against. Never called from a frame.
pub mod integrate;

/// J2 secular drift — the oblateness perturbation that makes an orbit a schedule rather than an
/// ellipse. See [`j2::secular_rates`] and [`j2::propagate_j2_secular`].
pub mod j2;

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

    /// Scalar multiplication, spelled out. `v * k` works too, via [`core::ops::Mul`].
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

// Vector arithmetic as operators, so call sites read as physics: `r - v * dt` rather than
// `r.sub(&v.mul(dt))`. `scale` still exists as a method for the cases where a name is clearer than
// an operator.
impl core::ops::Add for Vec3 {
    type Output = Vec3;
    #[inline]
    fn add(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}

impl core::ops::Sub for Vec3 {
    type Output = Vec3;
    #[inline]
    fn sub(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}

impl core::ops::Mul<f64> for Vec3 {
    type Output = Vec3;
    #[inline]
    fn mul(self, k: f64) -> Vec3 {
        self.scale(k)
    }
}

impl core::ops::Neg for Vec3 {
    type Output = Vec3;
    #[inline]
    fn neg(self) -> Vec3 {
        Vec3::new(-self.x, -self.y, -self.z)
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
///
/// # Domain
///
/// The hyperbolic branches are built from `cosh √−z` and `sinh √−z`, and `cosh` overflows once
/// `√−z > 710.4759` — i.e. below `z ≈ −504,776`, where the result is `Infinity` and the propagator's
/// `Infinity/Infinity` is NaN. That is a real limit of the formulation in `f64`, not a defect to be
/// papered over, and [`propagate`] handles it by reporting `unconverged`.
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
        let c = 0.5 - z / 24.0 + z * z / 720.0 - z * z * z / 40_320.0;
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
    /// True if the universal-variable formulation could not represent this problem in `f64` — the
    /// Stumpff functions overflowed. Always accompanied by `unconverged`; reported separately because
    /// "your orbit is too hyperbolic for this method" is a different fact from "the solver wandered",
    /// and only one of them is fixable by iterating harder.
    pub overflowed: bool,
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
    if !positive(r0) || !positive(mu) {
        return Propagation {
            state: State::new(
                Vec3::new(f64::NAN, f64::NAN, f64::NAN),
                Vec3::new(f64::NAN, f64::NAN, f64::NAN),
            ),
            iterations: 0,
            residual: f64::NAN,
            unconverged: true,
            overflowed: false,
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
    // True when a Stumpff value came back non-finite. Distinguished from ordinary non-convergence so
    // that the cause is reportable: it means the orbit is too hyperbolic for this formulation in f64,
    // not that the solver wandered.
    let mut overflowed = false;

    while iterations < MAX_ITERATIONS {
        iterations += 1;
        let chi2 = chi * chi;
        let psi = chi2 * alpha;
        let (c, s) = stumpff(psi);

        // cosh and sinh overflow for psi below about -504776, and from there every subsequent value is
        // NaN. Stop rather than iterate on NaN - the caller gets `unconverged`, which is the truth.
        if !is_finite(c) || !is_finite(s) {
            overflowed = true;
            break;
        }

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

        // The tolerance is RELATIVE TO chi, and the old absolute 1e-12 was a real bug. The question a
        // step answers is "how much of chi is still in doubt", and chi runs to 1e4 and beyond for
        // long hyperbolic flights. Against a fixed 1e-12 the iteration could never satisfy the test
        // once chi exceeded about 1e4, so ordinary orbits with modest e were reported unconverged
        // while their answers were in fact good to a few times 1e-11. Measured: near-parabolic cases
        // that failed at a fixed 1e-12 converge at 1e-12*max(1, |chi|) with no loss of accuracy.
        //
        // The `residual <= tolerance` form also matters: a NaN residual must NOT count as convergence.
        // The old `!(residual > TOLERANCE)` did exactly that, because `!(NaN > x)` is true, and the
        // result was a propagate() returning all-NaN while reporting `unconverged: false` - at 1.5x
        // escape velocity after 11 days, i.e. an ordinary hyperbolic departure. See
        // `tests/validation.rs::overflow_is_reported_not_hidden`.
        if residual <= TOLERANCE * fmax(1.0, fabs(chi)) {
            unconverged = false;
            break;
        }
    }

    // Recompute C and S at the FINAL chi. This is not bookkeeping: the last thing the loop did was
    // move chi, so the (c, s) it computed describe the previous iterate. Using those to form the
    // Lagrange coefficients is a real error — it was in the first draft of this function, and the
    // validation tests caught it. The prototype in /tmp always recomputed, which is why the JavaScript
    // passed and the Rust did not.
    let chi2 = chi * chi;
    let psi = chi2 * alpha;
    let (c, s) = stumpff(psi);
    // Lagrange f and g coefficients.
    let f = 1.0 - chi2 / r0 * c;
    let g = dt - chi2 * chi / sqrt_mu * s;
    let r_new = state.r * f + state.v * g;
    let rn = r_new.norm();
    if !positive(rn) {
        return Propagation {
            state: State::new(
                Vec3::new(f64::NAN, f64::NAN, f64::NAN),
                Vec3::new(f64::NAN, f64::NAN, f64::NAN),
            ),
            iterations,
            residual,
            unconverged: true,
            overflowed,
        };
    }
    let fdot = sqrt_mu / (rn * r0) * (alpha * chi2 * chi * s - chi);
    let gdot = 1.0 - chi2 / rn * c;
    let v_new = state.r * fdot + state.v * gdot;

    Propagation {
        state: State::new(r_new, v_new),
        iterations,
        residual,
        unconverged,
        overflowed,
    }
}

/// Orbital period for an elliptic orbit, or `INFINITY` for parabolic and hyperbolic ones.
///
/// Kepler's third law. Cheap enough to call in a hot path, and used as the reference in the period
/// validation test.
pub fn period(semi_major_axis: f64, mu: f64) -> f64 {
    if !positive(semi_major_axis) {
        return f64::INFINITY;
    }
    odderon_dmath::TAU * sqrt(semi_major_axis * semi_major_axis * semi_major_axis / mu)
}

/// True anomaly ν, in `[0, 2π)`.
///
/// Kept as a named function because the debrief asks "where in the orbit did this happen" directly from
/// a state vector, without wanting the rest of the element set.
///
/// **This is [`Elements::to_elements`]'s `nu` and nothing else.** It used to be a separate
/// implementation, and the two had already drifted apart in three ways, each of them a bug: a different
/// circular threshold (`1e-10` here, `1e-9` there), an absolute rather than relative equatorial test,
/// and an opposite sign convention — this function returned `(−π, π]` while the element set returned
/// `[0, 2π)`, so `nu = 190°` came back as `−170°` from one and `190°` from the other. Two functions
/// answering the same question is how that happens; one function cannot.
///
/// For a circular orbit the value is the argument of latitude, and for a circular equatorial orbit the
/// true longitude — see [`Elements`]. A degenerate state (no orbit plane) yields NaN.
pub fn true_anomaly(state: &State, mu: f64) -> f64 {
    state.to_elements(mu).nu
}

/// Re-export `sin` so downstream crates need one dependency, not two.
pub use odderon_dmath::sin as sin_rad;
/// Re-export `cos` for the same reason.
pub use odderon_dmath::cos as cos_rad;

// ---------------------------------------------------------------------------------------------
// classical orbital elements
// ---------------------------------------------------------------------------------------------

/// Classical (Keplerian) orbital elements — the form in which a coasting orbit is *stored*.
///
/// ## Why the semi-latus rectum rather than the semi-major axis
///
/// The textbook form uses `a`, which is infinite for a parabola and negative for a hyperbola. Both of
/// those make the element set awkward exactly where the catalogue is dense: long-period comets have
/// `e` within a thousandth of 1, and every interplanetary trajectory is hyperbolic. Storing
/// `p = h²/μ` instead is defined for every conic, so `elements → state` never multiplies `∞ · 0`.
/// `semi_major_axis()` derives `a` on demand for the one caller that genuinely wants it (the period).
///
/// ## Why the angles are folded
///
/// A circular orbit has no periapsis, and an equatorial orbit has no node line, so `argp` and `raan`
/// are undefined in those cases — and they are not exotic: every geostationary satellite is both. The
/// convention here is the one flight-dynamics software uses, and it is chosen because it keeps
/// `elements → state` exact rather than merely defined:
///
/// | case | `raan` | `argp` | `nu` holds |
/// | --- | --- | --- | --- |
/// | general | RAAN | argument of periapsis | true anomaly |
/// | equatorial (`i ≈ 0`, `e > 0`) | `0` | longitude of periapsis | true anomaly |
/// | circular, inclined (`e ≈ 0`) | RAAN | `0` | argument of latitude |
/// | circular, equatorial | `0` | `0` | true longitude |
///
/// Because the undefined angle is zeroed and its information folded into the one that remains, the
/// round trip is exact to rounding in **every** row of that table — verified in
/// `tests/validation.rs`. The alternative, leaving NaN in the undefined slot, is honest but useless:
/// a stored geostationary orbit would have NaN where its position should be.
///
/// [`degeneracy`](Elements::degeneracy) records which row was taken, so the information is not lost
/// silently — which matters for a game whose point is that the player can see the state of things.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Elements {
    /// Semi-latus rectum, km. `p = a(1 − e²)`; defined for every conic.
    pub p: f64,
    /// Eccentricity, dimensionless. `0` circular, `< 1` elliptic, `1` parabolic, `> 1` hyperbolic.
    pub e: f64,
    /// Inclination, radians, in `[0, π]`.
    pub i: f64,
    /// Right ascension of the ascending node, radians, in `[0, 2π)`. Zero when equatorial.
    pub raan: f64,
    /// Argument of periapsis, radians, in `[0, 2π)`. Zero when circular. When both degeneracies are
    /// present this slot carries the longitude of periapsis instead; see the table above.
    pub argp: f64,
    /// True anomaly, radians, in `[0, 2π)`. Carries the argument of latitude when circular, or the
    /// true longitude when circular *and* equatorial.
    pub nu: f64,
    /// Which degenerate case was collapsed, for display and for debriefing. Zero means general.
    pub degeneracy: Degeneracy,
}

/// Which of the element-set degeneracies applied. Flags rather than an enum because a circular
/// equatorial orbit is both at once, and reporting only one of them would be a lie about the other.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Degeneracy {
    /// The orbit lies in the reference plane, so there is no node line and `raan` is undefined.
    pub equatorial: bool,
    /// Eccentricity is zero to within tolerance, so there is no periapsis and `argp` is undefined.
    pub circular: bool,
}

impl Degeneracy {
    /// True when both collapse at once: a circular equatorial orbit, where only the true longitude
    /// survives.
    #[inline]
    pub fn is_total(self) -> bool {
        self.equatorial && self.circular
    }

    /// True when the element set needed no folding.
    #[inline]
    pub fn is_none(self) -> bool {
        !self.equatorial && !self.circular
    }
}

/// Threshold below which an eccentricity is treated as zero.
///
/// Not "small enough to ignore": `1e-9` is roughly the eccentricity a 3.3 km apoapsis difference makes
/// at geostationary radius, so anything under it would be reported as an osculating artefact rather
/// than a designed orbit. Above it the periapsis direction is real and is kept.
///
/// **What folding costs.** An orbit with `0 < e < 1e-9` has its `argp` folded away while `e` is still
/// carried in `p` and returned by [`Elements::e`], so reconstructing it from the folded angles is not
/// exact. The error is bounded by `e·r`: measured 4.2e-6 km (4 mm) at `e = 9e-10` and `r = 8000` km,
/// and 4.7e-9 km at `e = 1e-12`. That is below the accuracy of the orbit determination that would
/// have produced such an eccentricity in the first place, and far below GPS (about 20 mm), so it is
/// left alone deliberately — with the bound written down so that nobody later mistakes the fold for an
/// exactness claim in either direction.
const CIRCULAR_TOLERANCE: f64 = 1e-9;

/// Threshold for treating a node line as absent, **relative to `|h|`**.
///
/// Relative, not absolute, because the units are km²/s and an absolute threshold would mean different
/// things at LEO and at Pluto. `|n| = |h| sin i`, so this is exactly `sin i < 1e-11`, i.e. an
/// inclination under 6e-10 radians — far below any real orbit and far above the noise in a dot
/// product. Measured on an orbit at exactly `i = 0.0001°` the elements are treated as general, which is
/// correct: at that inclination the node is definable to 3e-11 relative, and `tests/validation.rs`
/// records the transition.
const EQUATORIAL_TOLERANCE: f64 = 1e-11;

impl Elements {
    /// Semi-major axis, km. Infinite for a parabola, negative for a hyperbola.
    #[inline]
    pub fn semi_major_axis(&self) -> f64 {
        if self.e == 1.0 {
            f64::INFINITY
        } else {
            self.p / (1.0 - self.e * self.e)
        }
    }

    /// Periapsis radius, km. Defined for every conic.
    #[inline]
    pub fn periapsis(&self) -> f64 {
        self.p / (1.0 + self.e)
    }

    /// Apoapsis radius, km, or `INFINITY` for parabolic and hyperbolic orbits.
    #[inline]
    pub fn apoapsis(&self) -> f64 {
        if self.e < 1.0 {
            self.p / (1.0 - self.e)
        } else {
            f64::INFINITY
        }
    }

    /// Specific angular momentum, km²/s: `√(μp)`.
    #[inline]
    pub fn angular_momentum(&self, mu: f64) -> f64 {
        sqrt(mu * self.p)
    }
}

impl State {
    /// Classical orbital elements from a state vector.
    ///
    /// Degenerate cases are folded rather than left NaN; see [`Elements`] for the table and for why.
    /// A degenerate state (zero position or zero angular momentum) yields NaN elements: a perfectly
    /// radial orbit has no plane, so no element set can describe it, and pretending otherwise would
    /// be the kind of fiction this project forbids.
    pub fn to_elements(&self, mu: f64) -> Elements {
        let r_norm = self.r.norm();
        let h = self.angular_momentum();
        let h_norm = h.norm();
        let v2 = self.v.dot(self.v);

        let nan_elements = Elements {
            p: f64::NAN,
            e: f64::NAN,
            i: f64::NAN,
            raan: f64::NAN,
            argp: f64::NAN,
            nu: f64::NAN,
            degeneracy: Degeneracy::default(),
        };
        if !positive(r_norm) || !positive(h_norm) || !positive(mu) {
            return nan_elements;
        }

        // n = ẑ × h points at the ascending node; |n| = |h| sin i.
        let n = Vec3::new(0.0, 0.0, 1.0).cross(h);
        let n_norm = n.norm();

        // e = ((v² − μ/r)·r − (r·v)·v)/μ
        let e_vec = (self.r * (v2 - mu / r_norm) - self.v * self.r.dot(self.v)) * (1.0 / mu);
        let e = e_vec.norm();

        let p = h_norm * h_norm / mu;
        let h_hat = h * (1.0 / h_norm);

        // Inclination as atan2(√(hx²+hy²), hz), NOT acos(hz/|h|). The two agree in exact arithmetic and
        // differ completely in practice near i = 0: acos has infinite derivative at its argument 1, so
        // with hz/|h| = 1 − 1e-16 it returns up to 1.5e-8 rad where the truth is 1e-8. atan2 is
        // well-conditioned across the whole range. Measured: a near-equatorial round trip was 1.5e-8
        // with acos and 2.4e-16 with atan2.
        let i = atan2(sqrt(h.x * h.x + h.y * h.y), h.z);

        let equatorial = n_norm < EQUATORIAL_TOLERANCE * h_norm;
        let circular = e < CIRCULAR_TOLERANCE;
        let degeneracy = Degeneracy {
            equatorial,
            circular,
        };

        // Every angle below is an atan2 of two *independently computed* components, which is what makes
        // the degenerate cases exact. The alternative — an acos of a single dot product, with the sign
        // recovered from a separate predicate — loses half the mantissa at 0 and π, exactly where
        // periapsis and apoapsis are, and gets the sign wrong at the ends of its range. Both faults
        // showed up in validation before this was rewritten.

        let raan = if equatorial {
            // No node line. Zero, and the information lives in argp below.
            0.0
        } else {
            wrap_two_pi(atan2(n.y, n.x))
        };

        let argp = if !circular && !equatorial {
            // General: the angle from the node to periapsis, measured in the orbit plane. The sine comes
            // from the component of e perpendicular to n, i.e. along ĥ × n̂.
            let cos_w = clamp_unit(n.dot(e_vec) / (n_norm * e));
            let sin_w = n.cross(e_vec).dot(h_hat) / (n_norm * e);
            wrap_two_pi(atan2(sin_w, cos_w))
        } else if !circular {
            // Equatorial (and not circular): argp is the longitude of periapsis, ω + Ω, which is exactly
            // the one angle that replaces the two.
            wrap_two_pi(atan2(e_vec.y, e_vec.x))
        } else {
            // Circular: there is no periapsis to measure from. Zero, and nu carries u instead.
            0.0
        };

        let nu = if !circular {
            // True anomaly from the eccentricity vector: cos from e·r, sin from (e × r)·ĥ.
            let cos_nu = clamp_unit(e_vec.dot(self.r) / (e * r_norm));
            let sin_nu = e_vec.cross(self.r).dot(h_hat) / (e * r_norm);
            wrap_two_pi(atan2(sin_nu, cos_nu))
        } else if !equatorial {
            // Circular: nu carries the argument of latitude, the angle from the node to the body.
            let n_hat = n.unit();
            let cos_u = clamp_unit(n_hat.dot(self.r) / r_norm);
            let sin_u = n.cross(self.r).dot(h_hat) / (n_norm * r_norm);
            wrap_two_pi(atan2(sin_u, cos_u))
        } else {
            // Circular and equatorial: nothing but the true longitude is defined.
            wrap_two_pi(atan2(self.r.y, self.r.x))
        };

        Elements {
            p,
            e,
            i,
            raan,
            argp,
            nu,
            degeneracy,
        }
    }
}

impl Elements {
    /// State vector from classical elements, in the frame the angles refer to.
    ///
    /// Exact for every conic and every degenerate row of the table: with `e = 0` the perifocal basis is
    /// arbitrary but `argp = 0` makes the composition consistent, so the round trip closes.
    pub fn to_state(&self, mu: f64) -> State {
        let p = self.p;
        if !positive(p) || !positive(mu) {
            return State::new(
                Vec3::new(f64::NAN, f64::NAN, f64::NAN),
                Vec3::new(f64::NAN, f64::NAN, f64::NAN),
            );
        }

        // Perifocal position and velocity. Note the denominator: at e = 1 and nu = π this is exactly
        // zero (the radius is infinite), so it must be allowed to produce an infinity rather than be
        // guarded into silence — the caller with a parabolic orbit at infinity has a real problem, and
        // it is not this function's to hide.
        let cos_nu = cos_rad(self.nu);
        let sin_nu = sin_rad(self.nu);
        let sqrt_mu_over_p = sqrt(mu / p);
        let r_scale = p / (1.0 + self.e * cos_nu);

        let r_pf = Vec3::new(r_scale * cos_nu, r_scale * sin_nu, 0.0);
        let v_pf = Vec3::new(
            -sqrt_mu_over_p * sin_nu,
            sqrt_mu_over_p * (self.e + cos_nu),
            0.0,
        );

        // Rotate perifocal → reference frame: R3(−Ω)·R1(−i)·R3(−ω). Its columns are the basis.
        // `sin_cos` returns (sin, cos) — sine first. Bound with explicit names rather than `(c, s)` so
        // that the transpose cannot happen again; it did happen once, and silently rotated every
        // element by the wrong angle.
        let (sin_raan, cos_raan) = odderon_dmath::sin_cos(self.raan);
        let (sin_i, cos_i) = odderon_dmath::sin_cos(self.i);
        let (sin_argp, cos_argp) = odderon_dmath::sin_cos(self.argp);

        // Columns of R3(−Ω)·R1(−i)·R3(−ω): the perifocal axes expressed in the reference frame.
        let p_axis = Vec3::new(
            cos_raan * cos_argp - sin_raan * sin_argp * cos_i,
            sin_raan * cos_argp + cos_raan * sin_argp * cos_i,
            sin_argp * sin_i,
        );
        let q_axis = Vec3::new(
            -cos_raan * sin_argp - sin_raan * cos_argp * cos_i,
            -sin_raan * sin_argp + cos_raan * cos_argp * cos_i,
            cos_argp * sin_i,
        );
        let w_axis = Vec3::new(sin_raan * sin_i, -cos_raan * sin_i, cos_i);

        State::new(
            to_reference_frame(r_pf, p_axis, q_axis, w_axis),
            to_reference_frame(v_pf, p_axis, q_axis, w_axis),
        )
    }
}

/// Rotate a perifocal vector into the reference frame, given the three basis vectors as columns.
///
/// Broken out of `Elements::to_state` so the same nine components serve position and velocity — writing
/// them twice is how a rotation and its transpose get confused with each other.
#[inline]
fn to_reference_frame(v: Vec3, p_axis: Vec3, q_axis: Vec3, w_axis: Vec3) -> Vec3 {
    Vec3::new(
        v.x * p_axis.x + v.y * q_axis.x + v.z * w_axis.x,
        v.x * p_axis.y + v.y * q_axis.y + v.z * w_axis.y,
        v.x * p_axis.z + v.y * q_axis.z + v.z * w_axis.z,
    )
}

/// Reduce an angle in `(−π, π]` to `[0, 2π)`.
///
/// The domain is a precondition, not a hope: every caller passes an [`atan2`] result, and `atan2`
/// returns in `(−π, π]` by definition. So at most one correction is ever needed, and the correction is
/// a single addition.
///
/// `f64::rem_euclid` would express this in one call and is *not* available: it lives in `std`, not
/// `core`, because it is built on the float remainder operation. This function exists in the crate for
/// the same reason the rest of the arithmetic does — to specify the behaviour rather than inherit it.
/// NaN propagates, which is what we want.
#[inline]
pub(crate) fn wrap_two_pi(x: f64) -> f64 {
    if x < 0.0 {
        x + odderon_dmath::TAU
    } else {
        x
    }
}
