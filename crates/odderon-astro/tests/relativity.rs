//! Relativistic corrections: Schwarzschild precession of Mercury.
//!
//! `CREDIBILITY.md` §3.1 row "Relativistic corrections": the Schwarzschild precession of Mercury,
//! within 1 % of 43.0″/century.
//!
//! The tests here are not "does it run." The first test computes the full precession rate from
//! first principles — the solar gravitational parameter, Mercury's orbital elements, the speed of
//! light — and checks the result against the published value. The rest are boundary conditions that
//! the formula must satisfy: it must vanish in the Newtonian limit, it must scale correctly with
//! `p`, and it must not perturb the Newtonian propagator.

use odderon_astro::relativity::*;
use odderon_astro::{mu, Elements};
use odderon_dmath::{fabs, PI, pow, sqrt};

/// The published Schwarzschild precession of Mercury, arcseconds per century.
///
/// This is the value from general relativity: the observed periapsis precession of 574.1″/century
/// minus the 531.6″/century from planetary perturbations leaves 43.0″/century as the residual that
/// the Schwarzschild metric predicts. The 1 % acceptance bound from `CREDIBILITY.md` is 0.43″.
const MERCURY_PREDICTION_ARCSEC_PER_CENTURY: f64 = 43.0;

/// The acceptance tolerance from `CREDIBILITY.md`: within 1 % of 43.0″/century.
const ACCEPTABLE_RELATIVE_ERROR: f64 = 0.01;

// ---------------------------------------------------------------------------------------------
// The headline acceptance test
// ---------------------------------------------------------------------------------------------

/// The Schwarzschild precession of Mercury must be within 1 % of the published 43.0″/century.
///
/// This is the row's acceptance criterion, stated as a test. The computation uses the solar
/// gravitational parameter, Mercury's semi-major axis and eccentricity, and Mercury's orbital
/// period — all of which are published constants. There are no fitted parameters.
#[test]
fn mercury_precession_matches_published_value() {
    let computed = mercury_precession_arcsec_per_century();
    let relative_error = fabs(computed - MERCURY_PREDICTION_ARCSEC_PER_CENTURY)
        / MERCURY_PREDICTION_ARCSEC_PER_CENTURY;

    assert!(
        relative_error < ACCEPTABLE_RELATIVE_ERROR,
        "Schwarzschild precession of Mercury: computed {computed:.4}″/century, \
         expected {MERCURY_PREDICTION_ARCSEC_PER_CENTURY}″/century, \
         relative error {:.4}",
        relative_error * 100.0
    );
}

/// The same computation, decomposed: the per-orbit advance, the number of orbits per century, and
/// the product. This is the intermediate form that makes it possible to see *where* a discrepancy
/// would come from — an error in the per-orbit formula versus an error in the period.
#[test]
fn mercury_precession_decomposition() {
    let advance_per_orbit = schwarzschild_advance_per_orbit_arcsec(
        mu::SUN,
        MERCURY_SEMI_MAJOR_AXIS,
        MERCURY_ECCENTRICITY,
    );
    let orbits_per_century =
        SECONDS_PER_CENTURY / (MERCURY_PERIOD_DAYS * SECONDS_PER_DAY);
    let total = advance_per_orbit * orbits_per_century;

    // The decomposition must agree with the direct function.
    let direct = mercury_precession_arcsec_per_century();
    assert!(
        fabs(total - direct) < 1e-6,
        "decomposition {total} disagrees with direct {direct}"
    );

    // Each factor must be physically sensible.
    assert!(advance_per_orbit > 0.0, "per-orbit advance must be positive");
    assert!(
        orbits_per_century > 400.0 && orbits_per_century < 430.0,
        "Mercury has ~415 orbits per century, got {orbits_per_century}"
    );
    // ~43″/century ÷ ~415 orbits/century ≈ 0.1″ per orbit.
    assert!(
        advance_per_orbit > 0.05 && advance_per_orbit < 0.2,
        "per-orbit advance should be ~0.1″, got {advance_per_orbit}"
    );
}

// ---------------------------------------------------------------------------------------------
// Boundary conditions
// ---------------------------------------------------------------------------------------------

/// The correction must vanish when there is no mass: `GM → 0` means no gravity and no precession.
///
/// This catches a missing factor of `mu` in the numerator — the most likely way to get a wrong
/// magnitude while still having a plausible-looking formula.
#[test]
fn correction_vanishes_without_gravity() {
    let zero_mu_result = schwarzschild_advance_per_orbit(0.0, MERCURY_SEMI_MAJOR_AXIS, 0.2);
    assert!(
        zero_mu_result == 0.0,
        "with GM = 0 the precession must be exactly zero, got {zero_mu_result}"
    );

    let tiny_mu = 1e-12;
    let tiny_result = schwarzschild_advance_per_orbit(tiny_mu, MERCURY_SEMI_MAJOR_AXIS, 0.2);
    assert!(
        tiny_result < 1e-9,
        "with tiny GM the precession must be negligible, got {tiny_result}"
    );
}

/// The correction must vanish in the Newtonian limit: `c → ∞` means the relativistic term
/// disappears. This is the "must not break Newtonian cases" requirement expressed as a boundary
/// condition on the formula.
#[test]
fn correction_vanishes_in_newtonian_limit() {
    // The formula is proportional to 1/c². We cannot set c to infinity, but we can verify the
    // scaling: doubling c reduces the correction by exactly 4×.
    let c = C_LIGHT;
    let mu_val = mu::SUN;
    let a = MERCURY_SEMI_MAJOR_AXIS;
    let e = MERCURY_ECCENTRICITY;

    // Recompute with a doubled c by using the formula directly.
    let advance_c = 6.0 * PI * mu_val / (c * c * a * (1.0 - e * e));
    let advance_2c = 6.0 * PI * mu_val / ((2.0 * c) * (2.0 * c) * a * (1.0 - e * e));
    assert!(
        fabs(advance_c / advance_2c - 4.0) < 1e-12,
        "doubling c must reduce the correction by 4×, got ratio {}",
        advance_c / advance_2c
    );

    // And the function itself agrees with the hand-computed value.
    let computed = schwarzschild_advance_per_orbit(mu_val, a, e);
    assert!(
        fabs(computed - advance_c) < 1e-15,
        "function result {computed} disagrees with direct formula {advance_c}"
    );
}

/// The correction must scale as `1/p`: a more distant orbit precesses less. This is the
/// `1/(1−e²)` dependence — the reason the effect is measurable for Mercury and invisible for
/// Neptune.
#[test]
fn correction_scales_inversely_with_semi_latus_rectum() {
    let mu_val = mu::SUN;

    // Same `p`, different `a` and `e`: the correction must be identical.
    // Orbit 1: a = 50000, e = 0.4 → p = 50000·(1−0.16) = 42000
    // Orbit 2: a = 60000, e = 0.3 → p = 60000·(1−0.09) = 54600
    // These have different `p`, so let's instead compare orbits with the same `p`:
    // Orbit A: a = 42000, e = 0 → p = 42000
    // Orbit B: a = 84000, e = √(1−42000/84000) = √0.5 ≈ 0.7071 → p = 84000·0.5 = 42000
    let advance_a = schwarzschild_advance_per_orbit(mu_val, 42000.0, 0.0);
    let advance_b = schwarzschild_advance_per_orbit(mu_val, 84000.0, sqrt(0.5));
    assert!(
        fabs(advance_a - advance_b) < 1e-10,
        "orbits with the same p must have the same advance: {advance_a} vs {advance_b}"
    );

    // At the same `a`, an eccentric orbit has a smaller `p` (p = a(1−e²)), so it precesses
    // faster. The ratio of the eccentric advance to the circular advance must be 1/(1−e²).
    let advance_circular = schwarzschild_advance_per_orbit(mu_val, 57_909_050.0, 0.0);
    let advance_mercury = schwarzschild_advance_per_orbit(mu_val, 57_909_050.0, 0.205_630);
    assert!(
        advance_mercury > advance_circular,
        "eccentric orbit must precess faster than circular at the same a \
         (eccentric p is smaller)"
    );

    // The ratio must equal 1/(1−e²).
    let ratio = advance_mercury / advance_circular;
    let expected_ratio = 1.0 / (1.0 - MERCURY_ECCENTRICITY * MERCURY_ECCENTRICITY);
    assert!(
        fabs(ratio - expected_ratio) < 1e-12,
        "ratio {ratio} must equal 1/(1−e²) = {expected_ratio}"
    );
}

// ---------------------------------------------------------------------------------------------
// Period and rate checks
// ---------------------------------------------------------------------------------------------

/// The precession rate (radians per second) must be the per-orbit advance divided by the period.
#[test]
fn precession_rate_is_per_orbit_over_period() {
    let mu_val = mu::SUN;
    let a = MERCURY_SEMI_MAJOR_AXIS;
    let e = MERCURY_ECCENTRICITY;

    let rate = schwarzschild_precession_rate(mu_val, a, e);
    let advance = schwarzschild_advance_per_orbit(mu_val, a, e);
    // The rate function divides by the Keplerian period 2π√(a³/μ). The published sidereal period
    // (87.9691 days) differs slightly because Mercury's actual period includes planetary
    // perturbations. Use the Keplerian period for the consistency check.
    let kepler_period = 2.0 * PI * sqrt(pow(a, 3.0) / mu_val);
    assert!(
        fabs(rate - advance / kepler_period) < 1e-25,
        "rate {rate} must equal advance/kepler_period = {:.25}",
        advance / kepler_period
    );
}

/// The advance over an interval must be the rate times the interval.
#[test]
fn advance_over_interval_is_rate_times_dt() {
    let mu_val = mu::SUN;
    let a = MERCURY_SEMI_MAJOR_AXIS;
    let e = MERCURY_ECCENTRICITY;
    let dt = SECONDS_PER_CENTURY;

    let advance = advance_over_interval(mu_val, a, e, dt);
    let rate = schwarzschild_precession_rate(mu_val, a, e);
    assert!(
        fabs(advance - rate * dt) < 1e-15,
        "advance {advance} must equal rate × dt"
    );
}

// ---------------------------------------------------------------------------------------------
// Application to elements
// ---------------------------------------------------------------------------------------------

/// Applying the correction must rotate the argument of periapsis without changing anything else.
///
/// This is the "optionally model it without breaking Newtonian cases" requirement: the caller can
/// apply the correction to an `Elements` set and the rest of the orbit is untouched.
#[test]
fn apply_advance_rotates_only_argp() {
    let el = Elements {
        p: MERCURY_SEMI_MAJOR_AXIS * (1.0 - MERCURY_ECCENTRICITY * MERCURY_ECCENTRICITY),
        e: MERCURY_ECCENTRICITY,
        i: 0.1,
        raan: 0.5,
        argp: 1.0,
        nu: 2.0,
        degeneracy: Default::default(),
    };

    let advanced = apply_advance(&el, mu::SUN, SECONDS_PER_CENTURY);

    // The advance must be exactly the computed Schwarzschild angle.
    let expected_advance = advance_over_interval(
        mu::SUN,
        MERCURY_SEMI_MAJOR_AXIS,
        MERCURY_ECCENTRICITY,
        SECONDS_PER_CENTURY,
    );
    assert!(
        fabs(advanced.argp - (el.argp + expected_advance)) < 1e-15,
        "argp advance must match the computed value"
    );

    // Everything else must be unchanged.
    assert!(advanced.p == el.p, "p must be unchanged");
    assert!(advanced.e == el.e, "e must be unchanged");
    assert!(advanced.i == el.i, "i must be unchanged");
    assert!(advanced.raan == el.raan, "raan must be unchanged");
    assert!(advanced.nu == el.nu, "nu must be unchanged");
}

/// Applying the correction over zero time must be a no-op.
#[test]
fn apply_advance_zero_time_is_noop() {
    let el = Elements {
        p: 42_000.0,
        e: 0.3,
        i: 0.5,
        raan: 1.0,
        argp: 2.0,
        nu: 0.5,
        degeneracy: Default::default(),
    };

    let unchanged = apply_advance(&el, mu::SUN, 0.0);
    assert!(unchanged.argp == el.argp, "zero-time advance must be a no-op");
    assert!(unchanged.p == el.p);
    assert!(unchanged.e == el.e);
}

// ---------------------------------------------------------------------------------------------
// Correction magnitude diagnostic
// ---------------------------------------------------------------------------------------------

/// The correction fraction must be a small positive number, consistent with the `1/c²` structure.
#[test]
fn correction_fraction_is_small_and_positive() {
    let fraction = correction_fraction(mu::SUN, MERCURY_SEMI_MAJOR_AXIS, MERCURY_ECCENTRICITY);
    assert!(
        fraction > 0.0,
        "correction fraction must be positive, got {fraction}"
    );
    assert!(
        fraction < 1e-6,
        "correction fraction must be tiny (~1e-8 for Mercury), got {fraction}"
    );
    // The fraction is the advance per orbit divided by 2π, so it must be positive and small.
    let advance = schwarzschild_advance_per_orbit(mu::SUN, MERCURY_SEMI_MAJOR_AXIS, 0.205_630);
    let expected = advance / (2.0 * PI);
    assert!(
        fabs(fraction - expected) < 1e-15,
        "correction_fraction must equal advance/2π"
    );
}

/// A smaller semi-latus rectum (closer orbit) gives a larger correction.
#[test]
fn correction_decreases_with_distance() {
    let mu_val = mu::SUN;
    let e = 0.2;

    // Mercury-like: p ≈ 45.6 million km
    let mercury_p = MERCURY_SEMI_MAJOR_AXIS * (1.0 - e * e);
    // Earth-like: p ≈ 149.6 million km
    let earth_p = 149_597_870.0 * (1.0 - e * e);

    let mercury_advance = schwarzschild_advance_per_orbit(mu_val, mercury_p / (1.0 - e * e), e);
    let earth_advance = schwarzschild_advance_per_orbit(mu_val, earth_p / (1.0 - e * e), e);

    assert!(
        mercury_advance > earth_advance,
        "Mercury's correction ({mercury_advance}) must exceed Earth's ({earth_advance})"
    );
    // The ratio must be earth_p / mercury_p.
    let ratio = mercury_advance / earth_advance;
    let expected = earth_p / mercury_p;
    assert!(
        fabs(ratio - expected) < 1e-10,
        "ratio {ratio} must equal {expected}"
    );
}
