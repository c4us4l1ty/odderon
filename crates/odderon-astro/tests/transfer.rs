//! Interplanetary Hohmann transfer tests: the transfer planner and the ICRF → ecliptic transform.
//!
//! The checks are not "does it run." They are the physical properties a Hohmann transfer must
//! satisfy, computed independently in Python and pinned here:
//!
//! * **Circular-orbit reduction:** for circular orbits, the planner's Δv must match the textbook
//!   Hohmann formula and the rendezvous module's answer.
//! * **Elliptical transfer:** the transfer ellipse's elements, the burn radii, the Δv, the transfer
//!   time, and the phase angle are checked against independently computed values.
//! * **Inward transfer:** the same quantities for a transfer from a higher orbit to a lower one,
//!   where the phase angle is negative and the eccentricity must still be non-negative.
//! * **Departure window:** the time-to-window for a given current phase, in both directions.
//! * **Coplanarity refusal:** non-coplanar orbits are refused, not answered.
//! * **Input validation:** malformed orbits and parameters are refused with the right error.
//! * **Bi-elliptic threshold:** the predicate says whether the Hohmann answer is the optimum.
//! * **ICRF → ecliptic:** the inclination transformation is checked against the rotation formula.

use odderon_astro::transfer::*;
use odderon_astro::mu;
use odderon_dmath::{fabs, PI, TAU};

// ---------------------------------------------------------------------------------------------
// Constants and helpers
// ---------------------------------------------------------------------------------------------

/// One astronomical unit, km.
const AU: f64 = 149_597_870.7;

/// The Sun's gravitational parameter, km³/s².
const SUN_MU: f64 = mu::SUN;

/// A circular orbit at `a_km` AU with zero inclination (exactly in the ecliptic).
fn circular(a_au: f64) -> OrbitalElements {
    OrbitalElements {
        a_km: a_au * AU,
        e: 0.0,
        i_rad: 0.0,
    }
}

/// An elliptical orbit at `a_km` AU with eccentricity `e` and zero inclination.
fn elliptical(a_au: f64, e: f64) -> OrbitalElements {
    OrbitalElements {
        a_km: a_au * AU,
        e,
        i_rad: 0.0,
    }
}

/// Assert two values are equal within a tolerance.
///
/// Uses relative tolerance when the expected value is non-zero, absolute tolerance when it is zero
/// (dividing by zero would give NaN, which is not a meaningful comparison).
fn approx_eq(a: f64, b: f64, tol: f64, label: &str) {
    let err = fabs(a - b);
    let ok = if b == 0.0 {
        err < tol
    } else {
        err / fabs(b) < tol
    };
    assert!(ok, "{label}: got {a}, expected {b}, error {err}");
}

// ---------------------------------------------------------------------------------------------
// Circular-orbit reduction
// ---------------------------------------------------------------------------------------------

/// For circular orbits, the planner must produce the textbook Hohmann transfer.
///
/// The expected values are computed independently in Python from the vis-viva equation and Kepler's
/// third law — not from the code's own arithmetic. For a 1 AU → 1.5 AU transfer around the Sun:
/// transfer semi-major axis 1.25 AU, eccentricity 0.2, total Δv ≈ 5.41 km/s, transfer time
/// ≈ 255.23 days, phase angle ≈ 43.07°.
#[test]
fn circular_transfer_matches_textbook_values() {
    let dep = circular(1.0);
    let arr = circular(1.5);
    let plan = transfer_plan(&dep, &arr, SUN_MU).expect("coplanar circular orbits");

    // Transfer ellipse: a = (r_dep + r_arr) / 2 = (1.0 + 1.5) / 2 AU = 1.25 AU.
    approx_eq(plan.a_transfer_km, 1.25 * AU, 1e-12, "a_transfer");
    // e = (r_arr - r_dep) / (r_arr + r_dep) = 0.5 / 2.5 = 0.2.
    approx_eq(plan.e_transfer, 0.2, 1e-12, "e_transfer");
    // Departure radius: periapsis of the departure orbit = 1.0 AU.
    approx_eq(plan.r_departure_km, 1.0 * AU, 1e-12, "r_departure");
    // Arrival radius: apoapsis of the arrival orbit = 1.5 AU.
    approx_eq(plan.r_arrival_km, 1.5 * AU, 1e-12, "r_arrival");

    // Δv values from Python: dv_dep ≈ 2.8428033377, dv_arr ≈ 2.5674355987, total ≈ 5.4102389364.
    approx_eq(plan.dv_departure_km_s, 2.842_803_337_7, 1e-9, "dv_departure");
    approx_eq(plan.dv_arrival_km_s, 2.567_435_598_7, 1e-9, "dv_arrival");
    approx_eq(plan.dv_total_km_s, 5.410_238_936_4, 1e-9, "dv_total");

    // Transfer time: π · sqrt(a³/μ) ≈ 22051959.86 s ≈ 255.231 days.
    approx_eq(plan.transfer_time_s, 22_051_959.855_527, 1e-6, "transfer_time_s");
    // Phase angle: π − n_arr · t ≈ 0.7517021496 rad ≈ 43.069°.
    approx_eq(plan.phase_angle_rad, 0.751_702_149_6, 1e-9, "phase_angle");

    assert!(plan.outward, "1 AU → 1.5 AU is an outward transfer");
}

/// The circular case must agree with the rendezvous module's answer.
///
/// This is a cross-module consistency check: the transfer planner generalises the rendezvous module
/// from circular to elliptical orbits, so for circular orbits the two must agree. If they disagree,
/// one of them has a bug.
#[test]
fn circular_transfer_agrees_with_rendezvous() {
    let dep = circular(1.0);
    let arr = circular(1.5);
    let plan = transfer_plan(&dep, &arr, SUN_MU).expect("coplanar circular orbits");

    let r_from = dep.a_km; // circular: r = a
    let r_to = arr.a_km;
    let expected_dv = odderon_astro::rendezvous::rendezvous_transfer_dv(r_from, r_to, SUN_MU);

    approx_eq(
        plan.dv_total_km_s,
        expected_dv,
        1e-10,
        "transfer dv vs rendezvous dv",
    );

    let expected_time = odderon_astro::rendezvous::rendezvous_transfer_time_s(r_from, r_to, SUN_MU);
    approx_eq(
        plan.transfer_time_s,
        expected_time,
        1e-10,
        "transfer time vs rendezvous time",
    );
}

// ---------------------------------------------------------------------------------------------
// Elliptical transfer
// ---------------------------------------------------------------------------------------------

/// An elliptical transfer: 1 AU (e=0.1) → 1.5 AU (e=0.05).
///
/// The expected values are computed independently in Python. The departure burn is at periapsis of
/// the departure orbit (0.9 AU), the arrival burn at apoapsis of the arrival orbit (1.575 AU).
#[test]
fn elliptical_transfer_matches_independent_values() {
    let dep = elliptical(1.0, 0.1);
    let arr = elliptical(1.5, 0.05);
    let plan = transfer_plan(&dep, &arr, SUN_MU).expect("coplanar elliptical orbits");

    // r_dep = 0.9 AU, r_arr = 1.575 AU.
    approx_eq(plan.r_departure_km, 0.9 * AU, 1e-12, "r_departure");
    approx_eq(plan.r_arrival_km, 1.575 * AU, 1e-12, "r_arrival");
    // a_transfer = (0.9 + 1.575) / 2 = 1.2375 AU.
    approx_eq(plan.a_transfer_km, 1.237_5 * AU, 1e-12, "a_transfer");
    // e_transfer = (1.575 - 0.9) / (1.575 + 0.9) = 0.675 / 2.475 = 0.272727...
    approx_eq(plan.e_transfer, 0.272_727_272_7, 1e-9, "e_transfer");

    // Python: dv_dep ≈ 2.4910481592, dv_arr ≈ 2.8924978973, total ≈ 5.3835460565.
    approx_eq(
        plan.dv_departure_km_s,
        2.491_048_159_2,
        1e-9,
        "dv_departure",
    );
    approx_eq(plan.dv_arrival_km_s, 2.892_497_897_3, 1e-9, "dv_arrival");
    approx_eq(plan.dv_total_km_s, 5.383_546_056_5, 1e-9, "dv_total");

    // Python: t_transfer ≈ 21722008.79 s ≈ 251.412 days.
    approx_eq(
        plan.transfer_time_s,
        21_722_008.789_630,
        1e-6,
        "transfer_time_s",
    );
    // Python: phase ≈ 0.7874607363 rad ≈ 45.118°.
    approx_eq(plan.phase_angle_rad, 0.787_460_736_3, 1e-9, "phase_angle");

    assert!(plan.outward);
}

// ---------------------------------------------------------------------------------------------
// Inward transfer
// ---------------------------------------------------------------------------------------------

/// An inward transfer: 1.5 AU → 1.0 AU (both circular).
///
/// The phase angle is negative (the arrival body must be behind, not ahead), the eccentricity must
/// still be non-negative, and the total Δv equals the outward case's (the burns are the same
/// magnitudes, just applied in reverse order).
#[test]
fn inward_transfer_has_negative_phase_and_nonnegative_eccentricity() {
    let dep = circular(1.5);
    let arr = circular(1.0);
    let plan = transfer_plan(&dep, &arr, SUN_MU).expect("coplanar circular orbits");

    assert!(!plan.outward, "1.5 AU → 1.0 AU is an inward transfer");
    // Eccentricity must be non-negative regardless of direction.
    assert!(
        plan.e_transfer >= 0.0,
        "eccentricity must be non-negative, got {}",
        plan.e_transfer
    );
    approx_eq(plan.e_transfer, 0.2, 1e-12, "e_transfer (inward)");

    // r_dep = apoapsis of departure = 1.5 AU, r_arr = periapsis of arrival = 1.0 AU.
    approx_eq(plan.r_departure_km, 1.5 * AU, 1e-12, "r_departure");
    approx_eq(plan.r_arrival_km, 1.0 * AU, 1e-12, "r_arrival");

    // The total Δv equals the outward case's total (same burn magnitudes, reversed).
    approx_eq(
        plan.dv_total_km_s,
        5.410_238_936_4,
        1e-9,
        "dv_total (inward)",
    );
    // Transfer time is the same (same ellipse, same half-period).
    approx_eq(
        plan.transfer_time_s,
        22_051_959.855_527,
        1e-6,
        "transfer_time_s (inward)",
    );
    // Phase angle is negative: π − n_arr · t where n_arr is the inner orbit's mean motion.
    // Python: phase ≈ -1.2489165533 rad ≈ -71.558°.
    approx_eq(
        plan.phase_angle_rad,
        -1.248_916_553_3,
        1e-9,
        "phase_angle (inward)",
    );
}

// ---------------------------------------------------------------------------------------------
// Departure window
// ---------------------------------------------------------------------------------------------

/// The window is open now when the current phase equals the required phase.
#[test]
fn window_is_open_now_when_phase_matches() {
    let dep = circular(1.0);
    let arr = circular(1.5);
    let plan = transfer_plan(&dep, &arr, SUN_MU).expect("valid");
    let required = plan.phase_angle_rad; // positive for outward
    assert_eq!(transfer_window_s(&dep, &arr, required, SUN_MU), 0.0);
}

/// The window time is positive when the phase is short of the required value.
#[test]
fn window_time_is_positive_when_phase_is_short() {
    let dep = circular(1.0);
    let arr = circular(1.5);
    let plan = transfer_plan(&dep, &arr, SUN_MU).expect("valid");
    let required = plan.phase_angle_rad;
    // A phase slightly less than required: the window is in the future.
    let short = required - 0.01;
    let t = transfer_window_s(&dep, &arr, short, SUN_MU);
    assert!(t > 0.0, "window time must be positive, got {t}");
    // For 1 AU → 1.5 AU the arrival body (1.5 AU) moves slower than the departure body (1 AU),
    // so omega_rel = n_arr - n_dep < 0: the lead is decreasing. When current_phase < required,
    // the lead must wrap around 2π: time = (current_phase + TAU - required) / |omega_rel|.
    let n_dep = odderon_dmath::sqrt(SUN_MU / (dep.a_km * dep.a_km * dep.a_km));
    let n_arr = odderon_dmath::sqrt(SUN_MU / (arr.a_km * arr.a_km * arr.a_km));
    let omega_rel = n_arr - n_dep;
    assert!(omega_rel < 0.0, "arrival body must be slower for 1→1.5 AU");
    let expected = (short + TAU - required) / fabs(omega_rel);
    approx_eq(t, expected, 1e-6, "window time");
}

/// The window wraps around 2π correctly.
#[test]
fn window_wraps_around_two_pi() {
    let dep = circular(1.0);
    let arr = circular(1.5);
    let plan = transfer_plan(&dep, &arr, SUN_MU).expect("valid");
    let required = plan.phase_angle_rad;
    // A phase past required (lead has already passed the window): must wrap.
    let past = required + 0.1;
    let t = transfer_window_s(&dep, &arr, past, SUN_MU);
    assert!(t > 0.0, "wrapped window time must be positive, got {t}");
}

/// The window refuses invalid inputs.
#[test]
fn window_refuses_invalid_inputs() {
    let dep = circular(1.0);
    let arr = circular(1.5);
    assert!(transfer_window_s(&dep, &arr, f64::NAN, SUN_MU).is_nan());
    assert!(transfer_window_s(&dep, &arr, -0.1, SUN_MU).is_nan());
    assert!(transfer_window_s(&dep, &arr, TAU + 0.1, SUN_MU).is_nan());
    // Non-coplanar orbits: the window is NaN.
    let dep_tilted = OrbitalElements {
        a_km: dep.a_km,
        e: 0.0,
        i_rad: 0.05,
    };
    assert!(transfer_window_s(&dep_tilted, &arr, 0.5, SUN_MU).is_nan());
}

// ---------------------------------------------------------------------------------------------
// Coplanarity refusal
// ---------------------------------------------------------------------------------------------

/// Orbits with different inclinations are refused, not answered with a wrong plan.
#[test]
fn non_coplanar_orbits_are_refused() {
    let dep = OrbitalElements {
        a_km: 1.0 * AU,
        e: 0.0,
        i_rad: 0.0,
    };
    let arr = OrbitalElements {
        a_km: 1.5 * AU,
        e: 0.0,
        i_rad: 0.05, // ~2.86°, well above the 1° tolerance
    };
    assert_eq!(
        transfer_plan(&dep, &arr, SUN_MU).err(),
        Some(TransferError::NotCoplanar)
    );
}

/// Orbits within the coplanarity tolerance are accepted.
#[test]
fn near_coplanar_orbits_are_accepted() {
    let dep = OrbitalElements {
        a_km: 1.0 * AU,
        e: 0.0,
        i_rad: 0.0,
    };
    let arr = OrbitalElements {
        a_km: 1.5 * AU,
        e: 0.0,
        i_rad: 0.01, // ~0.57°, within the 1° tolerance
    };
    assert!(transfer_plan(&dep, &arr, SUN_MU).is_ok());
}

// ---------------------------------------------------------------------------------------------
// Input validation
// ---------------------------------------------------------------------------------------------

/// Malformed inputs are refused with the right error, never answered.
#[test]
fn malformed_inputs_are_refused() {
    let good = circular(1.0);
    let arr = circular(1.5);

    // Non-positive semi-major axis.
    let bad_a = OrbitalElements {
        a_km: -1.0,
        e: 0.0,
        i_rad: 0.0,
    };
    assert_eq!(
        transfer_plan(&bad_a, &arr, SUN_MU).err(),
        Some(TransferError::BadSemiMajorAxis)
    );
    assert_eq!(
        transfer_plan(&bad_a, &arr, SUN_MU).err(),
        Some(TransferError::BadSemiMajorAxis)
    );

    // Eccentricity >= 1 (hyperbolic, not valid for a Hohmann transfer).
    let bad_e = OrbitalElements {
        a_km: 1.0 * AU,
        e: 1.0,
        i_rad: 0.0,
    };
    assert_eq!(
        transfer_plan(&bad_e, &arr, SUN_MU).err(),
        Some(TransferError::BadEccentricity)
    );

    // Inclination outside [0, π].
    let bad_i = OrbitalElements {
        a_km: 1.0 * AU,
        e: 0.0,
        i_rad: -0.1,
    };
    assert_eq!(
        transfer_plan(&bad_i, &arr, SUN_MU).err(),
        Some(TransferError::BadInclination)
    );

    // Non-positive mu.
    assert_eq!(
        transfer_plan(&good, &arr, -1.0).err(),
        Some(TransferError::BadMu)
    );
    assert_eq!(
        transfer_plan(&good, &arr, 0.0).err(),
        Some(TransferError::BadMu)
    );
    assert_eq!(
        transfer_plan(&good, &arr, f64::NAN).err(),
        Some(TransferError::BadMu)
    );
}

/// NaN inputs are refused.
#[test]
fn nan_inputs_are_refused() {
    let arr = circular(1.5);
    let bad = OrbitalElements {
        a_km: f64::NAN,
        e: 0.0,
        i_rad: 0.0,
    };
    assert_eq!(
        transfer_plan(&bad, &arr, SUN_MU).err(),
        Some(TransferError::BadSemiMajorAxis)
    );
}

// ---------------------------------------------------------------------------------------------
// Bi-elliptic threshold
// ---------------------------------------------------------------------------------------------

/// A moderate ratio is within the bi-elliptic threshold: Hohmann is optimal.
#[test]
fn hohmann_is_optimal_for_moderate_ratios() {
    let dep = circular(1.0);
    let arr = circular(1.5);
    assert!(hohmann_is_optimal(&dep, &arr));
}

/// A very large ratio exceeds the threshold: Hohmann is not optimal.
#[test]
fn hohmann_is_not_optimal_for_extreme_ratios() {
    let dep = circular(1.0);
    let arr = circular(20.0); // ratio 20 > 11.9
    assert!(!hohmann_is_optimal(&dep, &arr));
}

/// The predicate is symmetric: the ratio is the same regardless of which orbit is "departure."
#[test]
fn hohmann_optimality_is_symmetric() {
    let a = circular(1.0);
    let b = circular(5.0);
    assert_eq!(
        hohmann_is_optimal(&a, &b),
        hohmann_is_optimal(&b, &a),
        "the bi-elliptic ratio must not depend on transfer direction"
    );
}

// ---------------------------------------------------------------------------------------------
// ICRF → ecliptic transformation
// ---------------------------------------------------------------------------------------------

/// An orbit in the ecliptic plane has ICRF inclination equal to the obliquity.
///
/// The ecliptic plane is tilted from the ICRF equatorial plane by the obliquity ε. An orbit that
/// lies exactly in the ecliptic has ICRF inclination ε (its angular momentum vector is tilted by
/// ε from the ICRF z-axis). When transformed to the ecliptic frame, its inclination must be 0.
#[test]
fn an_ecliptic_orbit_has_zero_ecliptic_inclination() {
    let eps = ecliptic_obliquity_rad();
    // An orbit in the ecliptic plane: ICRF inclination = obliquity, omega = 0.
    let result = elements_to_ecliptic(1.0 * AU, 0.0, eps, 0.0, eps).expect("valid");
    approx_eq(
        result.i_rad,
        0.0,
        1e-9,
        "ecliptic inclination of an ecliptic-plane orbit",
    );
    // a and e pass through unchanged.
    approx_eq(result.a_km, 1.0 * AU, 1e-12, "a passes through");
    approx_eq(result.e, 0.0, 1e-12, "e passes through");
}

/// An orbit in the ICRF equatorial plane has ICRF inclination 0 and ecliptic inclination ε.
///
/// The ICRF equatorial plane is tilted from the ecliptic by ε, so an orbit in the ICRF equatorial
/// plane has ecliptic inclination ε. This is the reverse of the test above and checks the
/// transformation is not a no-op.
#[test]
fn an_icrf_equatorial_orbit_has_ecliptic_inclination_equal_to_obliquity() {
    let eps = ecliptic_obliquity_rad();
    // ICRF inclination 0, omega = 0: the orbit is in the ICRF equatorial plane.
    let result = elements_to_ecliptic(1.0 * AU, 0.0, 0.0, 0.0, eps).expect("valid");
    approx_eq(
        result.i_rad,
        eps,
        1e-9,
        "ecliptic inclination of an ICRF-equatorial orbit",
    );
}

/// The transformation refuses non-finite inputs and degenerate obliquity.
#[test]
fn elements_to_ecliptic_refuses_bad_inputs() {
    assert!(elements_to_ecliptic(f64::NAN, 0.0, 0.0, 0.0, 0.5).is_none());
    assert!(elements_to_ecliptic(1.0, f64::NAN, 0.0, 0.0, 0.5).is_none());
    assert!(elements_to_ecliptic(1.0, 0.0, f64::NAN, 0.0, 0.5).is_none());
    assert!(elements_to_ecliptic(1.0, 0.0, 0.0, f64::NAN, 0.5).is_none());
    assert!(elements_to_ecliptic(1.0, 0.0, 0.0, 0.0, f64::NAN).is_none());
    // Degenerate obliquity (0 or π): no rotation is defined.
    assert!(elements_to_ecliptic(1.0, 0.0, 0.0, 0.0, 0.0).is_none());
    assert!(elements_to_ecliptic(1.0, 0.0, 0.0, 0.0, PI).is_none());
}

/// The obliquity constant matches the published IAU 2006 value.
#[test]
fn ecliptic_obliquity_matches_published_value() {
    let eps = ecliptic_obliquity_rad();
    let expected = 23.439_291_0 * PI / 180.0;
    approx_eq(eps, expected, 1e-12, "obliquity");
}

// ---------------------------------------------------------------------------------------------
// Design hash
// ---------------------------------------------------------------------------------------------

/// The design hash is deterministic and nonzero.
#[test]
fn design_hash_is_deterministic_and_nonzero() {
    let h1 = transfer_design_hash();
    let h2 = transfer_design_hash();
    assert_eq!(h1, h2, "design hash must be deterministic");
    assert_ne!(h1, 0, "design hash must be nonzero");
}
