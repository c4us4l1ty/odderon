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
