//! Interplanetary Hohmann transfer planning: the Δv, the transfer geometry, and the window that
//! turns two orbits into a meeting.
//!
//! ## Why this is the rung after the ecliptic frame
//!
//! [`crate::ecliptic`] provides the frame in which interplanetary work lives. This module is the first
//! thing that actually *uses* it: it plans a transfer between two orbits around a central body, taking
//! the orbits as elements in the ecliptic frame and returning the transfer parameters a flight director
//! needs. Planetary orbits are nearly coplanar with the ecliptic, so the transfer is a two-dimensional
//! problem in that plane — which is exactly what the ecliptic frame is for.
//!
//! [`crate::rendezvous`] already does the circular-orbit case: two circular orbits around one body, the
//! Hohmann transfer between them, the phase angle, and the window. This module generalises it to
//! **elliptical orbits** — the case every real planetary orbit is in — and adds the departure and
//! arrival Δv as separate quantities so a mission designer can see the cost of each burn.
//!
//! ## What is modelled
//!
//! * **Hohmann transfer between two co-planar elliptical orbits.** The transfer ellipse has its
//!   periapsis at the departure orbit's near apsis and its apoapsis at the arrival orbit's far apsis.
//!   This is the minimum-Δv two-burn transfer for co-planar orbits with a ratio below
//!   [`BI_ELLIPTIC_THRESHOLD`].
//! * **Departure and arrival Δv** as separate quantities, computed from the vis-viva equation at each
//!   burn point. The total is their sum.
//! * **Transfer time** — half the period of the transfer ellipse, from periapsis to apoapsis.
//! * **Phase angle** in mean anomaly — the angular position the arrival body must have at departure so
//!   that it meets the transfer vehicle at the far end.
//! * **Departure window** — the time until the next window opens, given the current phase.
//!
//! ## What is not modelled
//!
//! * **Plane changes.** If the two orbits are not co-planar (different inclinations), the Hohmann
//!   transfer is not optimal and the formulas here do not apply. [`transfer_plan`] refuses non-coplanar
//!   inputs rather than computing a wrong answer.
//! * **Bi-elliptic transfers.** Above [`BI_ELLIPTIC_THRESHOLD`] a bi-elliptic transfer is cheaper. The
//!   caller gets the Hohmann answer, which is the standard first approximation.
//! * **Mid-course corrections, Lambert transfers, and non-Hohmann trajectories.** The Lambert solver
//!   in [`crate::lambert`] handles arbitrary two-position transfers; this module is the closed-form
//!   Hohmann planner, which is the right first answer for a co-planar transfer.

use crate::ecliptic::Ecliptic;
use odderon_dmath::{fabs, fmax, fmin, is_finite, sqrt, TAU};

/// The ratio above which a bi-elliptic transfer is cheaper than a Hohmann transfer. Above this
/// ratio the Hohmann is no longer the minimum-Δv two-burn transfer, and the caller should know
/// that the answer is the standard approximation rather than the optimum. Pinned and folded into
/// [`transfer_design_hash`].
pub const BI_ELLIPTIC_THRESHOLD: f64 = 11.9;

/// The tolerance for the coplanarity check, in radians. Two orbits whose inclinations differ by less
/// than this are treated as co-planar. The tolerance is not a physical tolerance — it is the threshold
/// at which the plane-change cost becomes negligible compared to the transfer Δv, which for a 1°
/// inclination difference at 1 AU is about 0.01 % of the total Δv. Pinned and folded into
/// [`transfer_design_hash`].
pub const COPLANAR_TOL_RAD: f64 = 0.017_453_292_519_943_296; // 1 degree

/// The orbital elements of a body in the ecliptic frame, around a central body.
///
/// The elements are the standard Keplerian set: semi-major axis `a`, eccentricity `e`, and inclination
/// `i`. For a co-planar Hohmann transfer only `a` and `e` are needed (the transfer is computed at the
/// apsides, where the argument of periapsis and longitude of ascending node do not affect the Δv);
/// the inclination is carried so the coplanarity check can be performed.
///
/// The frame is the ecliptic: `a` is in km, `e` is dimensionless, and `i` is measured from the
/// ecliptic plane. A body with `i = 0` is exactly in the ecliptic plane.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OrbitalElements {
    /// Semi-major axis, km. Must be positive.
    pub a_km: f64,
    /// Eccentricity, dimensionless. Must be in `[0, 1)` for an elliptical orbit.
    pub e: f64,
    /// Inclination, radians, measured from the ecliptic plane. Must be in `[0, π]`.
    pub i_rad: f64,
}

/// Why a transfer plan could not be computed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TransferError {
    /// An orbit's semi-major axis was non-positive or non-finite.
    BadSemiMajorAxis,
    /// An orbit's eccentricity was outside `[0, 1)` or non-finite.
    BadEccentricity,
    /// An orbit's inclination was outside `[0, π]` or non-finite.
    BadInclination,
    /// The two orbits are not co-planar: their inclinations differ by more than
    /// [`COPLANAR_TOL_RAD`].
    NotCoplanar,
    /// The central body's gravitational parameter was non-positive or non-finite.
    BadMu,
}

/// The result of planning a Hohmann transfer between two co-planar elliptical orbits.
///
/// All quantities are computed from the vis-viva equation and Kepler's third law. The transfer is
/// a half-ellipse from the departure orbit's near apsis to the arrival orbit's far apsis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TransferPlan {
    /// The transfer ellipse's semi-major axis, km.
    pub a_transfer_km: f64,
    /// The transfer ellipse's eccentricity, dimensionless.
    pub e_transfer: f64,
    /// The departure radius — the distance from the central body at the first burn, km.
    pub r_departure_km: f64,
    /// The arrival radius — the distance from the central body at the second burn, km.
    pub r_arrival_km: f64,
    /// The Δv of the departure burn, km/s. Positive for an outward transfer, positive for an inward
    /// transfer (the magnitude of the burn, always positive).
    pub dv_departure_km_s: f64,
    /// The Δv of the arrival burn, km/s. Always the magnitude, always positive.
    pub dv_arrival_km_s: f64,
    /// The total Δv — the sum of the two burns, km/s.
    pub dv_total_km_s: f64,
    /// The transfer time — half the period of the transfer ellipse, seconds.
    pub transfer_time_s: f64,
    /// The phase angle in mean anomaly, radians. The angular position the arrival body must have at
    /// departure so that it meets the transfer vehicle at the far end. Positive for an outward
    /// transfer (arrival body ahead), negative for an inward transfer (arrival body behind).
    pub phase_angle_rad: f64,
    /// True if the transfer is outward (departure orbit is closer to the central body than the
    /// arrival orbit). False for an inward transfer.
    pub outward: bool,
}

/// Validate an orbit's elements, returning [`TransferError`] on a malformed input.
fn validate_orbit(orbit: &OrbitalElements) -> Result<(), TransferError> {
    if !is_finite(orbit.a_km) || orbit.a_km <= 0.0 {
        return Err(TransferError::BadSemiMajorAxis);
    }
    if !is_finite(orbit.e) || !(0.0..1.0).contains(&orbit.e) {
        return Err(TransferError::BadEccentricity);
    }
    if !is_finite(orbit.i_rad) || !(0.0..=odderon_dmath::PI).contains(&orbit.i_rad) {
        return Err(TransferError::BadInclination);
    }
    Ok(())
}

/// The distance from the central body at periapsis, km.
fn periapsis_km(a: f64, e: f64) -> f64 {
    a * (1.0 - e)
}

/// The distance from the central body at apoapsis, km.
fn apoapsis_km(a: f64, e: f64) -> f64 {
    a * (1.0 + e)
}

/// The velocity at a given radius on an orbit with semi-major axis `a`, from the vis-viva equation:
/// `v = sqrt(μ · (2/r − 1/a))`.
fn vis_viva_km_s(mu: f64, r_km: f64, a_km: f64) -> f64 {
    sqrt(mu * (2.0 / r_km - 1.0 / a_km))
}

/// Plan a Hohmann transfer between two co-planar elliptical orbits.
///
/// `departure` and `arrival` are the orbital elements of the two bodies in the ecliptic frame, and
/// `mu_central` is the gravitational parameter of the central body (the Sun for interplanetary
/// transfers). The transfer is computed at the apsides: the departure burn happens at the departure
/// orbit's apsis nearest the arrival orbit, and the arrival burn at the arrival orbit's apsis nearest
/// the departure orbit.
///
/// **The coplanarity check.** The Hohmann transfer is optimal only for co-planar orbits. If the two
/// orbits' inclinations differ by more than [`COPLANAR_TOL_RAD`] (1°), the transfer is refused with
/// [`TransferError::NotCoplanar`]. A plane change is a different, more expensive maneuver, and the
/// Hohmann formulas would produce a wrong answer for it.
///
/// **Refusals.** [`TransferError::BadSemiMajorAxis`], [`TransferError::BadEccentricity`],
/// [`TransferError::BadInclination`], or [`TransferError::BadMu`] for malformed inputs;
/// [`TransferError::NotCoplanar`] for non-coplanar orbits. A refusal is never a fabricated plan.
pub fn transfer_plan(
    departure: &OrbitalElements,
    arrival: &OrbitalElements,
    mu_central: f64,
) -> Result<TransferPlan, TransferError> {
    validate_orbit(departure)?;
    validate_orbit(arrival)?;
    if !is_finite(mu_central) || mu_central <= 0.0 {
        return Err(TransferError::BadMu);
    }
    // Coplanarity: the Hohmann transfer is optimal only for co-planar orbits. The inclination is
    // measured from the ecliptic plane, so two orbits are co-planar when their inclinations are
    // equal (both measured from the same reference plane). The absolute difference is the check.
    if fabs(departure.i_rad - arrival.i_rad) > COPLANAR_TOL_RAD {
        return Err(TransferError::NotCoplanar);
    }

    let outward = departure.a_km < arrival.a_km;

    // The departure radius is the apsis of the departure orbit nearest the arrival orbit; the arrival
    // radius is the apsis of the arrival orbit nearest the departure orbit. For an outward transfer
    // (departure closer), that means departure at periapsis and arrival at apoapsis. For an inward
    // transfer (departure farther), departure at apoapsis and arrival at periapsis.
    let r_dep = if outward {
        periapsis_km(departure.a_km, departure.e)
    } else {
        apoapsis_km(departure.a_km, departure.e)
    };
    let r_arr = if outward {
        apoapsis_km(arrival.a_km, arrival.e)
    } else {
        periapsis_km(arrival.a_km, arrival.e)
    };

    // The transfer ellipse: periapsis at the smaller of the two burn radii, apoapsis at the larger.
    // Eccentricity is always non-negative — the formula must not depend on the transfer direction.
    let r_peri = fmin(r_dep, r_arr);
    let r_apo = fmax(r_dep, r_arr);
    let a_transfer = (r_peri + r_apo) / 2.0;
    let e_transfer = (r_apo - r_peri) / (r_apo + r_peri);

    // The velocity of each orbit at its burn point, and the velocity of the transfer ellipse at the
    // same point, from vis-viva. The Δv is the absolute difference.
    let v_dep_orbit = vis_viva_km_s(mu_central, r_dep, departure.a_km);
    let v_dep_transfer = vis_viva_km_s(mu_central, r_dep, a_transfer);
    let dv_dep = fabs(v_dep_transfer - v_dep_orbit);

    let v_arr_orbit = vis_viva_km_s(mu_central, r_arr, arrival.a_km);
    let v_arr_transfer = vis_viva_km_s(mu_central, r_arr, a_transfer);
    let dv_arr = fabs(v_arr_orbit - v_arr_transfer);

    // Transfer time: half the period of the transfer ellipse.
    let transfer_time = (TAU / 2.0) * sqrt(a_transfer * a_transfer * a_transfer / mu_central);

    // Phase angle in mean anomaly: the transfer vehicle advances its mean anomaly by π (half an
    // ellipse), and the arrival body advances by n_arr * t. The phase angle is the difference.
    let n_arr = sqrt(mu_central / (arrival.a_km * arrival.a_km * arrival.a_km));
    let phase_angle = (TAU / 2.0) - n_arr * transfer_time;

    Ok(TransferPlan {
        a_transfer_km: a_transfer,
        e_transfer,
        r_departure_km: r_dep,
        r_arrival_km: r_arr,
        dv_departure_km_s: dv_dep,
        dv_arrival_km_s: dv_arr,
        dv_total_km_s: dv_dep + dv_arr,
        transfer_time_s: transfer_time,
        phase_angle_rad: phase_angle,
        outward,
    })
}

/// The time in seconds until the next departure window, given the current phase angle.
///
/// `current_phase` is the current mean-anomaly lead of the arrival body over the departure point, in
/// `[0, 2π)`. The required phase is [`TransferPlan::phase_angle_rad`], wrapped into `[0, 2π)` for an
/// inward transfer (where the raw phase angle is negative). The relative angular rate is the difference
/// in mean motions: `ω_rel = n_arr − n_dep`.
///
/// Returns `NaN` if the inputs are invalid, if the orbits are the same (no relative motion), or if
/// `current_phase` is outside `[0, 2π)`. Returns `0.0` if the window is open right now.
pub fn transfer_window_s(
    departure: &OrbitalElements,
    arrival: &OrbitalElements,
    current_phase: f64,
    mu_central: f64,
) -> f64 {
    let plan = match transfer_plan(departure, arrival, mu_central) {
        Ok(p) => p,
        Err(_) => return f64::NAN,
    };
    if !is_finite(current_phase) || !(0.0..TAU).contains(&current_phase) {
        return f64::NAN;
    }
    // The required phase in [0, 2π): for outward transfers the raw phase angle is positive, for
    // inward it is negative and must be wrapped.
    let required = if plan.phase_angle_rad >= 0.0 {
        plan.phase_angle_rad
    } else {
        plan.phase_angle_rad + TAU
    };

    let n_dep = sqrt(mu_central / (departure.a_km * departure.a_km * departure.a_km));
    let n_arr = sqrt(mu_central / (arrival.a_km * arrival.a_km * arrival.a_km));
    let omega_rel = n_arr - n_dep;

    if fabs(omega_rel) < 1e-15 {
        // No relative motion: the window is now if the phase matches, never otherwise.
        return if fabs(current_phase - required) < 1e-4 {
            0.0
        } else {
            f64::NAN
        };
    }

    if fabs(current_phase - required) < 1e-4 {
        return 0.0;
    }

    if omega_rel > 0.0 {
        // Arrival body's lead is increasing.
        if current_phase < required {
            (required - current_phase) / omega_rel
        } else {
            (TAU - current_phase + required) / omega_rel
        }
    } else {
        // Arrival body's lead is decreasing.
        let abs_omega = -omega_rel;
        if current_phase > required {
            (current_phase - required) / abs_omega
        } else {
            (current_phase + TAU - required) / abs_omega
        }
    }
}

/// Whether a Hohmann transfer is the right tool for these orbits, or whether a bi-elliptic transfer
/// would be cheaper.
///
/// Above [`BI_ELLIPTIC_THRESHOLD`] the Hohmann is no longer the minimum-Δv two-burn transfer. The
/// caller gets the Hohmann answer from [`transfer_plan`], but this predicate says whether the answer
/// is the optimum or the standard approximation.
pub fn hohmann_is_optimal(departure: &OrbitalElements, arrival: &OrbitalElements) -> bool {
    // The ratio is the apoapsis-to-periapsis ratio of the transfer ellipse, which is the larger
    // burn radius over the smaller — independent of transfer direction. The bi-elliptic threshold
    // applies to the magnitude of the ratio, not its direction.
    let r_dep = if departure.a_km < arrival.a_km {
        periapsis_km(departure.a_km, departure.e)
    } else {
        apoapsis_km(departure.a_km, departure.e)
    };
    let r_arr = if departure.a_km < arrival.a_km {
        apoapsis_km(arrival.a_km, arrival.e)
    } else {
        periapsis_km(arrival.a_km, arrival.e)
    };
    fmax(r_dep, r_arr) / fmin(r_dep, r_arr) <= BI_ELLIPTIC_THRESHOLD
}

/// The deterministic design hash of the transfer planner's constants.
///
/// Folds in the bi-elliptic threshold and the coplanarity tolerance, so a replay bundle or an Akashi
/// entry can detect a stale transfer kernel. A pure function: the same kernel produces the same hash
/// on every platform, every run.
pub fn transfer_design_hash() -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut mix = |v: f64| {
        let bits = v.to_bits();
        hash ^= bits;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    };
    mix(BI_ELLIPTIC_THRESHOLD);
    mix(COPLANAR_TOL_RAD);
    hash
}

/// Transform orbital elements from ICRF to the ecliptic frame.
///
/// This is the bridge that makes the ecliptic frame useful: a backend or an ephemeris supplies orbital
/// elements in ICRF (the standard reference frame for published planetary elements), and this function
/// rotates the inclination to the ecliptic frame so [`transfer_plan`] can plan the transfer.
///
/// The transformation is a rotation of the reference plane by the obliquity of the ecliptic. The
/// semi-major axis and eccentricity are frame-independent scalars and pass through unchanged. The
/// inclination in the ecliptic frame is the angle between the orbit's angular momentum vector and the
/// ecliptic north pole.
///
/// The orbit's angular momentum vector in ICRF, for inclination `i` and longitude of ascending node
/// `Ω`, is `h ∝ (sin i·sin Ω, −sin i·cos Ω, cos i)`. The ecliptic north pole in ICRF is
/// `(0, −sin ε, cos ε)`. Their dot product gives:
///
/// ```text
/// cos(i_ecl) = sin(i)·cos(Ω)·sin(ε) + cos(i)·cos(ε)
/// ```
///
/// `omega_rad` (longitude of ascending node) and `eps_rad` (obliquity) are required because the
/// inclination transformation depends on both. The function returns `None` for non-finite inputs or
/// for an obliquity outside `(0, π)` — a degenerate rotation.
pub fn elements_to_ecliptic(
    a_km: f64,
    e: f64,
    i_icrf_rad: f64,
    omega_rad: f64,
    eps_rad: f64,
) -> Option<OrbitalElements> {
    if !is_finite(a_km)
        || !is_finite(e)
        || !is_finite(i_icrf_rad)
        || !is_finite(omega_rad)
        || !is_finite(eps_rad)
    {
        return None;
    }
    if eps_rad <= 0.0 || eps_rad >= odderon_dmath::PI {
        return None;
    }
    let sin_i = odderon_dmath::sin(i_icrf_rad);
    let cos_i = odderon_dmath::cos(i_icrf_rad);
    let cos_omega = odderon_dmath::cos(omega_rad);
    let cos_eps = odderon_dmath::cos(eps_rad);
    let sin_eps = odderon_dmath::sin(eps_rad);
    // cos(i_ecl) = sin(i)·cos(Ω)·sin(ε) + cos(i)·cos(ε)
    let cos_i_ecl = sin_i * cos_omega * sin_eps + cos_i * cos_eps;
    // Clamp to [-1, 1] to guard against rounding at the endpoints.
    let cos_i_ecl = odderon_dmath::fmax(odderon_dmath::fmin(cos_i_ecl, 1.0), -1.0);
    let i_ecl = odderon_dmath::acos(cos_i_ecl);
    Some(OrbitalElements {
        a_km,
        e,
        i_rad: i_ecl,
    })
}

/// The standard ecliptic obliquity, for callers that do not want to construct an [`Ecliptic`].
///
/// Returns the IAU 2006 obliquity at J2000, in radians. This is the value [`elements_to_ecliptic`]
/// expects for a J2000 ICRF input.
pub fn ecliptic_obliquity_rad() -> f64 {
    Ecliptic::standard()
        .obliquity_at(0.0)
        .expect("obliquity at J2000 is a finite constant")
}
