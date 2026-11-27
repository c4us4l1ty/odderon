//! Orbital rendezvous: the Δv, the transfer geometry, and the window that turns one orbit into a
//! meeting with another.
//!
//! ## Why this is the rung after the orbit
//!
//! [`crate::rocket`] answers "does this launch reach orbit" and [`crate::propagate`] answers
//! "where is it at time `t`". Rendezvous is the next question: *given two orbits, how much Δv does
//! it cost to meet, and when can the transfer begin?* It is the mechanic `GAME-DESIGN-BIBLE.md`
//! §6 marks as "the first real rendezvous" (Block 6), and the BUILD-PLAN's orbit row names it
//! explicitly: "J2+J3+J4 for the rendezvous case."
//!
//! This module implements the **two-body Hohmann transfer** — the minimum-Δv co-planar transfer
//! between two circular orbits. It is the textbook first-order answer, and it is the right level
//! of fidelity for the same reason [`crate::rocket`] is: the physics that is modelled is exact,
//! and what is not (J2 perturbation, plane changes, mid-course corrections) is named. A rendezvous
//! in the full kernel will need J2-aware transfer planning; this module is the two-body foundation
//! that the J2 layer will build on, and the design hash records which model produced a plan.
//!
//! ## What is modelled, and what is not
//!
//! * **Co-planar, circular orbits only.** Both the departure and arrival orbits are circular
//!   (`e = 0`) and share the same orbital plane. A real rendezvous between elliptical or inclined
//!   orbits needs a plane-change and a non-Hohmann transfer; this module does not model them, and
//!   says so by refusing inputs that do not describe circular orbits.
//! * **Two-body physics.** No J2, no drag, no third-body perturbation. The transfer is a pure
//!   Kepler ellipse, the same [`crate::propagate`] that flies the reference orbit.
//! * **The Hohmann transfer is the minimum-Δv two-burn transfer.** It is optimal for co-planar
//!   circular orbits with a ratio less than ~11.9. Beyond that ratio a bi-elliptic transfer is
//!   cheaper, and this module does not compute it — the caller gets the Hohmann answer, which is
//!   the standard first approximation and the one the bible's "first real rendezvous" calls for.

use odderon_dmath::{TAU, fabs, is_finite, sqrt};

use crate::mu;

/// The ratio above which a bi-elliptic transfer is cheaper than a Hohmann transfer. Above this
/// ratio the Hohmann is no longer the minimum-Δv two-burn transfer, and the caller should know
/// that the answer is the standard approximation rather than the optimum. Pinned and folded into
/// [`rendezvous_design_hash`].
pub const BI_ELLIPTIC_THRESHOLD: f64 = 11.9;

/// The Hohmann transfer Δv between two circular orbits, in km/s.
///
/// The total Δv is the sum of the two burns: the departure burn that injects onto the transfer
/// ellipse, and the arrival burn that circularises at the target orbit. For a transfer from
/// radius `r_from` to radius `r_to` around a body with parameter `mu`:
///
/// ```text
/// a_transfer = (r_from + r_to) / 2
/// dv_dep = sqrt(mu / r_from) · |sqrt(2·r_to / (r_from + r_to)) − 1|
/// dv_arr = sqrt(mu / r_to)   · |sqrt(2·r_from / (r_from + r_to)) − 1|
/// dv_total = dv_dep + dv_arr
/// ```
///
/// The absolute values handle both outward (`r_to > r_from`) and inward (`r_to < r_from`)
/// transfers: in both cases the burn magnitudes are positive, and the total is the sum of the
/// magnitudes. A transfer from a lower orbit to a higher one burns prograde at departure and
/// prograde at arrival; from higher to lower, retrograde at both.
///
/// Returns `NaN` if either radius is non-positive or non-finite, or if `mu` is non-positive or
/// non-finite — a refusal, not an answer. If `r_from == r_to` the orbits are the same and no
/// transfer is needed: the result is `0.0`, not a degenerate ellipse.
pub fn rendezvous_transfer_dv(r_from_km: f64, r_to_km: f64, mu_body: f64) -> f64 {
    if !is_finite(r_from_km)
        || !is_finite(r_to_km)
        || !is_finite(mu_body)
        || r_from_km <= 0.0
        || r_to_km <= 0.0
        || mu_body <= 0.0
    {
        return f64::NAN;
    }
    if fabs(r_from_km - r_to_km) < 1e-6 {
        return 0.0;
    }
    let r_sum = r_from_km + r_to_km;
    // sqrt(2·r_to / (r_from + r_to)): the ratio of the transfer apoapsis speed to the circular
    // speed at the departure orbit. Above 1 for outward transfers, below 1 for inward.
    let dep_factor = sqrt(2.0 * r_to_km / r_sum);
    let arr_factor = sqrt(2.0 * r_from_km / r_sum);
    let dv_dep = sqrt(mu_body / r_from_km) * fabs(dep_factor - 1.0);
    let dv_arr = sqrt(mu_body / r_to_km) * fabs(arr_factor - 1.0);
    dv_dep + dv_arr
}

/// The half-period of the Hohmann transfer ellipse, in seconds.
///
/// The transfer ellipse has semi-major axis `a = (r_from + r_to) / 2`, and the transfer is half an
/// orbit (from periapsis to apoapsis, or vice versa). By Kepler's third law:
///
/// ```text
/// t_transfer = π · sqrt(a³ / mu) = π · sqrt((r_from + r_to)³ / (8 · mu))
/// ```
///
/// Returns `NaN` for the same reasons [`rendezvous_transfer_dv`] does. If `r_from == r_to` the
/// transfer ellipse degenerates to the circular orbit itself, and the half-period is the orbital
/// period — but since no transfer is needed, the result is `NaN`: there is no transfer to time.
pub fn rendezvous_transfer_time_s(r_from_km: f64, r_to_km: f64, mu_body: f64) -> f64 {
    if !is_finite(r_from_km)
        || !is_finite(r_to_km)
        || !is_finite(mu_body)
        || r_from_km <= 0.0
        || r_to_km <= 0.0
        || mu_body <= 0.0
        || fabs(r_from_km - r_to_km) < 1e-6
    {
        return f64::NAN;
    }
    let r_sum = r_from_km + r_to_km;
    // π · sqrt((r_from + r_to)³ / (8 · mu))
    (TAU / 2.0) * sqrt(r_sum * r_sum * r_sum / (8.0 * mu_body))
}

/// The lead angle the target must have at the moment of the transfer burn, in radians.
///
/// During the transfer time `t_transfer`, the target travels through an angle
/// `n_target · t_transfer` on its circular orbit, where `n_target = sqrt(mu / r_to³)` is its mean
/// motion. The vehicle arrives at the target orbit at angular position π (half an orbit of the
/// transfer ellipse from the departure point). So the target must start ahead of the departure
/// point by:
///
/// ```text
/// Δφ = π − n_target · t_transfer
///    = π · (1 − sqrt((r_from + r_to)³ / (8 · r_to³)))
///    = π · (1 − sqrt((1 + r_from/r_to)³ / 8))
/// ```
///
/// For an outward transfer (`r_to > r_from`), `Δφ > 0`: the target must be ahead. For an inward
/// transfer (`r_to < r_from`), `Δφ < 0`: the target must be behind, which is equivalent to being
/// ahead by `2π + Δφ`.
///
/// Returns `NaN` for the same reasons [`rendezvous_transfer_dv`] does. If `r_from == r_to` the
/// phase angle is undefined (no transfer needed): `NaN`.
pub fn rendezvous_phase_angle(r_from_km: f64, r_to_km: f64, mu_body: f64) -> f64 {
    if !is_finite(r_from_km)
        || !is_finite(r_to_km)
        || !is_finite(mu_body)
        || r_from_km <= 0.0
        || r_to_km <= 0.0
        || mu_body <= 0.0
        || fabs(r_from_km - r_to_km) < 1e-6
    {
        return f64::NAN;
    }
    // The mu cancels: the phase angle is a pure function of the radius ratio.
    // Δφ = π · (1 − sqrt((1 + r_from/r_to)³ / 8))
    let ratio = r_from_km / r_to_km;
    let one_plus = 1.0 + ratio;
    let cube = one_plus * one_plus * one_plus;
    let half_pi = TAU / 2.0;
    half_pi * (1.0 - sqrt(cube / 8.0))
}

/// The time in seconds until the next rendezvous window, given the current lead angle.
///
/// `theta` is the current angular lead of the target over the chaser, in `[0, 2π)`: the angle by
/// which the target is ahead. The required lead angle is [`rendezvous_phase_angle`] (which may be
/// negative for inward transfers; the equivalent positive lead is `2π + Δφ`).
///
/// The relative angular rate is `ω_rel = n_target − n_chaser`, where `n = sqrt(mu / r³)` is the
/// mean motion. The lead angle changes at this rate. The time to the next window is the time for
/// the lead to reach the required value, wrapping around at `2π` if necessary.
///
/// Returns `NaN` if the radii or `mu` are invalid, if `r_from == r_to` (no relative motion, so the
/// window is either now or never), or if `theta` is non-finite or outside `[0, 2π)`. Returns `0.0`
/// if the window is open right now (the current lead equals the required lead).
pub fn rendezvous_window_s(r_from_km: f64, r_to_km: f64, theta: f64, mu_body: f64) -> f64 {
    if !is_finite(r_from_km)
        || !is_finite(r_to_km)
        || !is_finite(theta)
        || !is_finite(mu_body)
        || r_from_km <= 0.0
        || r_to_km <= 0.0
        || mu_body <= 0.0
        || fabs(r_from_km - r_to_km) < 1e-6
        || theta < 0.0
        || theta >= TAU
    {
        return f64::NAN;
    }
    let dphi = rendezvous_phase_angle(r_from_km, r_to_km, mu_body);
    if !is_finite(dphi) {
        return f64::NAN;
    }
    // The required lead in [0, 2π): for outward transfers dphi > 0, use it directly. For inward
    // transfers dphi < 0, the target must be behind, which is equivalent to being ahead by
    // 2π + dphi.
    let required = if dphi >= 0.0 { dphi } else { dphi + TAU };

    let n_from = sqrt(mu_body / (r_from_km * r_from_km * r_from_km));
    let n_to = sqrt(mu_body / (r_to_km * r_to_km * r_to_km));
    let omega_rel = n_to - n_from; // rate at which the target's lead changes

    if omega_rel == 0.0 {
        // No relative motion (same orbit): the window is now if the lead matches, never otherwise.
        return if fabs(theta - required) < 1e-4 { 0.0 } else { f64::NAN };
    }

    // The window is open right now: the current lead is at the required lead. A tolerance is used
    // because the lead angle arrives from a degree input field, and a degree-rounded value will not
    // equal the exact radian phase angle. A tolerance of 1e-4 rad (~20 arcseconds) is far below
    // any practical orbital-mechanics precision and above the rounding of a degree input.
    if fabs(theta - required) < 1e-4 {
        return 0.0;
    }

    if omega_rel > 0.0 {
        // Target's lead is increasing. If theta < required, wait for it to grow. If theta >
        // required, the lead has already passed the window; wait for it to wrap around.
        if theta < required {
            (required - theta) / omega_rel
        } else {
            (TAU - theta + required) / omega_rel
        }
    } else {
        // Target's lead is decreasing (chaser is faster, i.e. chaser in lower orbit). If theta >
        // required, wait for it to shrink. If theta < required, wait for it to wrap around.
        let abs_omega = -omega_rel;
        if theta > required {
            (theta - required) / abs_omega
        } else {
            (theta + TAU - required) / abs_omega
        }
    }
}

/// The deterministic design hash of the rendezvous module's constants.
///
/// Folds in the bi-elliptic threshold and Earth's gravitational parameter, so a replay bundle or
/// an Akashi entry can detect a stale rendezvous kernel. A pure function: the same kernel produces
/// the same hash on every platform, every run.
pub fn rendezvous_design_hash() -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut mix = |v: f64| {
        let bits = v.to_bits();
        hash ^= bits;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    };
    mix(BI_ELLIPTIC_THRESHOLD);
    mix(mu::EARTH);
    hash
}
