//! J2 secular drift — the first perturbation that makes an orbit a *schedule* rather than an ellipse.
//!
//! ## Why this exists
//!
//! Two-body motion closes. A satellite returns to exactly where it was, forever, and the question
//! "where will it be next week" has an answer that costs nothing to compute and is wrong by kilometres.
//! Earth is oblate, and the oblateness makes the orbit plane **precess** and the argument of periapsis
//! **rotate**. Over a week at low Earth orbit that is tens of kilometres; over a mission it is the
//! difference between a ground track that repeats and one that does not.
//!
//! This is `BUILD-PLAN.md` §4b's next rung, and it is deliberately *secular only*: it applies the
//! first-order averaged effect, not the short-period wobble. That is the right level for a catalogue of
//! coasting bodies (`VISION.md` §5.1), and the wrong level for a rendezvous, which needs J2 + J3 + J4
//! through an integrator and has its own row in `CREDIBILITY.md`.
//!
//! ## The equations, and where they come from
//!
//! First-order secular theory (Vallado §9.8), averaged over one orbit:
//!
//! ```text
//! Ω̇ = -3/2 · n · J2 · (Rₑ/p)² · cos i
//! ω̇ =  3/4 · n · J2 · (Rₑ/p)² · (5cos²i − 1)
//! Ṁ =  n · [1 + 3/4 · J2 · (Rₑ/p)² · √(1−e²) · (3cos²i − 1)]
//! ```
//!
//! with `n = √(μ/a³)` the mean motion, `p = a(1−e²)` the semi-latus rectum, and `Rₑ` the equatorial
//! radius. Note `p` rather than `a` in the factor: that is not cosmetic, it is what makes the
//! formulation behave near parabolic orbits where `a → ∞` but `p` stays finite. This crate stores `p`
//! for exactly that reason ([`crate::Elements`]), so the perturbation uses the same quantity the
//! elements already carry.
//!
//! ## What must be true when this is right
//!
//! * At **i = 90°**, `Ω̇ = 0` exactly — a polar orbit's plane does not precess. This is the cheapest
//!   possible check and it catches sign errors, factor-of-2 errors, and a `cos`/`sin` swap.
//! * At the **critical inclination** `i ≈ 63.4349°`, `ω̇ = 0` — the reason Molniya and Tundra orbits
//!   exist. An implementation that misses this cannot place a real satellite.
//! * The rates must **vanish as J2 → 0**, recovering the two-body solution exactly.
//! * The sign must be **negative for prograde orbits**: Earth's equatorial bulge pulls the ascending
//!   node westward, which is why sun-synchronous orbits need a retrograde inclination above ~97°.
//!
//! Each of those is an assertion in `tests/j2.rs`, with the tolerance written before the measurement.

use crate::{is_positive_finite, State, Vec3};
use odderon_dmath::{cos, sin, sqrt};

/// J2, the Earth's dominant oblateness harmonic, as a **positive magnitude**. Dimensionless.
///
/// This is `1.08263e-3`, and the sign convention is the one thing in this module most likely to be got
/// wrong, so it is stated here rather than left to a comment in the middle of a formula.
///
/// The conventional value is `J2 = +1.08263e-3` and the *equations* carry the negative signs
/// (`Ω̇ = −3/2 …`). Some texts instead write a negative coefficient, in which case the formula's signs
/// have to flip to compensate. Combining a negative coefficient with the negative formula double-negates
/// and produces a nodal regression of `+5°/day` where the ISS is measured at `−5°/day`, a *retrograde*
/// sun-synchronous solution at 81.8° where the real one is 98.6°, and a periapsis that retreats where it
/// should advance. All three errors are individually plausible and individually silent.
///
/// This module uses the positive magnitude with the standard equations. `tests/j2.rs` pins the
/// convention three ways: the ISS-class nodal regression must be negative, the sun-synchronous
/// inclination must land near 98.6° rather than 81.8°, and periapsis must advance below the critical
/// inclination. A sign flip fails all three.
///
/// It is a *measured property of a body*, not a tuning constant, so it belongs in a curated body
/// catalogue rather than a literal in an algorithm. Other bodies differ by orders of magnitude: Mars
/// `1.96e-3`, Jupiter `1.47e-2`, the Moon `2.03e-4`.
pub const EARTH_J2: f64 = 1.082_626_68e-3;

/// J3, the Earth's third zonal harmonic (the "pear-shape" term). Dimensionless, **signed**.
///
/// `J3 = −2.5327e-6` for Earth. The sign is negative: the Earth's northern hemisphere is slightly more
/// massive than the southern, so the gravitational potential is deeper in the north. In the potential
/// expansion `U = −μ/r · [1 − J2(R/r)²P₂ − J3(R/r)³P₃ − …]`, a negative `J3` makes the `−J3(R/r)³P₃`
/// term positive where `P₃(sin φ) > 0` (northern hemisphere), deepening the potential there — the
/// correct physical effect.
///
/// J3 is an **odd** harmonic: it breaks the north-south symmetry that J2 preserves. Its secular effect
/// on the orbit vanishes for circular orbits (a circular orbit spends equal time north and south, so
/// the effects cancel) and is proportional to eccentricity. Its short-period effect, which the
/// integrator captures, does not vanish — it produces a small north-south oscillation of the orbit.
///
/// The magnitude is roughly 430 times smaller than J2, so J3's contribution to the total field is
/// small — but over a 7-day LEO propagation it is a measurable part of the residual that the
/// J2-only secular theory leaves behind.
pub const EARTH_J3: f64 = -2.532_654_0e-6;

/// J4, the Earth's fourth zonal harmonic. Dimensionless, **signed**.
///
/// `J4 = −1.6197e-6` for Earth. Like J2, J4 is an **even** harmonic: it preserves the north-south
/// symmetry and its secular effect does **not** vanish for circular orbits. It contributes to the
/// nodal regression and periapsis rotation at the same structural level as J2, but with a coefficient
/// roughly 670 times smaller.
///
/// J4's secular rates are second-order in the potential expansion (the `(R/r)⁴` factor), so its
/// contribution is small — but it is the leading even-harmonic correction to J2, and including it
/// alongside J3 in the integrator is what the `CREDIBILITY.md` §3.1 row "J2 + J3 + J4 propagation"
/// asks for. The target there is a difference under **50 m** over 7 days at LEO, which is the
/// self-consistency test: the analytic (secular) prediction and the numerical integrator must agree
/// to that tolerance when the full field is used.
pub const EARTH_J4: f64 = -1.619_681_4e-6;

/// Earth's equatorial radius, km. The `Rₑ` in the `(Rₑ/p)²` factor.
pub const EARTH_EQUATORIAL_RADIUS: f64 = crate::radius::EARTH;

/// The secular drift rates of a J2-perturbed orbit, in radians per second.
///
/// These are *rates*, not angles. Multiplying by an elapsed time gives the first-order drift, valid
/// while the drift itself stays small compared with `2π` — at LEO a full nodal period is about 100
/// days, so anything shorter is comfortable and anything longer is not.
#[derive(Clone, Copy, Debug)]
pub struct SecularRates {
    /// Rate of the right ascension of the ascending node, rad/s. Negative for prograde orbits.
    pub raan: f64,
    /// Rate of the argument of periapsis, rad/s. Zero at the critical inclination.
    pub argp: f64,
    /// Rate of the mean anomaly, rad/s. Includes the two-body mean motion, so this is *always* positive.
    pub mean_anomaly: f64,
}

/// First-order J2 secular rates for an orbit described by elements.
///
/// # Arguments
///
/// * `a` — semi-major axis, km. Must be positive and finite (this function is for elliptic orbits;
///   `p` is derived from `a` and `e`, and a hyperbolic orbit has no secular `M`).
/// * `e` — eccentricity, dimensionless, `0 ≤ e < 1`.
/// * `i` — inclination, radians.
/// * `mu` — gravitational parameter, km³/s².
/// * `j2` — the body's oblateness coefficient as a **positive magnitude** (see [`EARTH_J2`]).
/// * `equatorial_radius` — km.
///
/// # Degenerate input
///
/// Returns `None` for input that has no meaningful answer — non-positive `a` or `mu`, `e ≥ 1`, or a
/// non-finite argument. Returning `None` rather than NaNs is deliberate here: unlike [`crate::propagate`],
/// which must survive a player handing it nonsense mid-simulation, this is a catalogue query and a
/// caller that gets `None` has asked a question with no answer.
pub fn secular_rates(
    a: f64,
    e: f64,
    i: f64,
    mu: f64,
    j2: f64,
    equatorial_radius: f64,
) -> Option<SecularRates> {
    if !is_positive_finite(a) || !is_positive_finite(mu) {
        return None;
    }
    // `contains` is false for NaN, so this covers finiteness as well as range.
    if !(0.0..1.0).contains(&e) {
        return None;
    }
    if !i.is_finite() || !j2.is_finite() || !is_positive_finite(equatorial_radius) {
        return None;
    }

    let e2 = e * e;
    let p = a * (1.0 - e2);
    if !is_positive_finite(p) {
        return None;
    }

    // mean motion
    let n = sqrt(mu / (a * a * a));

    // (Rₑ/p)² — the oblateness gets weaker as the orbit gets larger, as the fourth power of distance
    let ratio = (equatorial_radius / p) * (equatorial_radius / p);

    let cos_i = cos(i);
    let cos2_i = cos_i * cos_i;

    Some(SecularRates {
        // The minus sign is the physics: a prograde orbit's node walks westward. An implementation
        // that "fixes" this sign so sun-synchronous orbits come out right has broken the prograde case.
        raan: -1.5 * n * j2 * ratio * cos_i,
        argp: 0.75 * n * j2 * ratio * (5.0 * cos2_i - 1.0),
        mean_anomaly: n * (1.0 + 0.75 * j2 * ratio * sqrt(1.0 - e2) * (3.0 * cos2_i - 1.0)),
    })
}

/// Apply J2 secular drift to a state over `dt` seconds.
///
/// This is the closed-form propagation the catalogue actually calls: a state in, a state out, no
/// integration. It is two-body [`crate::propagate`] followed by three small rotations, which is why it
/// costs the same order of operations and can run per-frame if it ever needs to.
///
/// # Why rotation rather than element editing
///
/// The obvious implementation is: state → elements, add `rate × dt` to `Ω`, `ω` and `M`, elements →
/// state. **That implementation is wrong for every circular orbit, and circular orbits are most of the
/// catalogue.**
///
/// Classical elements are degenerate at `e = 0`: there is no periapsis, so `ω` is undefined and is
/// returned as 0 with all the in-plane information carried by the true anomaly. Adding drift to `ω` and
/// then rebuilding the state therefore adds it to a number that the library is about to ignore, and the
/// drift silently vanishes. Measured: a year of J2 at 400 km moved the node by **0.000°** — not "a
/// little", *exactly nothing* — because the circular-orbit branch zeroed the angles on the way back out.
///
/// The fix is to apply the drift as the rotations it physically is, to the vectors, where there is no
/// degeneracy to trip over:
///
/// * `ΔΩ` is a rotation of the whole orbit plane about the inertial **Z** axis.
/// * `Δω + ΔM` is a rotation **within** the plane, about the orbit normal `ĥ`.
/// * Inclination is unchanged, which first-order theory requires and which the vector form gets for free.
///
/// The two rotations do not commute, but their commutator is `O(Δ²)` and this is a first-order theory,
/// so applying them in either order changes the answer at second order only. They are applied in-plane
/// first, then about Z, which matches the order the elements are conventionally constructed in.
///
/// # What this does not do
///
/// It does not model short-period wobble. An orbit propagated this way is correct *on average* over an
/// orbit but wrong at any instant by up to the magnitude of the periodic term — tens of kilometres at
/// LEO. For a catalogue that answers "where will it be in a week" that is irrelevant; for a rendezvous
/// burn it is not, and `CREDIBILITY.md` §3.1 has a separate row for the J2+J3+J4 integrator this is not.
///
/// # Degenerate input
///
/// Returns `None` when the state has no elliptic orbit or when `dt` is not finite. A `dt` of zero is
/// valid and returns the input state exactly.
pub fn propagate_j2_secular(
    state: &State,
    dt: f64,
    mu: f64,
    j2: f64,
    equatorial_radius: f64,
) -> Option<State> {
    if !dt.is_finite() {
        return None;
    }

    let el = state.to_elements(mu);
    if !el.p.is_finite() || !(0.0..1.0).contains(&el.e) {
        return None;
    }
    let a = el.semi_major_axis();
    if !is_positive_finite(a) {
        return None;
    }

    let rates = secular_rates(a, el.e, el.i, mu, j2, equatorial_radius)?;

    // Two-body motion first, by the universal-variable propagator that already owns that problem.
    let two_body = crate::propagate(state, dt, mu);
    if two_body.unconverged || two_body.overflowed {
        return None;
    }
    let mut r = two_body.state.r;
    let mut v = two_body.state.v;

    // The two-body part of the mean-anomaly rate is now accounted for, so only the perturbation is left.
    let n = sqrt(mu / (a * a * a));
    let delta_raan = rates.raan * dt;
    let in_plane = (rates.argp + (rates.mean_anomaly - n)) * dt;

    // Normal of the *original* orbit, which is the axis the in-plane rotation turns about.
    let h = state.r.cross(state.v);
    let h_norm = h.norm();
    if !is_positive_finite(h_norm) {
        return None;
    }
    let h_hat = Vec3::new(h.x / h_norm, h.y / h_norm, h.z / h_norm);

    // In-plane rotation about ĥ, then the plane rotation about Z. Both go through one Rodrigues helper
    // so the sign convention for "positive rotation" is written down exactly once.
    r = rotate_about_axis(r, h_hat, in_plane);
    v = rotate_about_axis(v, h_hat, in_plane);
    let z = Vec3::new(0.0, 0.0, 1.0);
    r = rotate_about_axis(r, z, delta_raan);
    v = rotate_about_axis(v, z, delta_raan);

    Some(State::new(r, v))
}

/// Rodrigues' rotation of `v` about the unit axis `k` through angle `theta`.
///
/// `v cosθ + (k × v) sinθ + k (k·v)(1 − cosθ)`. The axis must be normalised by the caller.
pub(crate) fn rotate_about_axis(v: Vec3, k: Vec3, theta: f64) -> Vec3 {
    let c = cos(theta);
    let s = sin(theta);
    let kv = k.cross(v);
    let kd = k.dot(v);
    Vec3::new(
        v.x * c + kv.x * s + k.x * kd * (1.0 - c),
        v.y * c + kv.y * s + k.y * kd * (1.0 - c),
        v.z * c + kv.z * s + k.z * kd * (1.0 - c),
    )
}
