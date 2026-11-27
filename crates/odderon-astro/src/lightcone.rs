//! The deeper light-cone structure of comms: *not just how long a signal takes, but what the causal
//! structure of the sky looks like.*
//!
//! [`crate::lighttime`] answers the first question — "how long does a signal take to cross the
//! distance between two positions?" — and the causal-mail panel is built on it. This module answers
//! the questions that come *after* light-time, the ones that turn a delay into a causal structure:
//!
//! * **The light-cone envelope** ([`light_cone_radius_km`]): the radius of the region a signal can
//!   reach after a given time. This is the boundary of causal influence — beyond it, no signal sent
//!   now can arrive yet.
//! * **Causal reachability** ([`causal_reachable`]): whether a position at a given distance is
//!   within the light cone at a given time. This is the kernel's answer to "can this peer receive
//!   my signal yet?"
//! * **The relativistic Doppler shift** ([`doppler_factor`]): a signal from a moving source is
//!   shifted in frequency by its relative motion — the classic relativistic effect. This is *deeper*
//!   than light-time because it accounts for the source's velocity, not just its distance.
//! * **Signal attenuation** ([`signal_power_ratio`]): the inverse-square law — how much of the
//!   transmitted power survives the crossing. This is the physical reason a distant peer's signal
//!   is weak, not just delayed.
//!
//! ## Why these are kernel functions
//!
//! Each is a pure function of physics constants and the caller's inputs, and each carries a constant
//! that must appear exactly once in the tree: the speed of light (already owned by
//! [`crate::lighttime`], reused here, never duplicated), and the inverse-square law's exponent. A
//! page that computed any of these from a local copy of the constant would be the same defect this
//! project has found in the room, the orbit, the pass, the rendezvous and the arc — a number the
//! panel reports that the kernel did not produce. The page reads the kernel's answers; it does not
//! re-derive them.
//!
//! ## The refusal discipline
//!
//! Every function here refuses a degenerate input rather than answering with a number the caller
//! cannot tell apart from a real one. A non-finite time, a non-finite distance, a velocity that
//! exceeds the speed of light, or a zero reference distance — each returns `None` (or `false` for
//! the reachability predicate), never a fabricated value. The same discipline [`crate::lighttime`]
//! and [`crate::passes`] use.

use odderon_dmath::{cos, pow, sqrt};

use crate::lighttime::SPEED_OF_LIGHT_KM_S;

/// The inverse-square exponent: signal power falls as the distance squared.
///
/// This is the physical law, not a fitted parameter. It is the only copy in the tree; a second
/// constant elsewhere is the defect this module exists to prevent.
pub const INVERSE_SQUARE_EXPONENT: f64 = 2.0;

/// The radius of the light cone after `dt_s` seconds, in km.
///
/// The region a signal can reach from a point after time `dt_s` is a sphere of radius
/// `SPEED_OF_LIGHT_KM_S * dt_s`. This is the boundary of causal influence: beyond it, no signal
/// sent now can arrive yet. Returns `None` for a non-finite or non-positive time — a signal that
/// has not been sent (`dt_s <= 0`) has not expanded the cone at all, and a non-finite time has no
/// meaningful radius.
#[inline]
pub fn light_cone_radius_km(dt_s: f64) -> Option<f64> {
    if dt_s.is_finite() && dt_s > 0.0 {
        Some(SPEED_OF_LIGHT_KM_S * dt_s)
    } else {
        None
    }
}

/// Whether a position at distance `r_km` is within the light cone after `dt_s` seconds.
///
/// The position is causally reachable — a signal sent now can reach it — if and only if
/// `r_km <= SPEED_OF_LIGHT_KM_S * dt_s`. Returns `false` when either input is non-finite or
/// non-positive, or when the distance is non-finite: a position the kernel cannot place has no
/// reachability, and a non-positive time has not expanded the cone.
///
/// This is a predicate, not an `Option`: the answer is always a definite `true` or `false` for
/// well-formed inputs, and `false` is the correct answer for inputs the kernel refuses to place.
#[inline]
pub fn causal_reachable(r_km: f64, dt_s: f64) -> bool {
    match (light_cone_radius_km(dt_s), r_km.is_finite()) {
        (Some(radius), true) => r_km <= radius,
        _ => false,
    }
}

/// The relativistic Doppler factor: `f_received / f_emitted` for a source moving at `v_km_s`
/// relative to the observer, with the signal traveling at angle `theta_rad` to the velocity
/// vector.
///
/// The formula is the standard special-relativistic Doppler shift:
///
/// ```text
/// f_received / f_emitted = sqrt(1 - beta^2) / (1 - beta * cos(theta))
/// ```
///
/// where `beta = v / c` and `theta` is the angle between the source's velocity vector and the
/// direction from the source to the observer. The three classic cases:
///
/// * **Approaching** (`theta = 0`, `cos(theta) = 1`): the factor is `sqrt((1 + beta) / (1 - beta))`
///   — a blueshift, the received frequency is higher.
/// * **Receding** (`theta = pi`, `cos(theta) = -1`): the factor is `sqrt((1 - beta) / (1 + beta))`
///   — a redshift, the received frequency is lower.
/// * **Transverse** (`theta = pi/2`, `cos(theta) = 0`): the factor is `sqrt(1 - beta^2)` — the
///   purely relativistic transverse shift, a redshift even though there is no line-of-sight motion.
///
/// Returns `None` when the velocity is not finite, the angle is not finite, or the velocity
/// reaches or exceeds the speed of light (`beta >= 1`). A source that cannot move, or that moves at
/// or above light speed, has no physical Doppler factor — refusing is correct.
#[inline]
pub fn doppler_factor(v_km_s: f64, theta_rad: f64) -> Option<f64> {
    if !v_km_s.is_finite() || !theta_rad.is_finite() {
        return None;
    }
    let beta = v_km_s / SPEED_OF_LIGHT_KM_S;
    if beta >= 1.0 {
        return None;
    }
    let cos_theta = cos(theta_rad);
    let denom = 1.0 - beta * cos_theta;
    if denom <= 0.0 {
        return None;
    }
    let shift = sqrt(1.0 - beta * beta) / denom;
    if shift.is_finite() {
        Some(shift)
    } else {
        None
    }
}

/// The inverse-square attenuation: the fraction of transmitted signal power that survives at
/// distance `d_km`, relative to the power at reference distance `d_ref_km`.
///
/// The inverse-square law: power falls as `1 / d^2`, so the ratio of received power at `d_km` to
/// the reference power at `d_ref_km` is `(d_ref_km / d_km)^2`. A signal twice as far carries a
/// quarter of the power; ten times as far, one hundredth.
///
/// Returns `None` when either distance is non-finite or non-positive, or when `d_km` is zero
/// (a signal that has not crossed any distance has no attenuation to speak of, and the ratio would
/// be infinite). The reference distance must also be positive for the ratio to be meaningful.
#[inline]
pub fn signal_power_ratio(d_km: f64, d_ref_km: f64) -> Option<f64> {
    if d_km.is_finite() && d_km > 0.0 && d_ref_km.is_finite() && d_ref_km > 0.0 {
        let ratio = d_ref_km / d_km;
        let power = pow(ratio, INVERSE_SQUARE_EXPONENT);
        if power.is_finite() {
            Some(power)
        } else {
            None
        }
    } else {
        None
    }
}

/// The deterministic design hash of the light-cone module's constants.
///
/// Folds in the speed of light (reused from [`crate::lighttime`], not duplicated) and the
/// inverse-square exponent, so a replay bundle or an Akashi entry can detect a stale light-cone
/// kernel. A pure function: the same kernel produces the same hash on every platform, every run.
pub fn lightcone_design_hash() -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut mix = |v: f64| {
        let bits = v.to_bits();
        hash ^= bits;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    };
    mix(SPEED_OF_LIGHT_KM_S);
    mix(INVERSE_SQUARE_EXPONENT);
    hash
}
