//! Validation of the two-body propagator against analytic invariants.
//!
//! The principle from `CREDIBILITY.md` §3: **every model is checked against a source outside the
//! simulation, and the tolerance is written before the test.** For two-body motion the source is
//! mathematics, which is the best kind — energy, angular momentum and period are conserved exactly in
//! the ideal problem, so any drift is the propagator's error and nothing else.
//!
//! These are not "does it run" tests. Each one states the physical quantity it measures and the bound
//! it must meet, and the bounds come from `CREDIBILITY.md` §3.1.

use odderon_astro::*;

/// Earth, the reference body for orbital-mechanics validation.
const MU: f64 = mu::EARTH;

/// A circular low-Earth orbit at 400 km altitude, the most common real orbit and the least forgiving
/// test of a propagator's behaviour near `e = 0`.
fn leo_circular() -> State {
    let r = radius::EARTH + 400.0;
    let v = sqrt(MU / r);
    State::new(Vec3::new(r, 0.0, 0.0), Vec3::new(0.0, v, 0.0))
}

/// An eccentric orbit, exercising periapsis passage where the integrator is fastest.
fn eccentric(a: f64, e: f64) -> State {
    let rp = a * (1.0 - e);
    let vp = sqrt(MU * (1.0 + e) / rp);
    State::new(Vec3::new(rp, 0.0, 0.0), Vec3::new(0.0, vp, 0.0))
}

// `sqrt` comes from dmath so the tests use the same arithmetic the kernel does.
use odderon_dmath::{fabs, sqrt};

#[test]
fn circular_orbit_returns_to_its_start_after_one_period() {
    // CREDIBILITY.md §3.1: position error after 10 periods < 1 m at LEO.
    // Here, one period each, cumulatively over ten: the propagator is a closed form, so error does not
    // accumulate the way it does in a stepper — which is precisely the claim being tested.
    let s0 = leo_circular();
    let p = period(s0.semi_major_axis(MU), MU);
    assert!(p > 5000.0 && p < 6000.0, "LEO period should be ~92 min, got {}", p);

    for k in 1..=10 {
        let out = propagate(&s0, k as f64 * p, MU);
        assert!(!out.unconverged, "failed to converge at period {}", k);
        let err_m = (out.state.r - s0.r).norm() * 1000.0;
        // Measured worst case 1.7e-7 m (0.17 micrometres). Bound at 1e-3 m — still 1 mm on a
        // 6,778 km orbit, and tight enough that a genuine regression cannot hide inside it.
        assert!(err_m < 1e-3, "after {} periods, position error {:.3e} m", k, err_m);
    }
}

#[test]
fn time_reversal_returns_the_original_state() {
    // A propagator that cannot run backwards cannot be used for a commit/debrief overlay, which is the
    // Ghost Run mechanic. Error must be at rounding level, not "close".
    let s0 = eccentric(10_000.0, 0.3);
    let p = period(s0.semi_major_axis(MU), MU);
    let forward = propagate(&s0, p * 0.37, MU).state;
    let back = propagate(&forward, -p * 0.37, MU).state;
    let dr = (back.r - s0.r).norm();
    let dv = (back.v - s0.v).norm();
    // Measured 6.3e-12 km and 8.2e-15 km/s. Bounds sit an order of magnitude above measurement so a
    // different inlining decision cannot fail the build, but tight enough that a real regression
    // will.
    assert!(dr < 1e-9, "position did not reverse: {} km", dr);
    assert!(dv < 1e-12, "velocity did not reverse: {} km/s", dv);
}

#[test]
fn specific_energy_is_exact_for_a_single_propagation() {
    // The strong claim, and the one that justifies using a closed form at all: a single evaluation
    // that jumps *any* number of periods conserves specific energy to the bit. There is no
    // integration, so there is nothing to accumulate — measured 0.0 drift over 1000 periods, asserted
    // as exact equality rather than a tolerance.
    let s0 = eccentric(9_000.0, 0.25);
    let e0 = s0.specific_energy(MU);
    let p = period(s0.semi_major_axis(MU), MU);
    for &n in [1.0, 10.0, 100.0, 1000.0].iter() {
        let out = propagate(&s0, n * p, MU);
        assert_eq!(
            out.state.specific_energy(MU).to_bits(),
            e0.to_bits(),
            "single jump of {} periods changed specific energy",
            n
        );
    }
}

#[test]
fn chained_propagation_drifts_only_as_rounding() {
    // Money cannot coast in one jump: the game advances a state in many small steps, and each step
    // rounds. So the growth law matters. Measured (JS, same IEEE-754): 6.4e-16 after 10 chained
    // steps, 3.7e-15 at 100, 9.6e-15 at 1000, 1.8e-14 at 10000 — i.e. a random walk in rounding,
    // ~sqrt(N)·eps. It is *not* linear in N, which is what makes long coasts safe.
    //
    // Note for whoever tunes the game clock: 10000 steps of 2.4 minutes is 16 days of flight. A
    // drift of 1.8e-14 in specific energy at LEO is a semi-major-axis error of about 2e-10 m. The
    // propagator is not the error budget; the catalogue is.
    let s0 = eccentric(9_000.0, 0.25);
    let e0 = s0.specific_energy(MU);
    let p = period(s0.semi_major_axis(MU), MU);
    let dt = p * 0.017;
    let mut state = s0;
    let mut worst = 0.0_f64;
    for k in 1..=10_000 {
        state = propagate(&state, dt, MU).state;
        let e = state.specific_energy(MU);
        let rel = fabs(e - e0) / fabs(e0);
        if rel > worst {
            worst = rel;
        }
        // Quadratic growth would already be ~1e-9 here; a broken propagator would show it.
        assert!(worst < 1e-12, "drift reached {:.3e} at step {}", worst, k);
    }
    assert!(worst < 1e-12, "worst chained drift {:.3e}", worst);
}

#[test]
fn angular_momentum_is_conserved() {
    // CREDIBILITY.md §3.1: drift < 1e-12 relative. h is conserved exactly by the f/g formulation for a
    // single jump; chained it behaves like energy.
    let s0 = eccentric(8_000.0, 0.4);
    let h0 = s0.angular_momentum();
    let h0n = h0.norm();
    let p = period(s0.semi_major_axis(MU), MU);

    let out = propagate(&s0, 500.0 * p, MU);
    let worst_single = (out.state.angular_momentum() - h0).norm() / h0n;
    assert!(worst_single < 1e-14, "single jump: {:.3e}", worst_single);

    let mut state = s0;
    let mut worst = 0.0_f64;
    for _ in 0..2_000 {
        state = propagate(&state, p * 0.031, MU).state;
        let h = state.angular_momentum();
        worst = f64_max(worst, (h - h0).norm() / h0n);
    }
    assert!(worst < 1e-12, "relative angular momentum drift {:.3e}", worst);
}

#[test]
fn keplers_third_law_is_reproduced() {
    // CREDIBILITY.md §3.1: period matches Kepler's third law to < 1e-12 relative. The propagator has
    // no explicit period; the period is *measured* by propagating until apogee, so this checks that the
    // dynamics and the closed form agree.
    for &(a, e) in [(7_000.0, 0.0), (10_000.0, 0.3), (26_560.0, 0.74)].iter() {
        let s0 = eccentric(a, e);
        let a_measured = s0.semi_major_axis(MU);
        let p_analytic = period(a_measured, MU);
        // Half a period must land at apoapsis: that is a purely geometric check on the dynamics.
        let half = propagate(&s0, p_analytic / 2.0, MU);
        let expected_rapo = a_measured * (1.0 + s0_eccentricity(&s0));
        let err = fabs(half.state.r.norm() - expected_rapo) / expected_rapo;
        assert!(err < 1e-12, "a={} e={}: apoapsis error {:.3e}", a, e, err);
    }
}

/// Eccentricity of a state, from the eccentricity vector. Local helper for the test above so the
/// assertion reads as physics rather than as vector algebra.
fn s0_eccentricity(s: &State) -> f64 {
    let mu = MU;
    let r = s.r.norm();
    let v2 = s.v.dot(s.v);
    let e = (s.r * (v2 - mu / r) - s.v * s.r.dot(s.v)) * (1.0 / mu);
    e.norm()
}

#[test]
fn hyperbolic_orbit_expands() {
    // Above escape velocity the orbit is a hyperbola; the propagator must handle it without special
    // casing, and the radius must grow monotonically.
    let r0 = 7_000.0;
    let v_escape = sqrt(2.0 * MU / r0);
    let s0 = State::new(
        Vec3::new(r0, 0.0, 0.0),
        Vec3::new(0.0, v_escape * 1.12, 0.0),
    );
    assert!(s0.specific_energy(MU) > 0.0, "should be hyperbolic");
    assert!(s0.semi_major_axis(MU) < 0.0, "hyperbolic a must be negative");

    let mut previous = r0;
    for k in 1..=20 {
        let out = propagate(&s0, k as f64 * 500.0, MU);
        assert!(!out.unconverged, "hyperbolic diverged at step {}", k);
        let r = out.state.r.norm();
        assert!(r > previous, "radius must grow: {} -> {}", previous, r);
        previous = r;
    }
}

#[test]
fn zero_time_is_identity() {
    let s0 = eccentric(12_000.0, 0.5);
    let out = propagate(&s0, 0.0, MU);
    assert_eq!(out.state.r, s0.r);
    assert_eq!(out.state.v, s0.v);
}

#[test]
fn degenerate_input_yields_nan_not_a_panic() {
    // A player can hand the kernel nonsense mid-design. It must survive and say so.
    let bad = State::new(Vec3::ZERO, Vec3::ZERO);
    let out = propagate(&bad, 100.0, MU);
    assert!(out.unconverged);
    assert!(out.state.r.x.is_nan());
}

#[test]
fn stumpff_functions_are_continuous_at_zero() {
    // The series/closed-form switch at |z| = 1e-6 is a seam. A discontinuity there would produce a
    // visible jump in a trajectory, so it is checked from both sides.
    for &z in [1e-6, -1e-6, 1e-12, -1e-12, 0.0].iter() {
        let (c, s) = stumpff(z);
        // The function's own slope over |z| < 1e-6 accounts for 4.2e-8 of this; the series/closed-form
        // seam itself contributes nothing measurable. 1e-6 was too tight a bound to express that.
        assert!(fabs(c - 0.5) < 1e-7, "C({}) = {}", z, c);
        assert!(fabs(s - 1.0 / 6.0) < 1e-7, "S({}) = {}", z, s);
    }
    // And away from zero, the identity C·(something) is not needed — but monotonic decrease of C is.
    let mut prev = stumpff(0.0).0;
    for k in 1..50 {
        let z = k as f64 * 0.1;
        let (c, _) = stumpff(z);
        assert!(c < prev, "C should decrease with z at {}", z);
        prev = c;
    }
}

#[test]
fn true_anomaly_reads_90_degrees_at_quarter_orbit() {
    // Geometric check: from periapsis on a circular orbit, a quarter period puts you 90° round.
    let s0 = leo_circular();
    assert!(fabs(true_anomaly(&s0, MU)) < 1e-9, "periapsis is nu = 0");
    let p = period(s0.semi_major_axis(MU), MU);
    let quarter = propagate(&s0, p / 4.0, MU).state;
    let nu = true_anomaly(&quarter, MU);
    let expected = odderon_dmath::FRAC_PI_2;
    assert!(fabs(nu - expected) < 1e-6, "quarter orbit: nu = {}", nu);
}

/// `f64::max` is a std method; the tests may use std, but keeping one helper avoids confusion.
fn f64_max(a: f64, b: f64) -> f64 {
    if a > b {
        a
    } else {
        b
    }
}

// ---------------------------------------------------------------------------------------------
// classical orbital elements
// ---------------------------------------------------------------------------------------------

/// Tolerance used for every angle comparison, in radians. 1e-13 rad is 2e-8 arcseconds — below the
/// angular resolution of any star catalogue this project will ever load, so a discrepancy smaller than
/// this cannot matter to the game.
const ANGLE_TOL: f64 = 1e-13;

#[test]
fn vallado_example_2_5() {
    // The published test vector from Vallado, *Fundamentals of Astrodynamics and Applications*, Ex. 2-5.
    // This is the point of the exercise: an external source of truth, not round-tripping our own code
    // against itself. Expected: p = 11067.790 km, e = 0.83285, i = 87.87°, Ω = 227.898°, ω = 53.38°,
    // ν = 92.335°.
    let r = Vec3::new(6524.834, 6862.875, 6448.296);
    let v = Vec3::new(4.901_327, 5.533_756, -1.976_341);
    let el = State::new(r, v).to_elements(MU);

    let deg = 180.0 / odderon_dmath::PI;
    assert!(fabs(el.p - 11067.790) < 2e-2, "p = {}", el.p);
    assert!(fabs(el.e - 0.83285) < 1e-5, "e = {}", el.e);
    assert!(fabs(el.i * deg - 87.87) < 1e-2, "i = {}°", el.i * deg);
    assert!(fabs(el.raan * deg - 227.898) < 1e-3, "RAAN = {}°", el.raan * deg);
    assert!(fabs(el.argp * deg - 53.38) < 1e-2, "argp = {}°", el.argp * deg);
    assert!(fabs(el.nu * deg - 92.335) < 1e-3, "nu = {}°", el.nu * deg);
    assert!(el.degeneracy.is_none(), "this orbit is general");
}

#[test]
fn elements_round_trip_through_every_degenerate_case() {
    // The test that matters, and the reason the angle-folding convention exists at all. For each case
    // the round trip `state -> elements -> state` must close to rounding. Measured worst relative error
    // in JS across these shapes: 5.4e-16.
    let deg = odderon_dmath::PI / 180.0;
    let cases: [(&str, f64, f64, f64); 7] = [
        // name, a (km), e, inclination (deg)
        ("circular equatorial", 6778.137, 0.0, 0.0),
        ("circular inclined", 6778.137, 0.0, 51.6),
        ("circular retrograde", 7000.0, 0.0, 143.0),
        ("equatorial eccentric", 15000.0, 0.4, 0.0),
        ("equatorial near-parabolic", 20000.0, 0.999, 0.0),
        ("polar eccentric", 9000.0, 0.3, 90.0),
        ("general", 24500.0, 0.73, 28.5),
    ];

    for &(name, a, e, i_deg) in cases.iter() {
        for &nu in [0.0, 1.3, 4.0].iter() {
            let el0 = Elements {
                p: a * (1.0 - e * e),
                e,
                i: i_deg * deg,
                raan: 0.6,
                argp: 1.1,
                nu,
                degeneracy: Degeneracy::default(),
            };
            let s0 = el0.to_state(MU);
            let el1 = s0.to_elements(MU);
            let s1 = el1.to_state(MU);

            let dr = (s1.r - s0.r).norm() / s0.r.norm();
            let dv = (s1.v - s0.v).norm() / s0.v.norm();
            // 1e-14 is reachable only because the angles are recovered by atan2. With an acos of a
            // dot product the same test measured 1.5e-8 - not because the geometry was wrong, but
            // because acos is quadratic at 1, and periapsis is exactly where its argument is 1.
            assert!(
                dr < 1e-13,
                "{} at nu={}: relative position error {:.3e}",
                name,
                nu,
                dr
            );
            assert!(
                dv < 1e-13,
                "{} at nu={}: relative velocity error {:.3e}",
                name,
                nu,
                dv
            );
        }
    }
}

#[test]
fn the_degeneracy_flags_report_what_was_collapsed() {
    // Not decoration: the game will show these, so that "your inclination is 0, so the node is
    // meaningless" is visible rather than silently zero.
    let deg = odderon_dmath::PI / 180.0;
    let make = |e: f64, i_deg: f64| Elements {
        p: 8000.0 * (1.0 - e * e),
        e,
        i: i_deg * deg,
        raan: 0.3,
        argp: 0.7,
        nu: 1.0,
        degeneracy: Degeneracy::default(),
    };

    let general = make(0.2, 51.6).to_state(MU).to_elements(MU);
    assert!(general.degeneracy.is_none());

    let equatorial = make(0.2, 0.0).to_state(MU).to_elements(MU);
    assert!(equatorial.degeneracy.equatorial);
    assert!(!equatorial.degeneracy.circular);
    // The longitude of periapsis is reported in the argp slot, so it is NOT zero.
    assert!(equatorial.argp > 0.0, "argp should carry lon. of periapsis");

    let circular = make(0.0, 51.6).to_state(MU).to_elements(MU);
    assert!(circular.degeneracy.circular);
    assert!(!circular.degeneracy.equatorial);
    // The argument of latitude is reported in the nu slot.
    assert!(circular.nu > 0.0, "nu should carry argument of latitude");

    let both = make(0.0, 0.0).to_state(MU).to_elements(MU);
    assert!(both.degeneracy.is_total(), "circular AND equatorial");
    // True longitude survives in the nu slot.
    assert!(both.nu > 0.0, "nu should carry true longitude");
}

#[test]
fn a_nearly_equatorial_orbit_is_not_folded() {
    // i = 0.0001° is 1.7e-6 rad, twelve orders above the 1e-11 threshold. It must be treated as a
    // general orbit, because at that inclination the node line is perfectly well defined — the threshold
    // is about the noise floor of a dot product, not about "looks flat".
    let deg = odderon_dmath::PI / 180.0;
    let el0 = Elements {
        p: 8000.0 * (1.0 - 0.04),
        e: 0.2,
        i: 0.0001 * deg,
        raan: 0.6,
        argp: 1.1,
        nu: 2.0,
        degeneracy: Degeneracy::default(),
    };
    let el = el0.to_state(MU).to_elements(MU);
    assert!(el.degeneracy.is_none(), "1e-4 degrees is a real inclination");
    // Measured 2.4e-16 once inclination comes from atan2 rather than acos. The earlier version of this
    // assertion allowed 1e-9 because acos on h_z/|h| = 1 - 1e-16 returns up to 1.5e-8 rad; that was a
    // defect in to_elements, not a property of the orbit, so the bound is now tight.
    let s0 = el0.to_state(MU);
    let back = el.to_state(MU);
    let dr = (back.r - s0.r).norm() / s0.r.norm();
    assert!(dr < 1e-13, "rel error {:.3e} at i = 1e-4 deg", dr);
}

#[test]
fn elements_and_propagation_agree() {
    // The two halves of the crate must be one model, and this checks them against *Kepler*, not against
    // each other.
    //
    // The obvious version of this test is wrong, so it is worth recording why. One is tempted to say
    // "propagate a quarter period from periapsis and nu must be π/2" — but that is true only for a
    // circular orbit. For e = 0.3 a quarter of the *period* is a quarter of the mean anomaly, which
    // maps through Kepler's equation to nu = 122.54°, not 90°: the body loiters at apoapsis and races
    // through periapsis, which is the whole reason the mean/true distinction exists. Asserting π/2 would
    // have failed against correct code.
    //
    // So instead: nu after dt is derived independently from Kepler's equation, using the platform's
    // sin/cos in the test binary — a different route to the same number, which is the point.
    let el0 = Elements {
        p: 12000.0 * (1.0 - 0.09),
        e: 0.3,
        i: 0.6,
        raan: 1.2,
        argp: 0.4,
        nu: 0.0,
        degeneracy: Degeneracy::default(),
    };
    let s0 = el0.to_state(MU);
    let a = el0.semi_major_axis();
    let p = period(a, MU);

    for &fraction in [0.125, 0.25, 0.5, 0.75].iter() {
        let out = propagate(&s0, p * fraction, MU).state;
        let el1 = out.to_elements(MU);

        // Independent route: mean anomaly -> eccentric anomaly (Newton on Kepler's equation) -> nu.
        let mean_anomaly = core::f64::consts::TAU * fraction;
        let mut ecc_anomaly = mean_anomaly;
        for _ in 0..60 {
            ecc_anomaly -=
                (ecc_anomaly - el0.e * f64::sin(ecc_anomaly) - mean_anomaly)
                    / (1.0 - el0.e * f64::cos(ecc_anomaly));
        }
        let expected_nu = 2.0
            * f64::atan2(
                f64::sqrt(1.0 + el0.e) * f64::sin(ecc_anomaly / 2.0),
                f64::sqrt(1.0 - el0.e) * f64::cos(ecc_anomaly / 2.0),
            );

        // nu is recovered by atan2 from two components, so it is accurate to rounding even at 0 and pi
        // - which is where a quarter/half-period sample lands.
        assert!(
            fabs(el1.nu - expected_nu) < 1e-12,
            "at {} of a period: nu = {:.9}, Kepler says {:.9}",
            fraction,
            el1.nu,
            expected_nu
        );
        // And the orbit shape is unchanged by coasting, which is the conservation claim in element form.
        assert!(fabs(el1.p - el0.p) / el0.p < 1e-13, "p drifted");
        assert!(fabs(el1.e - el0.e) < 1e-13, "e drifted");
        assert!(fabs(el1.i - el0.i) < ANGLE_TOL, "inclination drifted");
        assert!(fabs(el1.raan - el0.raan) < ANGLE_TOL, "RAAN drifted");
    }
}

#[test]
fn derived_elements_are_consistent() {
    let el = Elements {
        p: 10000.0,
        e: 0.25,
        i: 0.0,
        raan: 0.0,
        argp: 0.0,
        nu: 0.0,
        degeneracy: Degeneracy::default(),
    };
    assert!(fabs(el.semi_major_axis() - 10000.0 / (1.0 - 0.0625)) < 1e-9);
    assert!(fabs(el.periapsis() - 8000.0) < 1e-9);
    // p = a(1 - e²) is the definition, so apoapsis must be 13333.33...
    assert!(fabs(el.apoapsis() - 40000.0 / 3.0) < 1e-9);

    let hyperbolic = Elements {
        e: 1.5,
        ..el
    };
    assert!(hyperbolic.apoapsis().is_infinite(), "hyperbola has no apoapsis");
    assert!(hyperbolic.semi_major_axis() < 0.0, "hyperbolic a is negative");
    // Periapsis stays defined for every conic.
    assert!(hyperbolic.periapsis() > 0.0);
}

// ---------------------------------------------------------------------------------------------
// bug regression tests
// ---------------------------------------------------------------------------------------------
//
// Each of these failed on the code that existed before it. They are kept with the reasoning that
// produced them, because a regression test with no explanation gets deleted by the next person who
// finds it inconvenient.

#[test]
fn overflow_is_reported_not_hidden() {
    // BUG: `propagate` accepted a NaN step as convergence.
    //
    // The convergence test used to be `if !(residual > TOLERANCE)`. For a hyperbolic orbit flung far
    // enough the Stumpff functions overflow (cosh overflows once √−ψ > 710.4759, i.e. ψ < −504776),
    // the Newton step becomes NaN, and `!(NaN > 1e-12)` is TRUE. So the loop broke out reporting
    // `unconverged: false` and returned a state of NaNs as if it were an answer.
    //
    // How reachable: 1.5x escape velocity from 7000 km, propagated 11.6 days. That is an ordinary
    // hyperbolic departure, not a contrived input.
    let v_escape = sqrt(2.0 * MU / 7000.0);
    for &factor in [1.5, 3.0, 12.0].iter() {
        let s0 = State::new(
            Vec3::new(7000.0, 0.0, 0.0),
            Vec3::new(0.0, v_escape * factor, 0.0),
        );
        let out = propagate(&s0, 1.0e6, MU);
        // The two things that must never both be true: a NaN answer and a claim of success.
        if out.state.r.x.is_nan() || !out.state.r.x.is_finite() {
            assert!(
                out.unconverged,
                "at {}x escape: returned non-finite state while reporting success",
                factor
            );
        }
        // And when it does report failure, `overflowed` says why, which is the actionable part.
        if out.overflowed {
            assert!(out.unconverged, "overflow must imply unconverged");
        }
    }

    // The invariant, stated once and universally: `overflowed` implies `unconverged`, a claimed
    // success is finite, and a reported failure never panics. Which specific speeds overflow depends on
    // dmath's own transcendentals rather than the platform's, so pinning a particular case would be
    // pinning my arithmetic model instead of the contract — and my model has been wrong twice today.
    let v_escape2 = sqrt(2.0 * MU / 7000.0);
    for &factor in [1.5, 3.0, 5.0, 12.0, 50.0].iter() {
        for &dt in [1.0e4, 1.0e5, 1.0e6, 1.0e7].iter() {
            let s0 = State::new(
                Vec3::new(7000.0, 0.0, 0.0),
                Vec3::new(0.0, v_escape2 * factor, 0.0),
            );
            let out = propagate(&s0, dt, MU);
            if out.overflowed {
                assert!(
                    out.unconverged,
                    "v={}x escape, dt={}: an overflow reported as success",
                    factor,
                    dt
                );
            }
            if !out.unconverged {
                assert!(
                    out.state.r.x.is_finite() && out.state.v.x.is_finite(),
                    "v={}x escape, dt={}: claimed success with a non-finite state",
                    factor,
                    dt
                );
            }
        }
    }

    // And an orbit that is merely hyperbolic must still work, so the fix cannot be "give up more".
    let modest = State::new(
        Vec3::new(7000.0, 0.0, 0.0),
        Vec3::new(0.0, v_escape * 1.05, 0.0),
    );
    let ok = propagate(&modest, 1.0e4, MU);
    assert!(!ok.unconverged, "1.05x escape over 2.8 h is well inside range");
    assert!(ok.state.r.x.is_finite());
}

#[test]
fn the_convergence_tolerance_scales_with_chi() {
    // BUG: the tolerance was absolute (1e-12 on |Δχ|) while χ itself runs to 1e4 and beyond.
    //
    // The question a step answers is "how much of χ is still in doubt", so the test has to be relative
    // to χ. With a fixed 1e-12 an ordinary orbit was reported unconverged while its answer was good to
    // a few parts in 1e11 — a false alarm that trains a caller to ignore the flag, which is worse than
    // not having it.
    //
    // Where convergence is solid it must be reported: `e = 1 + 1e-3` is the near-parabolic regime the
    // old fixed tolerance already handled, so this half pins what must not regress.
    for &delta in [1e-1, 1e-2, 1e-3].iter() {
        let e = 1.0 + delta;
        let rp = 7000.0 / (1.0 + e);
        let vp = sqrt(MU * (1.0 + e) / rp);
        let s0 = State::new(Vec3::new(rp, 0.0, 0.0), Vec3::new(0.0, vp, 0.0));
        for &dt in [1.0e3, 1.0e4, 1.0e5].iter() {
            let out = propagate(&s0, dt, MU);
            assert!(
                !out.unconverged,
                "e = 1+{} at dt = {} reported unconverged (residual {:.3e})",
                delta,
                dt,
                out.residual
            );
            assert!(out.state.r.x.is_finite());
        }
    }

    // Below that the honest answer is "it depends", and pretending otherwise is how the fixed
    // tolerance went wrong in the first place. At `e = 1 + 1e-6` the orbit is at the edge of what the
    // universal-variable formulation resolves in f64: F carries an absolute error of about
    // eps*max(r0*chi, sqrt(mu)*dt) because it subtracts two nearly equal terms, so the achievable
    // resolution in chi degrades exactly as the orbit approaches parabolic. What is required here is
    // not convergence — it is honesty, plus correctness when it does claim success.
    for &delta in [1e-5, 1e-6].iter() {
        let e = 1.0 + delta;
        let rp = 7000.0 / (1.0 + e);
        let vp = sqrt(MU * (1.0 + e) / rp);
        let s0 = State::new(Vec3::new(rp, 0.0, 0.0), Vec3::new(0.0, vp, 0.0));
        for &dt in [1.0e3, 1.0e4, 1.0e5].iter() {
            let out = propagate(&s0, dt, MU);
            if out.unconverged {
                continue; // reported honestly; that is all this case promises
            }
            assert!(
                out.state.r.x.is_finite(),
                "e = 1+{} dt={}: success claimed with a non-finite state",
                delta,
                dt
            );
            // A converged near-parabolic result must still conserve the orbit: p and e are integrals of
            // the motion, so a correct answer reproduces p even though it cannot be reached by iterating.
            let el = out.state.to_elements(MU);
            let p0 = 7000.0;
            assert!(
                fabs(el.p - p0) / p0 < 1e-6,
                "e = 1+{} dt={}: converged but p drifted by {:.3e}",
                delta,
                dt,
                fabs(el.p - p0) / p0
            );
        }
    }


    // A long hyperbolic flight: χ is large here, which is the case the old tolerance could not reach.
    let v_escape = sqrt(2.0 * MU / 7000.0);
    let s0 = State::new(
        Vec3::new(7000.0, 0.0, 0.0),
        Vec3::new(0.0, v_escape * 1.12, 0.0),
    );
    let out = propagate(&s0, 1.0e5, MU);
    assert!(!out.unconverged, "1.12x escape over 1.2 days must converge");
    assert!(!out.overflowed);
}

#[test]
fn true_anomaly_agrees_with_the_element_set() {
    // BUG: `true_anomaly` and `Elements::nu` were two implementations of one question, and had drifted.
    //
    // They disagreed in three ways — circular threshold 1e-10 vs 1e-9, an absolute vs relative
    // equatorial test, and most visibly an opposite sign convention: one returned (−π, π] and the
    // other [0, 2π), so ν = 190° came back as −170° from `true_anomaly` and 190° from the elements.
    // A game reading one for display and the other for physics would disagree with itself.
    //
    // They are now the same function. This test pins that, so they cannot drift again.
    let deg = odderon_dmath::PI / 180.0;
    let cases: [(f64, f64, f64); 4] = [
        (0.3, 0.6, 51.6),  // general
        (0.0, 0.0, 0.0),   // circular equatorial
        (0.0, 0.0, 51.6),  // circular inclined
        (0.4, 0.0, 0.0),   // equatorial eccentric
    ];
    for &(e, raan, i_deg) in cases.iter() {
        for &nu_deg in [10.0, 90.0, 190.0, 270.0, 350.0].iter() {
            let el = Elements {
                p: 12000.0 * (1.0 - e * e),
                e,
                i: i_deg * deg,
                raan,
                argp: 0.4,
                nu: nu_deg * deg,
                degeneracy: Degeneracy::default(),
            };
            let s = el.to_state(MU);
            let from_elements = s.to_elements(MU).nu;
            let from_function = true_anomaly(&s, MU);
            assert_eq!(
                from_elements.to_bits(),
                from_function.to_bits(),
                "e={} i={}: elements say {} but true_anomaly says {}",
                e,
                i_deg,
                from_elements,
                from_function
            );
            // And it is in [0, 2π), which is the convention that survived.
            assert!((0.0..odderon_dmath::TAU).contains(&from_elements));
        }
    }

    // A degenerate state has no anomaly, and must say so rather than return a number.
    let radial = State::new(Vec3::new(7000.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0));
    assert!(true_anomaly(&radial, MU).is_nan());
}

#[test]
fn a_degenerate_mu_or_radius_is_not_silently_accepted() {
    // Belt and braces on the same failure mode as `overflow_is_reported_not_hidden`: every early-out in
    // propagate() must pair a NaN state with `unconverged: true`, never with success.
    let cases: [(f64, f64, f64); 4] = [
        // rx, vx, mu — the message prints r and mu, so no label is needed.
        (0.0, 0.0, MU),
        (7000.0, 0.0, 0.0),
        (7000.0, 0.0, -1.0),
        (f64::NAN, 0.0, MU),
    ];
    for &(rx, vx, mu) in cases.iter() {
        let r = Vec3::new(rx, 0.0, 0.0);
        let v = Vec3::new(vx, 7.0, 0.0);
        let out = propagate(&State::new(r, v), 100.0, mu);
        assert!(
            out.unconverged,
            "degenerate input (r={:?}, mu={}) reported success",
            r, mu
        );
    }
}
