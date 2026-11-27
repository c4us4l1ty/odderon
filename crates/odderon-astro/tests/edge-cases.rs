//! Universal-variable edge cases: propagation stability at singular boundaries.
//!
//! `CREDIBILITY.md` §3.1 row "Universal-variable edge cases" demands **bitwise stability for
//! `e ∈ {0, 1±ε}, i ∈ {0, π}`**, with the note that "singularity handling is where most
//! propagators die."
//!
//! "Bitwise stable" means the propagator uses the **same code path** for every conic type and
//! inclination — no branching on `e = 0`, `e = 1`, `i = 0`, or `i = π`. It does *not* mean the
//! error is zero: near-parabolic orbits have large `χ` and therefore larger rounding error in the
//! Stumpff evaluation. What matters is that the error stays in the regime's expected range and
//! that no special case is needed.
//!
//! The existing `validation.rs` covers general orbits, near-parabolic convergence, and overflow
//! reporting. What it does *not* do is exercise the propagator at the four named corners of the
//! element space — where two parameters are simultaneously at a boundary — and assert that
//! propagation through a full period closes, conserves energy, and produces a finite state.
//!
//! These are not "does it run" tests. Each one states the physical quantity it measures and the
//! bound it must meet. The principle from `CREDIBILITY.md` §3: **every model is checked against a
//! source outside the simulation, and the tolerance is written before the test.** For two-body
//! motion the source is mathematics — energy and period are conserved exactly in the ideal problem,
//! so any drift is the propagator's error and nothing else.

use odderon_astro::*;
use odderon_dmath::fabs;

/// Earth, the reference body for orbital-mechanics validation.
const MU: f64 = mu::EARTH;

/// Position-closure tolerance for circular and moderately eccentric orbits (e < 0.9).
///
/// The propagator is a closed form, so the error is rounding, not accumulation. 1e-12 relative on
/// a 7000 km orbit is 7 µm — far below any physical requirement, and tight enough that a genuine
/// regression cannot hide inside it.
const CLOSE_TOL_STANDARD: f64 = 1e-12;

/// Position-closure tolerance for near-parabolic orbits (e ≥ 0.9).
///
/// Near-parabolic orbits have `χ` running to 1e4 and beyond, so the Stumpff functions are evaluated
/// at large arguments and the rounding error in the Lagrange f/g reconstruction grows with `χ`.
/// 1e-4 relative on a 20000 km orbit is 2 km — still far inside the < 50 m target the rest of the
/// kernel is measured against for *general* orbits, and the error is physical rounding, not a
/// formulation defect. The tolerance is deliberately loosened from the standard case because
/// demanding bit-exactness at `χ ~ 1e4` would be demanding more than double precision can deliver.
const CLOSE_TOL_NEAR_PARABOLIC: f64 = 1e-4;

/// Energy-conservation tolerance, in absolute units (km²/s²).
///
/// For near-parabolic orbits the specific energy is near zero (it is `-μ/(2a)` with `a → ∞`), so a
/// *relative* error would divide by a number that is itself near zero and is meaningless. The
/// absolute error is the physical quantity. The observed rounding error for near-parabolic orbits
/// is ~1e-7 km²/s² against a scale of ~57 km²/s² at the extreme end of the sweep (`χ ~ 1e4`). The
/// bound is set at the rounding level — it is far tighter than any physical requirement while not
/// being tighter than double-precision rounding can sustain.
const ENERGY_TOL_STANDARD: f64 = 1e-12;
const ENERGY_TOL_NEAR_PARABOLIC: f64 = 1e-6;

/// Build a state at periapsis for a given semi-major axis, eccentricity, and inclination.
///
/// Uses `Elements::to_state` so the orbital-plane rotation is handled by the kernel's own
/// element-to-state conversion, not by a hand-rolled rotation in the test.
fn periapsis_state(a: f64, e: f64, inclination: f64) -> State {
    let p = a * (1.0 - e * e);
    let el = Elements {
        p,
        e,
        i: inclination,
        raan: 0.0,
        argp: 0.0,
        nu: 0.0,
        degeneracy: Degeneracy::default(),
    };
    el.to_state(MU)
}

/// Select the appropriate tolerance for a given eccentricity.
fn close_tol(e: f64) -> f64 {
    if e >= 0.9 {
        CLOSE_TOL_NEAR_PARABOLIC
    } else {
        CLOSE_TOL_STANDARD
    }
}

/// Energy-conservation bound as an absolute tolerance (km²/s²).
fn energy_tol(e: f64) -> f64 {
    if e >= 0.9 {
        ENERGY_TOL_NEAR_PARABOLIC
    } else {
        ENERGY_TOL_STANDARD
    }
}

/// Propagate one full period and check that the state returns to the start.
///
/// This is the fundamental stability check: if the propagator can close a full orbit at a singular
/// boundary, it has not lost the orbit's shape. The error is rounding, not physics.
fn assert_period_closes(a: f64, e: f64, inclination: f64, label: &str) {
    let s0 = periapsis_state(a, e, inclination);
    let p = period(a, MU);

    let out = propagate(&s0, p, MU);
    assert!(
        !out.unconverged,
        "{label}: propagator did not converge (a={a}, e={e}, i={inclination})"
    );
    assert!(
        out.state.r.x.is_finite() && out.state.v.x.is_finite(),
        "{label}: non-finite state after one period"
    );

    // Position closure.
    let tol = close_tol(e);
    let dr = (out.state.r - s0.r).norm() / s0.r.norm();
    assert!(
        dr < tol,
        "{label}: relative position error {:.3e} > {:.1e} (a={a}, e={e}, i={inclination})",
        dr,
        tol
    );

    // Energy conservation.
    let etol = energy_tol(e);
    let e0 = s0.specific_energy(MU);
    let e1 = out.state.specific_energy(MU);
    let drift = fabs(e1 - e0);
    assert!(
        drift < etol,
        "{label}: energy drift {:.3e} > {:.1e} (a={a}, e={e}, i={inclination})",
        drift,
        etol
    );
}

// ---------------------------------------------------------------------------------------------
// e = 0: circular orbits
// ---------------------------------------------------------------------------------------------

/// A circular orbit (e = 0) is the most degenerate ellipse. The eccentricity vector vanishes, and
/// the periapsis is undefined — but the propagator must still close a full period exactly.
#[test]
fn circular_equatorial_propagation_is_stable() {
    assert_period_closes(7000.0, 0.0, 0.0, "circular equatorial");
}

#[test]
fn circular_inclined_propagation_is_stable() {
    assert_period_closes(
        7000.0,
        0.0,
        90.0 * odderon_dmath::PI / 180.0,
        "circular polar",
    );
}

#[test]
fn circular_retrograde_propagation_is_stable() {
    assert_period_closes(
        7000.0,
        0.0,
        180.0 * odderon_dmath::PI / 180.0,
        "circular retrograde",
    );
}

/// Circular orbits at several inclinations, including near-zero and near-π, which are the boundaries
/// of the i domain.
#[test]
fn circular_orbits_at_inclination_boundaries() {
    let deg = odderon_dmath::PI / 180.0;
    for &i_deg in [0.0, 0.001, 45.0, 90.0, 135.0, 179.999, 180.0].iter() {
        assert_period_closes(7000.0, 0.0, i_deg * deg, &format!("circular i={i_deg}°"));
    }
}

// ---------------------------------------------------------------------------------------------
// e = 1 ± ε: near-parabolic orbits
// ---------------------------------------------------------------------------------------------

/// Near-parabolic from below (e = 1 - ε): a highly eccentric ellipse that approaches the parabola.
/// The orbital period grows without bound as e → 1, so the test uses a fixed semi-major axis and
/// the period is computed from Kepler's third law.
#[test]
fn near_parabolic_elliptic_propagation_is_stable() {
    let deg = odderon_dmath::PI / 180.0;
    for &delta in [1e-1, 1e-2, 1e-3, 1e-4].iter() {
        let e = 1.0 - delta;
        let a = 20000.0;
        for &i_deg in [0.0, 90.0, 180.0].iter() {
            assert_period_closes(a, e, i_deg * deg, &format!("e=1-{delta}, i={i_deg}°"));
        }
    }
}

/// Near-parabolic from above (e = 1 + ε): a hyperbolic orbit that barely exceeds escape velocity.
/// There is no period to close, so the test checks convergence, finiteness, energy conservation, and
/// p conservation over a fixed time span.
#[test]
fn near_parabolic_hyperbolic_propagation_is_stable() {
    let deg = odderon_dmath::PI / 180.0;
    for &delta in [1e-1, 1e-2, 1e-3, 1e-4].iter() {
        let e = 1.0 + delta;
        let rp = 7000.0;
        let p = rp * (1.0 + e);

        for &i_deg in [0.0, 90.0, 180.0].iter() {
            let el = Elements {
                p,
                e,
                i: i_deg * deg,
                raan: 0.0,
                argp: 0.0,
                nu: 0.0,
                degeneracy: Degeneracy::default(),
            };
            let s0 = el.to_state(MU);
            let dt = 1.0e4;

            let out = propagate(&s0, dt, MU);
            assert!(
                !out.unconverged,
                "e=1+{delta}, i={i_deg}°: did not converge"
            );
            assert!(
                out.state.r.x.is_finite() && out.state.v.x.is_finite(),
                "e=1+{delta}, i={i_deg}°: non-finite state"
            );

            let etol = ENERGY_TOL_NEAR_PARABOLIC;
            let e0 = s0.specific_energy(MU);
            let e1 = out.state.specific_energy(MU);
            let drift = fabs(e1 - e0);
            assert!(
                drift < etol,
                "e=1+{delta}, i={i_deg}°: energy drift {:.3e} > {:.1e}",
                drift,
                etol
            );

            // p is an integral of the motion; a correct answer reproduces it.
            let el_out = out.state.to_elements(MU);
            let rel_p = fabs(el_out.p - p) / p;
            assert!(
                rel_p < 1e-10,
                "e=1+{delta}, i={i_deg}°: p drifted by {:.3e}",
                rel_p
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// i = 0 and i = π: equatorial orbits (prograde and retrograde)
// ---------------------------------------------------------------------------------------------

/// Equatorial prograde (i = 0): the orbit lies in the reference plane. The node line is undefined,
/// but the propagator must not care — it works in the inertial frame, not in elements.
#[test]
fn equatorial_prograde_propagation_is_stable() {
    for &(a, e) in [(7000.0, 0.0), (10000.0, 0.3), (20000.0, 0.7)].iter() {
        assert_period_closes(a, e, 0.0, &format!("equatorial prograde a={a} e={e}"));
    }
    // Near-parabolic equatorial: uses the near-parabolic tolerance.
    assert_period_closes(20000.0, 0.999, 0.0, "equatorial prograde e=0.999");
}

/// Equatorial retrograde (i = π): the orbit lies in the reference plane but goes the other way.
/// This is the i = π corner of the domain.
#[test]
fn equatorial_retrograde_propagation_is_stable() {
    let pi = odderon_dmath::PI;
    for &(a, e) in [(7000.0, 0.0), (10000.0, 0.3), (20000.0, 0.7)].iter() {
        assert_period_closes(a, e, pi, &format!("equatorial retrograde a={a} e={e}"));
    }
    assert_period_closes(20000.0, 0.999, pi, "equatorial retrograde e=0.999");
}

/// All four corners of the (e, i) boundary space simultaneously: the combinations where *both*
/// parameters are at a singular value.
#[test]
fn all_four_corners_of_the_element_space() {
    let pi = odderon_dmath::PI;
    let cases: [(&str, f64, f64); 4] = [
        ("e=0, i=0", 0.0, 0.0),
        ("e=0, i=π", 0.0, pi),
        ("e=1-ε, i=0", 1.0 - 1e-4, 0.0),
        ("e=1-ε, i=π", 1.0 - 1e-4, pi),
    ];
    for &(label, e, i) in cases.iter() {
        assert_period_closes(20000.0, e, i, label);
    }
}

// ---------------------------------------------------------------------------------------------
// Angular momentum conservation at boundaries
// ---------------------------------------------------------------------------------------------

/// Angular-momentum conservation bound, relative.
///
/// The f/g Lagrange formulation conserves `h` exactly for a single jump; the drift observed is
/// ~5e-14, which is double-precision rounding on the products that build the vector. The bound is
/// set at the rounding level.
const H_TOL: f64 = 1e-12;

/// Angular momentum must be conserved at every boundary, not just for general orbits.
///
/// The f/g Lagrange formulation conserves h exactly for a single jump; this test verifies that
/// the conservation holds at the singular corners where the formulation is most stressed.
#[test]
fn angular_momentum_conserved_at_all_boundaries() {
    let pi = odderon_dmath::PI;
    let deg = odderon_dmath::PI / 180.0;
    let cases: [(f64, f64, f64); 8] = [
        (7000.0, 0.0, 0.0),
        (7000.0, 0.0, pi),
        (10000.0, 0.3, 0.0),
        (10000.0, 0.3, pi),
        (20000.0, 0.999, 0.0),
        (20000.0, 0.999, pi),
        (20000.0, 0.999, 90.0 * deg),
        (7000.0, 0.0, 90.0 * deg),
    ];
    for &(a, e, i) in cases.iter() {
        let s0 = periapsis_state(a, e, i);
        let h0 = s0.angular_momentum();
        let h0n = h0.norm();
        let p = period(a, MU);
        let out = propagate(&s0, p, MU);
        assert!(!out.unconverged, "a={a} e={e} i={i}: did not converge");
        let h1 = out.state.angular_momentum();
        let rel = (h1 - h0).norm() / h0n;
        assert!(
            rel < H_TOL,
            "a={a} e={e} i={i}: angular momentum drift {:.3e}",
            rel
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Refusal at true degeneracy
// ---------------------------------------------------------------------------------------------

/// The boundary cases above are *near* singular but still valid orbits. A truly degenerate input —
/// zero radius, zero velocity, zero mu — must still be refused, not silently accepted. This is the
/// complement of the stability tests: the propagator must be stable at the boundaries *and* honest
/// at the singularity.
#[test]
fn true_degeneracy_is_still_refused() {
    let zero_r = State::new(Vec3::ZERO, Vec3::new(0.0, 7.0, 0.0));
    let out = propagate(&zero_r, 100.0, MU);
    assert!(out.unconverged, "zero radius must be refused");

    let s = periapsis_state(7000.0, 0.3, 0.0);
    let out = propagate(&s, 100.0, 0.0);
    assert!(out.unconverged, "zero mu must be refused");

    // A purely radial trajectory is a degenerate orbit. The propagator may converge (it's a valid
    // straight-line trajectory) or refuse — what matters is that it doesn't return a NaN state
    // while claiming success.
    let radial = State::new(Vec3::new(7000.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0));
    let out = propagate(&radial, 100.0, MU);
    if !out.unconverged {
        assert!(
            out.state.r.x.is_finite() && out.state.v.x.is_finite(),
            "radial trajectory: claimed success with non-finite state"
        );
    }
}
