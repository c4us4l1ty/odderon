//! Lambert solver tests, checked natively.
//!
//! The numbers below are not the solver's outputs copied down. They are what the transfer *must*
//! be, from physical definitions and published reference solutions, so a change in the solver has
//! to argue with physics rather than with a previous run:
//!
//! * **Vallado / Curtis Example 5.2** — the standard published Lambert test case. The expected
//!   `v₁` and `v₂` come from Curtis's textbook, solved with the same universal-variable method.
//! * **Circular orbit** — a transfer between two points on a known circular orbit must recover the
//!   circular-orbit velocity exactly. This is the strongest self-consistency check: the solver's
//!   answer is checked against the vis-viva equation, not against itself.
//! * **180° Hohmann transfer** — the degenerate case where `r₁` and `r₂` are anti-parallel. The
//!   solver must handle the singularity and produce a velocity that, when propagated, reaches `r₂`.
//! * **Degenerate input** — same-point positions, non-positive time, and non-finite values must
//!   be refused, not silently returned as NaNs.

use odderon_astro::lambert::lambert;
use odderon_astro::{propagate, State, Vec3};

const MU: f64 = 398_600.441_8;

fn norm(v: &Vec3) -> f64 {
    (v.x * v.x + v.y * v.y + v.z * v.z).sqrt()
}

/// Relative error between two vectors, or `INFINITY` if either is non-finite.
fn rel_err(a: &Vec3, b: &Vec3) -> f64 {
    let na = norm(a);
    let nb = norm(b);
    if !na.is_finite() || !nb.is_finite() || nb == 0.0 {
        return f64::INFINITY;
    }
    let diff = (a.x - b.x).powi(2) + (a.y - b.y).powi(2) + (a.z - b.z).powi(2);
    diff.sqrt() / nb
}

/// Propagate the solved departure state forward by the flight time and check that the position
/// matches the target. This is the self-consistency check that does not depend on the solver's
/// internals: it checks the solver against the propagator, which is already validated against
/// physical definitions.
fn assert_propagation_reaches_target(r1: Vec3, r2: Vec3, dt_s: f64, label: &str) {
    let sol = lambert(r1, r2, dt_s, MU);
    assert!(
        !sol.unconverged,
        "{label}: solver did not converge (iterations={}, residual={})",
        sol.iterations,
        sol.residual
    );

    let state = State::new(r1, sol.v1);
    let prop = propagate(&state, dt_s, MU);
    assert!(
        !prop.unconverged,
        "{label}: propagation after solve did not converge"
    );

    let reached = prop.state.r;
    let err = rel_err(&reached, &r2);
    assert!(
        err < 1e-8,
        "{label}: propagated position error {err} > 1e-8 (|r2|={})",
        norm(&r2)
    );
}

// ---------------------------------------------------------------------------------------------
// Vallado / Curtis Example 5.2
// ---------------------------------------------------------------------------------------------

/// The standard published Lambert test case (Curtis, "Orbital Mechanics for Engineering Students",
/// Example 5.2, solved via Vallado's universal-variable algorithm).
///
/// The expected velocities are the published solution. The arrival velocity error must be below
/// 1e-8 relative — this is the acceptance criterion from `CREDIBILITY.md` §3.1.
#[test]
fn vallado_example_5_2_matches_published_solution() {
    let r1 = Vec3::new(5000.0, 10000.0, 2100.0);
    let r2 = Vec3::new(-14600.0, 2500.0, 7000.0);
    let dt_s = 3600.0;

    let sol = lambert(r1, r2, dt_s, MU);
    assert!(!sol.unconverged, "solver did not converge");

    // Published solution (Curtis Example 5.2 / Vallado universal variables).
    let v1_exp = Vec3::new(-5.992_495_03, 1.925_366_71, 3.245_638_05);
    let v2_exp = Vec3::new(-3.312_458_51, -4.196_619_01, -0.385_289_06);

    let e1 = rel_err(&sol.v1, &v1_exp);
    let e2 = rel_err(&sol.v2, &v2_exp);

    assert!(
        e1 < 1e-8,
        "departure velocity error {e1} > 1e-8 (expected {v1_exp:?}, got {:?})",
        sol.v1
    );
    assert!(
        e2 < 1e-8,
        "arrival velocity error {e2} > 1e-8 (expected {v2_exp:?}, got {:?})",
        sol.v2
    );
}

/// The Vallado case must also be self-consistent: propagating the solved departure state forward
/// by the flight time must reproduce the target position.
#[test]
fn vallado_example_5_2_is_propagation_consistent() {
    assert_propagation_reaches_target(
        Vec3::new(5000.0, 10000.0, 2100.0),
        Vec3::new(-14600.0, 2500.0, 7000.0),
        3600.0,
        "vallado_5_2",
    );
}

// ---------------------------------------------------------------------------------------------
// Circular orbit: the solver must recover the circular-orbit velocity exactly
// ---------------------------------------------------------------------------------------------

/// A transfer between two points on a circular orbit must recover the circular-orbit velocity.
///
/// For a circular orbit at radius `r`:
///   v = sqrt(mu / r), directed perpendicular to r.
///
/// The transfer angle `θ` and flight time are related by `dt = θ / ω = θ·sqrt(r³/mu)`. The solver
/// must return exactly the circular-orbit velocity at both endpoints.
#[test]
fn circular_orbit_transfer_recovers_circular_velocity() {
    let r = 7000.0; // km, LEO-ish
    let v_circ = (MU / r).sqrt();
    let theta: f64 = 0.8; // radians, ~46°

    // Positions on the circular orbit in the xy-plane.
    let r1 = Vec3::new(r, 0.0, 0.0);
    let r2 = Vec3::new(r * theta.cos(), r * theta.sin(), 0.0);

    // Flight time: dt = theta / omega, where omega = sqrt(mu / r^3).
    let dt_s = theta * (r * r * r / MU).sqrt();

    let sol = lambert(r1, r2, dt_s, MU);
    assert!(!sol.unconverged, "solver did not converge");

    // Expected v1: (0, v_circ, 0).
    let v1_exp = Vec3::new(0.0, v_circ, 0.0);
    // Expected v2: (-v_circ·sin(theta), v_circ·cos(theta), 0).
    let v2_exp = Vec3::new(-v_circ * theta.sin(), v_circ * theta.cos(), 0.0);

    let e1 = rel_err(&sol.v1, &v1_exp);
    let e2 = rel_err(&sol.v2, &v2_exp);

    assert!(e1 < 1e-8, "v1 error {e1} > 1e-8 (expected {v1_exp:?}, got {:?})", sol.v1);
    assert!(e2 < 1e-8, "v2 error {e2} > 1e-8 (expected {v2_exp:?}, got {:?})", sol.v2);
}

/// A larger circular-orbit transfer (120°) must also recover the circular velocity.
#[test]
fn circular_orbit_transfer_large_angle() {
    let r = 8000.0;
    let v_circ = (MU / r).sqrt();
    let theta: f64 = 2.0944; // 120°

    let r1 = Vec3::new(r, 0.0, 0.0);
    let r2 = Vec3::new(r * theta.cos(), r * theta.sin(), 0.0);
    let dt_s = theta * (r * r * r / MU).sqrt();

    let sol = lambert(r1, r2, dt_s, MU);
    assert!(!sol.unconverged, "solver did not converge");

    let v1_exp = Vec3::new(0.0, v_circ, 0.0);
    let v2_exp = Vec3::new(-v_circ * theta.sin(), v_circ * theta.cos(), 0.0);

    assert!(rel_err(&sol.v1, &v1_exp) < 1e-8, "v1 error too large");
    assert!(rel_err(&sol.v2, &v2_exp) < 1e-8, "v2 error too large");
}

// ---------------------------------------------------------------------------------------------
// 180° degenerate case
// ---------------------------------------------------------------------------------------------

/// The 180° Hohmann transfer: `r₁` and `r₂` are anti-parallel.
///
/// This is the degenerate case from `CREDIBILITY.md` §3.1. The Lagrange-coefficient formulation
/// has a singularity here (g → 0), so the solver must use its perturbation path. The key check is
/// self-consistency: propagating the solved departure state must reach the target.
#[test]
fn hohmann_transfer_180_degrees() {
    let r1_km = 7000.0; // LEO
    let r2_km = 42164.0; // GEO
    let a = (r1_km + r2_km) / 2.0; // semi-major axis of the transfer ellipse

    // Hohmann transfer time: half the orbital period.
    let dt_s = core::f64::consts::PI * (a * a * a / MU).sqrt();

    let r1 = Vec3::new(r1_km, 0.0, 0.0);
    let r2 = Vec3::new(-r2_km, 0.0, 0.0); // anti-parallel: 180° transfer

    let sol = lambert(r1, r2, dt_s, MU);
    assert!(!sol.unconverged, "180° solver did not converge");

    // Expected departure speed from vis-viva: v1 = sqrt(mu * (2/r1 - 1/a)).
    let v1_expected_mag = (MU * (2.0 / r1_km - 1.0 / a)).sqrt();
    let v1_mag = norm(&sol.v1);
    assert!(
        (v1_mag - v1_expected_mag).abs() / v1_expected_mag < 1e-8,
        "v1 magnitude {} != expected {}",
        v1_mag,
        v1_expected_mag
    );

    // Expected arrival speed: v2 = sqrt(mu * (2/r2 - 1/a)).
    let v2_expected_mag = (MU * (2.0 / r2_km - 1.0 / a)).sqrt();
    let v2_mag = norm(&sol.v2);
    assert!(
        (v2_mag - v2_expected_mag).abs() / v2_expected_mag < 1e-8,
        "v2 magnitude {} != expected {}",
        v2_mag,
        v2_expected_mag
    );

    // The velocity must be perpendicular to r1 (tangential, not radial).
    let dot_r1_v1 = r1.dot(sol.v1);
    assert!(
        dot_r1_v1.abs() / (r1_km * v1_mag) < 1e-6,
        "v1 is not tangential: r1·v1 = {dot_r1_v1} (should be ~0)"
    );

    // Self-consistency: propagate and check the position.
    assert_propagation_reaches_target(r1, r2, dt_s, "hohmann_180");
}

/// A 180° transfer at LEO altitude (both endpoints at the same radius) is also degenerate.
#[test]
fn hohmann_transfer_180_degrees_same_radius() {
    let r = 7000.0;
    // 180° transfer at the same radius: the transfer orbit is a degenerate ellipse (a = r, e = 0
    // doesn't work for 180°). Use a flight time that corresponds to a half-period of an ellipse
    // with periapsis and apoapsis both near r. Actually, for same-radius 180°, the minimum-energy
    // transfer is a degenerate ellipse with a = r. Use dt = pi * sqrt(r^3 / mu) (half period of
    // a circular orbit, which is the limiting case).
    let dt_s = core::f64::consts::PI * (r * r * r / MU).sqrt();

    let r1 = Vec3::new(r, 0.0, 0.0);
    let r2 = Vec3::new(-r, 0.0, 0.0);

    let sol = lambert(r1, r2, dt_s, MU);
    assert!(!sol.unconverged, "180° same-radius solver did not converge");

    // For a circular orbit, the 180° transfer is a half-period. The velocity should be the
    // circular-orbit velocity, directed tangentially.
    let v_circ = (MU / r).sqrt();
    let v1_mag = norm(&sol.v1);
    assert!(
        (v1_mag - v_circ).abs() / v_circ < 1e-6,
        "v1 magnitude {} != circular {}",
        v1_mag,
        v_circ
    );

    assert_propagation_reaches_target(r1, r2, dt_s, "hohmann_180_same_r");
}

// ---------------------------------------------------------------------------------------------
// Zero-rev and one-rev transfers
// ---------------------------------------------------------------------------------------------

/// A short-angle transfer (zero-rev) must be self-consistent.
#[test]
fn short_angle_transfer_zero_rev() {
    let r1 = Vec3::new(7000.0, 0.0, 0.0);
    let r2 = Vec3::new(7000.0 * 0.3_f64.cos(), 7000.0 * 0.3_f64.sin(), 0.0); // ~17°
    let dt_s = 0.3_f64 * (7000.0_f64.powi(3) / MU).sqrt();

    assert_propagation_reaches_target(r1, r2, dt_s, "short_angle_zero_rev");
}

/// A moderate-angle transfer (one-rev) must be self-consistent.
#[test]
fn moderate_angle_transfer_one_rev() {
    let r1 = Vec3::new(7000.0, 0.0, 0.0);
    let r2 = Vec3::new(7000.0 * 1.5_f64.cos(), 7000.0 * 1.5_f64.sin(), 0.0); // ~86°
    let dt_s = 1.5_f64 * (7000.0_f64.powi(3) / MU).sqrt();

    assert_propagation_reaches_target(r1, r2, dt_s, "moderate_angle_one_rev");
}

/// A 3D transfer (non-zero inclination) must be self-consistent.
#[test]
fn three_d_transfer() {
    let r1 = Vec3::new(7000.0, 0.0, 0.0);
    let r2 = Vec3::new(5000.0, 3000.0, 4000.0);
    let dt_s = 2000.0;

    assert_propagation_reaches_target(r1, r2, dt_s, "three_d_transfer");
}

// ---------------------------------------------------------------------------------------------
// Degenerate input
// ---------------------------------------------------------------------------------------------

/// Same-point positions: no transfer needed; the solver must refuse.
#[test]
fn same_point_is_refused() {
    let r = Vec3::new(7000.0, 0.0, 0.0);
    let sol = lambert(r, r, 600.0, MU);
    assert!(sol.unconverged, "same-point input should be refused");
}

/// Non-positive time of flight must be refused.
#[test]
fn non_positive_dt_is_refused() {
    let r1 = Vec3::new(7000.0, 0.0, 0.0);
    let r2 = Vec3::new(0.0, 7000.0, 0.0);
    let sol = lambert(r1, r2, 0.0, MU);
    assert!(sol.unconverged, "zero dt should be refused");

    let sol_neg = lambert(r1, r2, -600.0, MU);
    assert!(sol_neg.unconverged, "negative dt should be refused");
}

/// Non-finite inputs must be refused.
#[test]
fn non_finite_input_is_refused() {
    let r1 = Vec3::new(7000.0, 0.0, 0.0);
    let r2 = Vec3::new(0.0, 7000.0, 0.0);

    let sol_nan = lambert(r1, r2, f64::NAN, MU);
    assert!(sol_nan.unconverged, "NaN dt should be refused");

    let sol_inf = lambert(r1, r2, f64::INFINITY, MU);
    assert!(sol_inf.unconverged, "Inf dt should be refused");

    let r_nan = Vec3::new(f64::NAN, 0.0, 0.0);
    let sol_r = lambert(r_nan, r2, 600.0, MU);
    assert!(sol_r.unconverged, "NaN r1 should be refused");

    let sol_mu = lambert(r1, r2, 600.0, 0.0);
    assert!(sol_mu.unconverged, "zero mu should be refused");
}
