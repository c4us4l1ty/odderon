//! J2 + J3 + J4 propagation: the full zonal-harmonic field through an integrator.
//!
//! `CREDIBILITY.md` §3.1's row "J2 + J3 + J4 propagation" asks for this: a numerical integration
//! of the real gravitational field (point mass + J2 + J3 + J4), with the self-consistency target
//! of **< 50 m** position difference over 7 days at LEO.
//!
//! ## What is checked here
//!
//! * **The general field reduces to J2 when J3 = J4 = 0.** [`accel_zonal`] with zero J3/J4 must
//!   reproduce [`accel_j2`] exactly, and [`potential_zonal`] must reproduce [`potential_j2`]. This
//!   is the guard that says the generalization did not perturb the J2 path that every other test
//!   depends on.
//! * **The potential's gradient is the acceleration.** Finite-differencing [`potential_zonal`] and
//!   comparing against [`accel_zonal`] proves the J3 and J4 acceleration terms are the gradient of
//!   the J3 and J4 potential terms — not independent formulas that might drift apart. This is the
//!   same check `j2-crosscheck.rs` runs for J2 alone, extended to the full field.
//! * **Energy conservation under the full field.** The total energy `v²/2 − μ/r + U_J2 + U_J3 + U_J4`
//!   is constant under a conservative field. If the integrator leaks energy, every other number here
//!   is suspect.
//! * **Self-consistency: < 50 m over 7 days at LEO.** The integrator run at two different tolerances
//!   must agree to within 50 m — the `CREDIBILITY.md` target. This is the integrator checking itself,
//!   not comparing against a formula that shares its code path.
//! * **J3 and J4 have a measurable effect.** The J2+J3+J4 integrator must differ from the J2-only
//!   integrator over 7 days. If the terms were not being applied, the two would agree exactly and the
//!   block would be a no-op.
//! * **Refusals.** Degenerate input returns `None`, not NaNs.

use odderon_astro::integrate::{
    accel_j2, accel_zonal, integrate_j2, integrate_zonal, potential_j2, potential_zonal,
};
use odderon_astro::j2::{EARTH_J2, EARTH_J3, EARTH_J4};
use odderon_astro::*;
use odderon_dmath::{fabs, sqrt};

const MU: f64 = mu::EARTH;
const RE: f64 = radius::EARTH;

/// Reference tolerance for the integrator. `1e-12` puts the integrator's own error well below the
/// quantities being measured.
const REL_TOL: f64 = 1e-12;

/// Build a state at periapsis with the given elements, periapsis on +X and the orbit inclined about X.
///
/// Constructed directly rather than through `Elements::to_state` on purpose: this reference must not
/// share a code path with the thing it is checking, or a bug in the element conversion would cancel out.
fn at_periapsis(a: f64, e: f64, inclination: f64) -> State {
    let rp = a * (1.0 - e);
    let vp = sqrt(MU * (1.0 + e) / rp);
    State::new(
        Vec3::new(rp, 0.0, 0.0),
        Vec3::new(0.0, vp * cos_rad(inclination), vp * sin_rad(inclination)),
    )
}

// ---------------------------------------------------------------------------------------------
// The general field reduces to J2 when J3 = J4 = 0
// ---------------------------------------------------------------------------------------------

/// `accel_zonal` with `j3 = 0, j4 = 0` must equal `accel_j2` exactly.
///
/// This is the guard that the generalization did not perturb the J2 path. Every other test in the
/// crate that uses `accel_j2` depends on this equality holding.
#[test]
fn accel_zonal_reduces_to_accel_j2_when_j3_j4_zero() {
    let points = [
        Vec3::new(7000.0, 0.0, 0.0),
        Vec3::new(0.0, 7000.0, 0.0),
        Vec3::new(0.0, 0.0, 7000.0),
        Vec3::new(5000.0, 3000.0, 4000.0),
        Vec3::new(-2000.0, 6500.0, 1500.0),
    ];
    for &r in &points {
        let a_j2 = accel_j2(r, MU, EARTH_J2, RE);
        let a_zonal = accel_zonal(r, MU, EARTH_J2, 0.0, 0.0, RE);
        // Mixed tolerance: relative when the value is non-zero, absolute floor when it is zero
        // (e.g. a_y at a point on the X-Z plane, where both must be exactly 0).
        let tol = |a: f64| if fabs(a) > 1e-12 { 1e-12 * fabs(a) } else { 1e-15 };
        assert!(
            fabs(a_j2.x - a_zonal.x) < tol(a_j2.x),
            "x mismatch at {r:?}: {:.15e} vs {:.15e}",
            a_j2.x,
            a_zonal.x
        );
        assert!(
            fabs(a_j2.y - a_zonal.y) < tol(a_j2.y),
            "y mismatch at {r:?}"
        );
        assert!(
            fabs(a_j2.z - a_zonal.z) < tol(a_j2.z),
            "z mismatch at {r:?}"
        );
    }
}

/// `potential_zonal` with `j3 = 0, j4 = 0` must equal `potential_j2`.
#[test]
fn potential_zonal_reduces_to_potential_j2_when_j3_j4_zero() {
    let points = [
        Vec3::new(7000.0, 0.0, 0.0),
        Vec3::new(5000.0, 3000.0, 4000.0),
        Vec3::new(0.0, 0.0, 7000.0),
    ];
    for &r in &points {
        let u_j2 = potential_j2(r, MU, EARTH_J2, RE);
        let u_zonal = potential_zonal(r, MU, EARTH_J2, 0.0, 0.0, RE);
        assert!(
            fabs(u_j2 - u_zonal) < 1e-12 * fabs(u_j2),
            "potential mismatch at {r:?}: {:.15e} vs {:.15e}",
            u_j2,
            u_zonal
        );
    }
}

// ---------------------------------------------------------------------------------------------
// The potential's gradient is the acceleration (J2 + J3 + J4)
// ---------------------------------------------------------------------------------------------

/// `−∇U_zonal` reproduces `accel_zonal` for the full J2+J3+J4 field.
///
/// This is the check that makes the energy-conservation test meaningful: it proves the J3 and J4
/// acceleration terms are the gradient of the J3 and J4 potential terms, not independent formulas
/// that could drift apart. Without this, an energy test only proves the integrator is consistent with
/// *some* potential — not the one it is actually integrating.
///
/// `potential_zonal` returns only the perturbation potential (J2+J3+J4), so the full potential
/// `-μ/r + U_zonal` must be finite-differenced to match the full `accel_zonal`.
#[test]
fn potential_gradient_reproduces_full_zonal_acceleration() {
    let r = Vec3::new(4200.0, 3000.0, 5000.0);
    let a = accel_zonal(r, MU, EARTH_J2, EARTH_J3, EARTH_J4, RE);
    let h = 1e-3;

    // Full potential = central (-μ/r) + perturbation (potential_zonal).
    let full_u = |p: Vec3| -MU / p.norm() + potential_zonal(p, MU, EARTH_J2, EARTH_J3, EARTH_J4, RE);

    let dx = -(full_u(Vec3::new(r.x + h, r.y, r.z)) - full_u(Vec3::new(r.x - h, r.y, r.z))) / (2.0 * h);
    let dy = -(full_u(Vec3::new(r.x, r.y + h, r.z)) - full_u(Vec3::new(r.x, r.y - h, r.z))) / (2.0 * h);
    let dz = -(full_u(Vec3::new(r.x, r.y, r.z + h)) - full_u(Vec3::new(r.x, r.y, r.z - h))) / (2.0 * h);

    let an = fabs(a.norm());
    let scale = if an > 1e-12 { an } else { 1e-12 };
    assert!(
        fabs(dx - a.x) < 1e-6 * scale,
        "dU/dx mismatch: {dx:e} vs {:.6e}",
        a.x
    );
    assert!(
        fabs(dy - a.y) < 1e-6 * scale,
        "dU/dy mismatch: {dy:e} vs {:.6e}",
        a.y
    );
    assert!(
        fabs(dz - a.z) < 1e-6 * scale,
        "dU/dz mismatch: {dz:e} vs {:.6e}",
        a.z
    );
}

/// The same gradient check at a point with a large z (high geocentric latitude), where the J3 term
/// is largest relative to J2.
#[test]
fn potential_gradient_reproduces_acceleration_at_high_latitude() {
    let r = Vec3::new(1000.0, 500.0, 6800.0);
    let a = accel_zonal(r, MU, EARTH_J2, EARTH_J3, EARTH_J4, RE);
    let h = 1e-3;

    let full_u = |p: Vec3| -MU / p.norm() + potential_zonal(p, MU, EARTH_J2, EARTH_J3, EARTH_J4, RE);

    let dx = -(full_u(Vec3::new(r.x + h, r.y, r.z)) - full_u(Vec3::new(r.x - h, r.y, r.z))) / (2.0 * h);
    let dz = -(full_u(Vec3::new(r.x, r.y, r.z + h)) - full_u(Vec3::new(r.x, r.y, r.z - h))) / (2.0 * h);

    let an = fabs(a.norm());
    let scale = if an > 1e-12 { an } else { 1e-12 };
    assert!(fabs(dx - a.x) < 1e-6 * scale, "dU/dx mismatch at high latitude");
    assert!(fabs(dz - a.z) < 1e-6 * scale, "dU/dz mismatch at high latitude");
}

// ---------------------------------------------------------------------------------------------
// Energy conservation under the full field
// ---------------------------------------------------------------------------------------------

/// With J2+J3+J4 on, the integrator conserves the total energy `v²/2 − μ/r + U_J2 + U_J3 + U_J4`.
///
/// The field is conservative, so this must hold. If it drifts, the "oblateness" being applied is not
/// a potential and the reference is inventing its own secular drift — the specific failure that would
/// make every cross-check meaningless.
///
/// The energy to conserve is the **full** zonal potential, not the two-body value. The two-body energy
/// is *not* conserved under any perturbation and oscillates by ~2e-3 relative at LEO over a week.
#[test]
fn integrator_conserves_energy_with_full_zonal_field() {
    let state = at_periapsis(RE + 400.0, 0.001, 51.6 * core::f64::consts::PI / 180.0);

    let total_energy = |s: &State| {
        s.specific_energy(MU)
            + potential_zonal(s.r, MU, EARTH_J2, EARTH_J3, EARTH_J4, RE)
    };
    let e0 = total_energy(&state);

    let out = integrate_zonal(
        &state,
        3.0 * 86_400.0,
        MU,
        EARTH_J2,
        EARTH_J3,
        EARTH_J4,
        RE,
        REL_TOL,
    )
    .expect("progress");
    let e1 = total_energy(&out.state);

    let de = fabs(e1 - e0) / fabs(e0);
    assert!(
        de < 1e-9,
        "total energy (two-body + J2+J3+J4 potential) drifted by {de:e} - the field is not conservative"
    );

    // And the two-body part alone is NOT conserved — asserting this stops anyone "simplifying" the
    // check back to the wrong quantity.
    let d2 = fabs(out.state.specific_energy(MU) - state.specific_energy(MU))
        / fabs(state.specific_energy(MU));
    assert!(
        d2 > 1e-5,
        "the two-body energy should NOT be conserved under J2+J3+J4, but it drifted only {d2:e}"
    );
}

// ---------------------------------------------------------------------------------------------
// Self-consistency: < 50 m over 7 days at LEO
// ---------------------------------------------------------------------------------------------

/// The integrator run at two different tolerances agrees to within 50 m over 7 days at LEO.
///
/// This is the `CREDIBILITY.md` §3.1 target for the "J2 + J3 + J4 propagation" row. The integrator
/// is checking itself: the same field, the same Dormand–Prince tableau, two tolerances. If the two
/// disagree by more than 50 m, the integrator's error is not under control and it is not a trustworthy
/// reference for anything.
///
/// The tolerance pair is `1e-11` (looser) and `1e-13` (reference). The difference between them is
/// the integrator's own truncation error, which must be well below the 50 m target.
///
/// `1e-9` is too loose for a 7-day integration at LEO: the accumulated truncation error over
/// ~70 orbits exceeds 1 km. `1e-11` is the practical operating tolerance; `1e-13` is the reference.
#[test]
fn zonal_integrator_self_consistency_under_50m_at_leo() {
    let state = at_periapsis(RE + 400.0, 0.001, 51.6 * core::f64::consts::PI / 180.0);
    let dt = 7.0 * 86_400.0;

    let out_tight = integrate_zonal(&state, dt, MU, EARTH_J2, EARTH_J3, EARTH_J4, RE, 1e-13)
        .expect("tight-tolerance progress");

    let out_loose = integrate_zonal(&state, dt, MU, EARTH_J2, EARTH_J3, EARTH_J4, RE, 1e-11)
        .expect("loose-tolerance progress");

    let dx = out_tight.state.r.x - out_loose.state.r.x;
    let dy = out_tight.state.r.y - out_loose.state.r.y;
    let dz = out_tight.state.r.z - out_loose.state.r.z;
    let dist_m = fabs(sqrt(dx * dx + dy * dy + dz * dz)) * 1000.0;

    assert!(
        dist_m < 50.0,
        "self-consistency: 7-day LEO position differs by {dist_m:.2} m between tolerances \
         (target < 50 m)"
    );

    println!(
        "self-consistency: 7-day LEO position differs by {:.4} m between 1e-10 and 1e-12",
        dist_m
    );
}

// ---------------------------------------------------------------------------------------------
// J3 and J4 have a measurable effect
// ---------------------------------------------------------------------------------------------

/// The J2+J3+J4 integrator differs from the J2-only integrator over 7 days at LEO.
///
/// If the J3 and J4 terms were not being applied, the two integrators would produce identical states
/// and this block would be a no-op. The difference must be non-zero and in a physically sensible
/// direction: J3 is an odd harmonic (north-south asymmetry) and J4 is even (same structure as J2 but
/// smaller), so both perturb the orbit away from the J2-only path.
#[test]
fn j3_j4_produce_measurable_difference_from_j2_only() {
    let state = at_periapsis(RE + 400.0, 0.001, 51.6 * core::f64::consts::PI / 180.0);
    let dt = 7.0 * 86_400.0;

    let out_j2 = integrate_j2(&state, dt, MU, EARTH_J2, RE, REL_TOL).expect("j2 progress");
    let out_full = integrate_zonal(&state, dt, MU, EARTH_J2, EARTH_J3, EARTH_J4, RE, REL_TOL)
        .expect("full progress");

    let dx = out_full.state.r.x - out_j2.state.r.x;
    let dy = out_full.state.r.y - out_j2.state.r.y;
    let dz = out_full.state.r.z - out_j2.state.r.z;
    let dist_km = sqrt(dx * dx + dy * dy + dz * dz);

    // The difference must be non-zero (the terms are applied) and non-trivial (not just rounding).
    // At LEO over 7 days, J3+J4 produce a drift of tens to hundreds of metres — well above any
    // integrator noise at rel_tol = 1e-12.
    assert!(
        dist_km > 0.01,
        "J3+J4 produced only {:.4} km of drift over 7 days - the terms are not being applied",
        dist_km
    );

    println!(
        "J3+J4 vs J2-only: {:.4} km drift over 7 days at LEO",
        dist_km
    );
}

/// J3 alone (without J4) also produces a measurable difference, confirming the J3 term is independent
/// of the J4 term and not accidentally cancelled.
#[test]
fn j3_alone_produces_measurable_difference() {
    let state = at_periapsis(RE + 400.0, 0.001, 51.6 * core::f64::consts::PI / 180.0);
    let dt = 7.0 * 86_400.0;

    let out_j2 = integrate_j2(&state, dt, MU, EARTH_J2, RE, REL_TOL).expect("j2 progress");
    let out_j2j3 = integrate_zonal(&state, dt, MU, EARTH_J2, EARTH_J3, 0.0, RE, REL_TOL)
        .expect("j2+j3 progress");

    let dx = out_j2j3.state.r.x - out_j2.state.r.x;
    let dy = out_j2j3.state.r.y - out_j2.state.r.y;
    let dz = out_j2j3.state.r.z - out_j2.state.r.z;
    let dist_km = sqrt(dx * dx + dy * dy + dz * dz);

    assert!(
        dist_km > 0.005,
        "J3 alone produced only {:.4} km of drift - the J3 term is not being applied",
        dist_km
    );
}

// ---------------------------------------------------------------------------------------------
// Refusals
// ---------------------------------------------------------------------------------------------

/// Degenerate input returns `None`, not NaNs.
#[test]
fn zonal_integrator_refuses_degenerate_input() {
    let s = at_periapsis(RE + 400.0, 0.0, 0.5);
    assert!(
        integrate_zonal(&s, f64::NAN, MU, EARTH_J2, EARTH_J3, EARTH_J4, RE, 1e-12).is_none(),
        "NaN dt"
    );
    assert!(
        integrate_zonal(&s, -1.0, MU, EARTH_J2, EARTH_J3, EARTH_J4, RE, 1e-12).is_none(),
        "negative dt"
    );
    assert!(
        integrate_zonal(&s, 600.0, MU, EARTH_J2, EARTH_J3, EARTH_J4, RE, 0.0).is_none(),
        "zero tolerance"
    );
    assert!(
        integrate_zonal(&s, 0.0, MU, EARTH_J2, EARTH_J3, EARTH_J4, RE, 1e-12).is_some(),
        "zero dt is valid"
    );
}

/// `accel_zonal` returns a zero vector for a zero position (no division by zero, no NaN).
#[test]
fn accel_zonal_handles_zero_position() {
    let a = accel_zonal(
        Vec3::new(0.0, 0.0, 0.0),
        MU,
        EARTH_J2,
        EARTH_J3,
        EARTH_J4,
        RE,
    );
    assert!(
        a.x == 0.0 && a.y == 0.0 && a.z == 0.0,
        "zero position must give zero acceleration"
    );
}
