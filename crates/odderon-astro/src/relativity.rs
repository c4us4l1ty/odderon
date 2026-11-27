//! Relativistic corrections — the Schwarzschild precession of the periapsis.
//!
//! ## Why this exists
//!
//! Newton's two-body problem closes: the periapsis of an ellipse stays put forever. The general
//! relativity of a point mass says it does not. A test particle orbiting a mass `M` has its
//! periapsis advance by a small but measurable amount each orbit:
//!
//! ```text
//! Δω = 6π · GM / (c² · p)   per orbit
//! ```
//!
//! where `p = a(1 − e²)` is the semi-latus rectum — the same quantity [`crate::Elements`] already
//! stores. The correction depends on `p`, not on `a` and `e` separately: two orbits with the same
//! `p` precess at the same rate regardless of their eccentricity. At a fixed semi-major axis, a
//! more eccentric orbit precesses faster because its `p` is smaller, and the effect diverges as
//! `e → 1` (parabolic) where `p` approaches zero.
//!
//! The historical test case is Mercury. The observed periapsis precession is 574.1″/century, of
//! which planetary perturbations account for 531.6″; the residual **43.0″/century** is the
//! Schwarzschild prediction, and it was the first quantitative confirmation of general relativity.
//! This module reproduces it from first principles — no fitted parameters, no external ephemeris.
//!
//! ## What is and is not modelled
//!
//! * **Modelled:** the first-order Schwarzschild periapsis advance, as a pure function of the
//!   orbital elements and the gravitational parameter. This is the *only* relativistic effect that
//! has a closed-form, element-based expression; it is the row in `CREDIBILITY.md` §3.1.
//! * **Not modelled:** the Schwarzschild metric's effect on the *propagator* — i.e. we do not
//!   integrate the geodesic equation. The precession rate is a *secular* correction: it tells you
//!   how far the periapsis has drifted, and the propagator can apply it as a rotation of the
//!   argument of periapsis. We do not modify `propagate` itself, which keeps the Newtonian path
//!   untouched — the row's explicit requirement that the correction be *optional* and must not
//!   break Newtonian cases.
//! * **Not modelled:** frame-dragging (Lense–Thirring), which is ~1000× smaller for Mercury and
//!   is a separate physical effect.
//!
//! ## What must be true when this is right
//!
//! * **Mercury:** the computed value must fall within 1 % of 43.0″/century. This is the acceptance
//!   criterion from `CREDIBILITY.md` and the test that gates the row.
//! * **Vanishes as `GM → 0`:** with no mass, there is no gravity and no precession. This catches a
//!   missing factor and a sign error at once.
//! * **Vanishes as `c → ∞`:** the Newtonian limit. The correction is proportional to `1/c²`, so
//!   it must go to zero in the non-relativistic limit — the "does not break Newtonian cases"
//!   requirement, expressed as a mathematical boundary condition.
//! * **Vanishes as `p → ∞`:** a distant orbit has a small precession. The `1/p` dependence is what
//!   makes the effect measurable only for the innermost planets.
//! * **Eccentric orbits precess faster at the same `a`:** `Δω` scales as `1/p = 1/(a(1−e²))`, so
//!   at a fixed `a` the advance increases with `e`. A test that verifies the `1/(1−e²)` dependence
//!   catches a swapped numerator/denominator.
//!
//! Each of those is an assertion in `tests/relativity.rs`, with the tolerance written before the
//! measurement.

use crate::{mu, Elements};
use odderon_dmath::{pow, sqrt, PI};

/// Speed of light in vacuum, km/s. Defined by the SI second and the metre, this is exact.
pub const C_LIGHT: f64 = 299_792.458;

/// Seconds per Julian century.
pub const SECONDS_PER_CENTURY: f64 = 100.0 * 365.25 * 86_400.0;

/// Mercury's semi-major axis, km. (JPL DE 431 / IAU 2012 mean orbital elements.)
pub const MERCURY_SEMI_MAJOR_AXIS: f64 = 57_909_050.0;

/// Mercury's eccentricity. Dimensionless.
pub const MERCURY_ECCENTRICITY: f64 = 0.205_630;

/// Mercury's orbital period, days.
pub const MERCURY_PERIOD_DAYS: f64 = 87.9691;

/// Seconds per day.
pub const SECONDS_PER_DAY: f64 = 86_400.0;

/// The Schwarzschild periapsis advance per orbit, in **radians**.
///
/// ```text
/// Δω = 6π · GM / (c² · p)
/// ```
///
/// where `p = a(1 − e²)` is the semi-latus rectum. This is the exact first-order result: there are
/// no fitted parameters and no approximations beyond the point-mass assumption.
///
/// # Arguments
///
/// * `mu` — gravitational parameter, km³/s². For the Sun this is [`mu::SUN`].
/// * `a` — semi-major axis, km. Must be positive.
/// * `e` — eccentricity, dimensionless.
///
/// # Returns
///
/// The periapsis advance per orbit, in radians. Always non-negative: the advance is in the
/// direction of orbital motion, and `GM` and `c²` are positive.
pub fn schwarzschild_advance_per_orbit(mu: f64, a: f64, e: f64) -> f64 {
    let p = a * (1.0 - e * e);
    6.0 * PI * mu / (C_LIGHT * C_LIGHT * p)
}

/// The Schwarzschild periapsis advance per orbit, in **arcseconds**.
///
/// A convenience wrapper around [`schwarzschild_advance_per_orbit`] for the comparison against
/// the published Mercury value. The factor is `206264.806…` radians per arcsecond (the number of
/// arcseconds in one radian, `180·60·60/π`).
pub fn schwarzschild_advance_per_orbit_arcsec(mu: f64, a: f64, e: f64) -> f64 {
    const ARCSEC_PER_RAD: f64 = 180.0 * 60.0 * 60.0 / PI;
    schwarzschild_advance_per_orbit(mu, a, e) * ARCSEC_PER_RAD
}

/// The Schwarzschild periapsis advance **per unit time**, in radians per second.
///
/// Divides the per-orbit advance by the orbital period. The period comes from Kepler's third law,
/// `T = 2π√(a³/μ)`, which is the Newtonian value — the relativistic correction to the period is
/// a higher-order effect and would be circular to use here.
///
/// # Arguments
///
/// * `mu` — gravitational parameter, km³/s².
/// * `a` — semi-major axis, km. Must be positive.
/// * `e` — eccentricity, dimensionless.
pub fn schwarzschild_precession_rate(mu: f64, a: f64, e: f64) -> f64 {
    let period = 2.0 * PI * sqrt(pow(a, 3.0) / mu);
    schwarzschild_advance_per_orbit(mu, a, e) / period
}

/// The Schwarzschild periapsis advance per century, in **arcseconds**.
///
/// This is the quantity that `CREDIBILITY.md` §3.1 names as the acceptance target: **43.0″ per
/// century for Mercury**, within 1 %.
///
/// # Arguments
///
/// * `mu` — gravitational parameter, km³/s². For the Sun this is [`mu::SUN`].
/// * `a` — semi-major axis, km.
/// * `e` — eccentricity, dimensionless.
/// * `period_days` — orbital period in days. For Mercury this is [`MERCURY_PERIOD_DAYS`].
pub fn schwarzschild_advance_per_century_arcsec(mu: f64, a: f64, e: f64, period_days: f64) -> f64 {
    let advance_per_orbit = schwarzschild_advance_per_orbit_arcsec(mu, a, e);
    let orbits_per_century = SECONDS_PER_CENTURY / (period_days * SECONDS_PER_DAY);
    advance_per_orbit * orbits_per_century
}

/// The Schwarzschild periapsis advance of **Mercury** per century, in arcseconds.
///
/// Uses the solar gravitational parameter [`mu::SUN`], Mercury's orbital elements, and Mercury's
/// period. This is the number the `CREDIBILITY.md` row is measured against.
pub fn mercury_precession_arcsec_per_century() -> f64 {
    schwarzschild_advance_per_century_arcsec(
        mu::SUN,
        MERCURY_SEMI_MAJOR_AXIS,
        MERCURY_ECCENTRICITY,
        MERCURY_PERIOD_DAYS,
    )
}

/// The Schwarzschild periapsis advance over a time span, in **radians**.
///
/// Multiplies the precession rate by the elapsed time. This is the function that would be used to
/// *apply* the correction: the propagator computes the Newtonian state, then this returns the
/// rotation angle to add to the argument of periapsis.
///
/// # Arguments
///
/// * `mu` — gravitational parameter, km³/s².
/// * `a` — semi-major axis, km.
/// * `e` — eccentricity, dimensionless.
/// * `dt` — elapsed time, seconds.
pub fn advance_over_interval(mu: f64, a: f64, e: f64, dt: f64) -> f64 {
    schwarzschild_precession_rate(mu, a, e) * dt
}

/// The precession over an interval, applied to an [`Elements`] set.
///
/// Returns a new `Elements` with the argument of periapsis advanced by the Schwarzschild amount.
/// The orbit shape (`p`, `e`), the inclination, the node, and the mean anomaly are unchanged —
/// only `argp` rotates. This is the "optionally model it" interface: the caller can apply the
/// correction when they want it and leave the Newtonian orbit alone when they do not.
///
/// # Arguments
///
/// * `el` — the orbital elements.
/// * `mu` — gravitational parameter, km³/s².
/// * `dt` — elapsed time, seconds.
pub fn apply_advance(el: &Elements, mu: f64, dt: f64) -> Elements {
    let advance = advance_over_interval(mu, el.p / (1.0 - el.e * el.e), el.e, dt);
    Elements {
        argp: el.argp + advance,
        ..*el
    }
}

/// The magnitude of the Schwarzschild correction, as a fraction of the Newtonian periapsis angle.
///
/// This is a diagnostic: it tells you how much the correction matters for a given orbit. For
/// Mercury it is ~1.5e-8 per orbit — small enough that it is invisible over a single orbit and
/// dominant over a century. The `1/c²` structure means this ratio is the same for every orbit of
/// the same `p`, which is why the correction scales with `1/p`.
pub fn correction_fraction(mu: f64, a: f64, e: f64) -> f64 {
    schwarzschild_advance_per_orbit(mu, a, e) / (2.0 * PI)
}
