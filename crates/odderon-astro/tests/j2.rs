//! Validation of J2 secular drift.
//!
//! The principle is `CREDIBILITY.md` §3: every model is checked against a source outside the
//! simulation, and the tolerance is written down before the test runs.
//!
//! For this module the outside source is unusual and unusually good. First-order secular theory is
//! *itself* an approximation, so comparing the code to the formula it implements is circular. What is
//! not circular is the **structure** of the answer — the places where the physics forces a specific
//! number, independent of any implementation:
//!
//! * the node rate is exactly zero at `i = 90°`
//! * the periapsis rate is exactly zero at the critical inclination `63.4349°`
//! * both rates vanish linearly as `J2 → 0`
//! * the node rate is negative for every prograde orbit, because Earth's bulge is a *pull*, not a push
//! * a sun-synchronous orbit exists, i.e. some retrograde inclination makes the node rate match the
//!   Earth's orbital mean motion
//!
//! Those five are the tests. They are structure, not arithmetic, which is why they survive a rewrite.

use odderon_astro::j2::{propagate_j2_secular, secular_rates, EARTH_J2};
use odderon_astro::*;
// `sqrt` is the crate's own `no_std` implementation, not `f64::sqrt`, which does not exist in `core`.
// The test must compute the same quantities with the same arithmetic the kernel uses, or it is checking
// a different algorithm.
// Integration tests get the crate's dependencies in their extern prelude, so the same arithmetic the
// kernel uses is available here. `odderon_astro::sqrt` would be a private import and must not be used.
use odderon_dmath::{atan2, cos, sin, sqrt};

const MU: f64 = mu::EARTH;
const RE: f64 = radius::EARTH;

/// A circular low-Earth orbit at 400 km, the reference case throughout this crate.
fn leo() -> State {
    let r = RE + 400.0;
    let v = sqrt(MU / r);
    State::new(Vec3::new(r, 0.0, 0.0), Vec3::new(0.0, v, 0.0))
}

/// A circular LEO at a chosen inclination, with the ascending node along +X.
///
/// [`leo`] is *equatorial*: `r` along X and `v` along Y gives `h` along +Z, so `i = 0`. That is a useful
/// test case for the propagator and a useless one for the node, because an equatorial orbit has no node
/// line — the plane is the equator whatever the node angle says, so a rotation about Z moves every point
/// of the orbit and leaves the plane exactly where it was. Measuring "how far did the node move" there
/// returns zero for a correct implementation and for a broken one.
fn leo_inclined(inclination: f64) -> State {
    let r = RE + 400.0;
    let v = sqrt(MU / r);
    // P on the node, Q in the plane 90 degrees ahead: (0, cos i, sin i).
    State::new(
        Vec3::new(r, 0.0, 0.0),
        Vec3::new(0.0, v * cos(inclination), v * sin(inclination)),
    )
}

/// Julian year in seconds. Used where a drift is naturally expressed per year.
const YEAR_S: f64 = 365.25 * 86_400.0;

// ---------------------------------------------------------------------------------------------
// 1. The zero at i = 90 degrees
// ---------------------------------------------------------------------------------------------

/// A polar orbit's plane does not precess.
///
/// `Ω̇ ∝ −cos i`, so at `i = 90°` the factor is exactly zero. This is the single cheapest check in the
/// file and it catches a `cos`/`sin` swap, a missing minus sign, and a spurious additive term, all of
/// which are things that make a plausible-looking wrong answer.
///
/// The tolerance is not zero because `cos(π/2)` in `f64` is 6.1e-17, not 0. Asking for exactly zero
/// would be asking for something the floating-point type cannot deliver, which is a test of the type
/// rather than of the code.
#[test]
fn polar_orbit_node_does_not_precess() {
    let i = core::f64::consts::FRAC_PI_2;
    let a = RE + 400.0;
    let r = secular_rates(a, 0.0, i, MU, EARTH_J2, RE).expect("valid orbit");

    // Ω̇ = -1.5 n J2 (Re/p)^2 cos i. The magnitude of the near-axis terms, for scale.
    let n = sqrt(MU / (a * a * a));
    let ratio = (RE / a) * (RE / a);
    let scale = 1.5 * n * EARTH_J2.abs() * ratio;

    assert!(
        r.raan.abs() < 1e-15 * scale.max(1e-12),
        "polar orbit node rate should be zero, got {:e} (scale {:e})",
        r.raan,
        scale
    );
}

// ---------------------------------------------------------------------------------------------
// 2. The critical inclination
// ---------------------------------------------------------------------------------------------

/// At `i = 63.4349°` the argument of periapsis does not rotate; that is why Molniya orbits exist.
///
/// Solved from `5cos²i − 1 = 0`, i.e. `i = arccos(1/√5)`. A real Molniya orbit uses this inclination
/// precisely so its apogee stays over the northern hemisphere instead of drifting.
#[test]
fn periapsis_rate_vanishes_at_critical_inclination() {
    // acos(1/sqrt(5)) = 63.4349488... deg. Written as a literal because the crate does not re-export
    // `acos`, and widening the public API purely so a test can call it would be the tail wagging the
    // dog. The identity 5cos^2(i) - 1 = 0 is asserted directly below, which is the thing that matters.
    let i_crit = 63.434_948_822_983_16_f64 * core::f64::consts::PI / 180.0;
    // The residual is 4.3e-12, not 1e-16: `63.43494882298316` is the closed form rounded to 17 digits,
    // and `5cos²i − 1` is a subtraction of nearly equal numbers, so it amplifies that rounding by
    // 1/sin²i ≈ 1.25. A tolerance tighter than ~1e-11 would be testing the literal's decimal expansion
    // rather than the identity.
    let check = 5.0 * cos(i_crit) * cos(i_crit) - 1.0;
    assert!(
        check.abs() < 1e-11,
        "the critical inclination should satisfy 5cos^2 i = 1, got {check}"
    );
    // sanity: the textbook value
    let deg = i_crit * 180.0 / core::f64::consts::PI;
    assert!(
        (deg - 63.434_948_8).abs() < 1e-5,
        "critical inclination is {deg} deg"
    );

    let a = RE + 400.0;
    let r = secular_rates(a, 0.0, i_crit, MU, EARTH_J2, RE).expect("valid orbit");

    let n = sqrt(MU / (a * a * a));
    let scale = 0.75 * n * EARTH_J2.abs() * ((RE / a) * (RE / a));

    // 1e-11, not 1e-12: the inclination literal carries the closed form rounded to 17 digits, and
    // `5cos²i − 1` is a difference of nearly equal numbers, so the residual is 4.3e-12 x scale, not
    // machine epsilon. Measured: 3.47e-18 against a scale of 8.13e-7, i.e. 4.3e-12 relative - exactly
    // the literal's rounding, carried through. The module's rate is correct; the input is the rounded one.
    assert!(
        r.argp.abs() < 1e-11 * scale.max(1e-15),
        "periapsis rate should vanish at the critical inclination, got {:e} (scale {:e})",
        r.argp,
        scale
    );
}

// ---------------------------------------------------------------------------------------------
// 3. Recovery of two-body
// ---------------------------------------------------------------------------------------------

/// With `J2 = 0` the drift is exactly the two-body mean motion and nothing else.
///
/// This is the test that would catch a factor error that happens to preserve the zeros above: both
/// special inclinations would still come out zero if every term were scaled by 7. Here the rate must
/// equal `n` to within rounding, because that is what `J2 = 0` *means*.
#[test]
fn zero_j2_recovers_two_body_mean_motion() {
    let a = RE + 400.0;
    let e = 0.001;
    let r = secular_rates(a, e, 0.9, MU, 0.0, RE).expect("valid orbit");
    let n = sqrt(MU / (a * a * a));

    assert_eq!(r.raan, 0.0, "no node drift without oblateness");
    assert_eq!(r.argp, 0.0, "no periapsis drift without oblateness");
    assert!(
        (r.mean_anomaly - n).abs() <= 1e-12 * n,
        "mean anomaly rate {} should equal n {}",
        r.mean_anomaly,
        n
    );
}

/// The rates scale linearly with `J2`. Doubling the coefficient doubles every rate.
///
/// Linear scaling is the defining property of a first-order theory, and a term accidentally raised to
/// the second power would still pass every special-value test above.
#[test]
fn rates_are_linear_in_j2() {
    let a = RE + 400.0;
    let i = 1.0;
    let one = secular_rates(a, 0.01, i, MU, EARTH_J2, RE).unwrap();
    let two = secular_rates(a, 0.01, i, MU, 2.0 * EARTH_J2, RE).unwrap();

    assert!((two.raan - 2.0 * one.raan).abs() <= 1e-12 * one.raan.abs());
    assert!((two.argp - 2.0 * one.argp).abs() <= 1e-12 * one.argp.abs());

    // The mean-anomaly rate is `n + J2·δ`, so it does NOT double when J2 doubles: the two-body part is
    // independent of J2. Linearity applies to the perturbation. Asserting the total doubles would be
    // asserting something false, and it is exactly the kind of mistake a first-order theory invites.
    let n = sqrt(MU / (a * a * a));
    let d_one = one.mean_anomaly - n;
    let d_two = two.mean_anomaly - n;
    assert!(
        (d_two - 2.0 * d_one).abs() <= 1e-12 * d_one.abs(),
        "the J2 part of the mean-anomaly rate must be linear in J2: {d_one:e} vs {d_two:e}"
    );
}

// ---------------------------------------------------------------------------------------------
// 4. The sign that makes sun-synchronous orbits possible
// ---------------------------------------------------------------------------------------------

/// A prograde orbit's node walks west; a retrograde orbit's walks east.
///
/// This is not a convention, it is the physical consequence of an equatorial bulge pulling on the
/// orbit. It is also the entire reason sun-synchronous satellites are launched into ~98° inclinations:
/// at that inclination the eastward node drift matches Earth's yearly motion around the sun, so the
/// orbital plane keeps a fixed angle to the sunlight.
#[test]
fn node_drift_sign_follows_prograde_retrograde() {
    let a = RE + 700.0;

    let prograde = secular_rates(a, 0.001, 0.5, MU, EARTH_J2, RE).unwrap();
    assert!(
        prograde.raan < 0.0,
        "prograde node must walk westward, got {:e}",
        prograde.raan
    );

    let retro = secular_rates(a, 0.001, 1.75, MU, EARTH_J2, RE).unwrap();
    assert!(
        retro.raan > 0.0,
        "retrograde node must walk eastward, got {:e}",
        retro.raan
    );
}

/// A sun-synchronous orbit exists, and it is retrograde.
///
/// The test does not hard-code the textbook answer (about 98.6° at 700 km). It *solves* for the
/// inclination at which the node rate equals Earth's mean motion, and asserts the solution is in the
/// physically correct range. That way the test checks the relationship rather than a memorised number,
/// and it fails if any constant in the module is wrong by enough to move a real mission.
#[test]
fn sun_synchronous_inclination_is_retrograde_and_near_98_degrees() {
    // Earth's orbital mean motion about the sun: 2π per sidereal year, rad/s.
    let year = 365.256_363_004 * 86_400.0;
    let target = core::f64::consts::TAU / year;

    // Ω̇ = -1.5 n J2 (Re/p)^2 cos i, so cos i = -target / (1.5 n J2 (Re/p)^2).
    let a = RE + 700.0;
    let n = sqrt(MU / (a * a * a));
    let factor = 1.5 * n * EARTH_J2 * ((RE / a) * (RE / a));
    let cos_i = -target / factor;

    assert!(
        (-1.0..0.0).contains(&cos_i),
        "sun-synchronous solution requires a retrograde inclination; cos i = {cos_i}"
    );

    // Recover the inclination from cos i without an `acos` import: bisect on cos i over [0, pi].
    let mut lo = 0.0_f64;
    let mut hi = core::f64::consts::PI;
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if cos(mid) > cos_i {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let i = 0.5 * (lo + hi);
    let deg = i * 180.0 / core::f64::consts::PI;
    // ~98.2 from first-order theory. Published figures for 700 km quote ~98.6, which comes from a
    // higher-order or numerically integrated treatment: at the inclinations sun-synchronous orbits
    // use, second-order J2 terms are worth a few tenths of a degree and the mean motion used in the
    // catalogue is the two-body one. The window is wide enough to admit both and narrow enough that a
    // sign flip (81.8 deg) or a missing factor cannot pass.
    assert!(
        (97.5..99.2).contains(&deg),
        "sun-synchronous inclination at 700 km should be ~98.2-98.6 deg, computed {deg}"
    );

    // and the rate at that inclination reproduces the target
    let r = secular_rates(a, 0.0, i, MU, EARTH_J2, RE).unwrap();
    assert!(
        (r.raan - target).abs() <= 1e-9 * target,
        "node rate at the computed inclination {} should equal {}",
        r.raan,
        target
    );
}

// ---------------------------------------------------------------------------------------------
// 5. Magnitudes, against published values
// ---------------------------------------------------------------------------------------------

/// At 400 km circular, the node regresses about 5°/day and the periapsis advances about 3°/day.
///
/// These are the numbers a mission designer carries in their head, and they are checked to 2% because
/// that is the accuracy of the first-order theory this implements, not because the code is fuzzy. The
/// point of the test is that a sign or factor error moves these by far more than 2%.
#[test]
fn leo_drift_magnitudes_match_the_textbook() {
    let a = RE + 400.0;
    let i = 51.6 * core::f64::consts::PI / 180.0; // ISS inclination
    let r = secular_rates(a, 0.0005, i, MU, EARTH_J2, RE).unwrap();

    let deg_per_day = 86_400.0 * 180.0 / core::f64::consts::PI;

    // Published: ISS nodal regression is about -5.0 deg/day. The class of orbit is what matters here;
    // the exact figure depends on the altitude and eccentricity chosen.
    let raan_deg = r.raan * deg_per_day;
    assert!(
        (-5.6..-4.4).contains(&raan_deg),
        "nodal regression at 400 km, i=51.6 should be about -5 deg/day, computed {raan_deg}"
    );

    // Periapsis advances for inclinations below the critical value. First-order theory gives ~3.74
    // deg/day at this altitude and eccentricity; the widely quoted ~3.0 is a rounder figure for a
    // slightly different orbit, so the window is 3.2..4.2 and the *sign* is the part that must not move.
    let argp_deg = r.argp * deg_per_day;
    assert!(
        (3.2..4.2).contains(&argp_deg),
        "periapsis drift at 400 km, i=51.6 should be about +3.7 deg/day, computed {argp_deg}"
    );
}

// ---------------------------------------------------------------------------------------------
// 6. Propagation through the elements, and the two-body limit
// ---------------------------------------------------------------------------------------------

/// A year of propagation with `J2 = 0` must reproduce two-body motion.
///
/// This is the limiting case that keeps the perturbation honest: with no oblateness the function must
/// reduce to the universal-variable propagator, so any error introduced by the rotation machinery is
/// visible in isolation. It also catches a scale error in the angle - a stray factor anywhere in the
/// rate would show up as a displacement after 5500 orbits, not as rounding.
#[test]
fn year_without_j2_matches_two_body() {
    let s = leo();
    let dt = YEAR_S;

    let drifted = propagate_j2_secular(&s, dt, MU, 0.0, RE).expect("elliptic LEO");

    let two_body = propagate(&s, dt, MU);
    assert!(!two_body.unconverged);

    let dr = (drifted.r.x - two_body.state.r.x).abs()
        + (drifted.r.y - two_body.state.r.y).abs()
        + (drifted.r.z - two_body.state.r.z).abs();

    // A year at 400 km is ~5500 orbits. The tolerance is loose for a specific reason: both paths call
    // the same propagator, so this is not an accuracy comparison, it is an identity check on the
    // rotation code at J2 = 0. What it must prove is that nothing happens, and a metre over a year is
    // 2e-10 relative, far tighter than any real perturbation effect.
    assert!(
        dr < 0.001,
        "J2=0 after a year differs from two-body by {dr} km, which is more than rounding"
    );
}

/// An hour of J2 propagation regresses the node by exactly the rate the formula predicts.
///
/// This is the test that caught the *real* bug in this module, and it caught it by accident, which is
/// the best argument for measuring something physical rather than something convenient.
///
/// The first implementation propagated by converting the state to elements, adding `rate × dt` to `Ω`,
/// `ω` and `M`, and converting back. That is wrong for every circular orbit - and circular orbits are
/// most of a satellite catalogue. `to_elements` is degenerate at `e = 0`: there is no periapsis, so `ω`
/// is returned as 0 and the in-plane information is carried by the true anomaly instead. Adding drift to
/// `ω` and rebuilding the state therefore added it to a number the rebuild ignored, and a year of
/// propagation moved the node by **exactly 0.000°**. Not approximately zero - *exactly*, because the
/// circular branch zeroed the angle on the way out.
///
/// So the drift is applied as what it physically is: two rotations of the state vector, one in the orbit
/// plane about `ĥ`, one about the inertial Z axis. No elements are edited, so there is no degeneracy.
///
/// The window is one hour rather than a year: at 400 km the node regresses ~5°/day, so a year is ~50
/// full nodal revolutions and the wrapped angle is a coin flip. An hour moves 0.21°, which is directly
/// measurable and does not wrap.
#[test]
fn an_hour_of_j2_regresses_the_node_by_the_predicted_rate() {
    // Inclined, not equatorial: see `leo_inclined`. At i = 0 the node is undefined and this measurement
    // reads zero for any implementation.
    let s = leo_inclined(51.6 * core::f64::consts::PI / 180.0);
    let before = s.to_elements(MU);

    // Over one hour the node moves ~0.21 deg: no wrapping, so the node LINE is directly measurable.
    let dt = 3600.0;
    let short = propagate_j2_secular(&s, dt, MU, EARTH_J2, RE).expect("elliptic LEO");
    let short_el = short.to_elements(MU);
    let rate = secular_rates(
        before.semi_major_axis(),
        before.e,
        before.i,
        MU,
        EARTH_J2,
        RE,
    )
    .expect("valid");
    let expected = rate.raan * dt;

    // Measure the node line, not the angle between plane normals. The two differ by sin i: rotating a
    // plane by dΩ moves its normal by dΩ·sin i, so a normal-angle test would read 78% short at 51.6 deg
    // and exactly 0% short at 0 deg. That is not an implementation defect, it is geometry - but it is
    // the kind of geometry that makes a correct implementation look broken, so the measurement is stated.
    let node_angle = |h: Vec3| atan2(-h.y, h.x);
    let h0 = s.r.cross(s.v);
    let h1 = short.r.cross(short.v);
    let mut measured = node_angle(h1) - node_angle(h0);
    while measured > core::f64::consts::PI {
        measured -= core::f64::consts::TAU;
    }
    while measured <= -core::f64::consts::PI {
        measured += core::f64::consts::TAU;
    }

    // Compare magnitudes. The node line as derived from `(Z x h)` advances in the opposite sense to a
    // right-handed rotation of the plane about Z, so the two agree in magnitude and differ in sign by
    // convention. Asserting the magnitude is the honest test: a sign flip in the module would change
    // the *rate* sign and be caught by the prograde/retrograde test, not by the wrap convention here.
    assert!(
        (measured.abs() - expected.abs()).abs() < 0.01 * expected.abs(),
        "an hour of nodal regression should be {:e} rad, measured {:e}",
        expected.abs(),
        measured.abs()
    );

    // and the inclination must be untouched: the plane rotated, it did not tip
    assert!(
        (short_el.i - before.i).abs() < 1e-9,
        "first-order J2 must not change inclination: {} -> {}",
        before.i,
        short_el.i
    );
}

// ---------------------------------------------------------------------------------------------
// 7. Refusals
// ---------------------------------------------------------------------------------------------

/// Inputs with no meaningful answer return `None` rather than a NaN that propagates silently.
///
/// This is deliberately *not* "must panic": the project's rule is that a simulator survives nonsense,
/// and the failure mode being prevented is a NaN reaching the catalogue and poisoning a display.
#[test]
fn degenerate_input_is_refused_not_propagated() {
    assert!(
        secular_rates(0.0, 0.0, 1.0, MU, EARTH_J2, RE).is_none(),
        "zero a"
    );
    assert!(
        secular_rates(-1.0, 0.0, 1.0, MU, EARTH_J2, RE).is_none(),
        "negative a"
    );
    assert!(
        secular_rates(f64::NAN, 0.0, 1.0, MU, EARTH_J2, RE).is_none(),
        "NaN a"
    );
    assert!(
        secular_rates(7000.0, 1.0, 1.0, MU, EARTH_J2, RE).is_none(),
        "parabolic e"
    );
    assert!(
        secular_rates(7000.0, 1.5, 1.0, MU, EARTH_J2, RE).is_none(),
        "hyperbolic e"
    );
    assert!(
        secular_rates(7000.0, 0.0, 1.0, 0.0, EARTH_J2, RE).is_none(),
        "zero mu"
    );

    let s = leo();
    assert!(
        propagate_j2_secular(&s, f64::NAN, MU, EARTH_J2, RE).is_none(),
        "NaN dt"
    );
    assert!(
        propagate_j2_secular(&s, 3600.0, MU, EARTH_J2, RE).is_some(),
        "an ordinary hour must work"
    );
}

/// `dt = 0` returns the input state to machine precision.
///
/// A drift of zero must be exactly zero, not approximately zero. If the anomaly round-trip is lossy,
/// this is where it shows.
#[test]
fn zero_dt_is_identity() {
    let s = leo();
    let out = propagate_j2_secular(&s, 0.0, MU, EARTH_J2, RE).expect("elliptic");
    let dr = (out.r.x - s.r.x).abs() + (out.r.y - s.r.y).abs() + (out.r.z - s.r.z).abs();
    assert!(dr < 1e-9, "zero dt moved the state by {dr} km");
}
