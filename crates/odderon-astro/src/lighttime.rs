//! Light-time and the comms link: *how long does a signal take to cross the distance between two
//! positions, and when does it arrive?*
//!
//! ## Why this is the rung after pass prediction
//!
//! [`crate::observe::look_at`] answers "is it visible **now**" and [`crate::passes::next_pass`] answers
//! "**when** will it be over my station". Both are about the *path of light arriving at the observer*.
//! Light-time is the other half of the same question: a signal does not arrive when it is sent, it
//! arrives after it has crossed the distance at the speed of light — and for an orbital body that
//! distance is hundreds of kilometres, so the delay is real and measurable, not a rounding error.
//!
//! This is the block that makes the game's **causal mail** claim testable (`GAME-DESIGN-BIBLE.md`
//! §11.2): messages arrive with real light-time delay because the kernel is deterministic and the
//! universe is a function of time. The primitive here is the part that has to be *right*: a light-time
//! that is computed locally on the page, or by a second copy of the constant, is the same defect this
//! project has found in the room, the orbit and the pass — a number the panel reports that the kernel
//! did not produce.
//!
//! ## The one constant, and why it is *defined* rather than measured
//!
//! The speed of light in vacuum is **exactly** 299 792 458 m/s, because the metre is defined as the
//! distance light travels in 1/299 792 458 of a second. There is no measurement uncertainty to carry:
//! [`SPEED_OF_LIGHT_KM_S`] is a *defined* value, not a fitted one, and a second copy of it anywhere in
//! the tree would be the defect this module exists to remove.
//!
//! ## The failure mode, and why it refuses rather than answers
//!
//! A light-time is a distance divided by a constant. The ways it can be wrong are not subtle: a caller
//! that passes a NaN position, or two coincident positions (distance zero, no signal ever arrives),
//! produces a NaN or an infinite delay. NaN propagates silently through a panel and reads as "no
//! answer"; a fabricated delay reads as an answer. So [`light_time_s`] **refuses** a distance that is
//! not strictly positive and finite — returning `None` — rather than producing a number the caller
//! cannot tell apart from a real one. The same discipline [`crate::passes`] uses: a guard that answers
//! "no" is a different claim from a guard that answers "nothing".

/// The speed of light in vacuum, km/s.
///
/// This is a **defined** value, not a measured one: the metre is defined as the distance light travels
/// in vacuum in 1/299 792 458 s, so the speed of light is exact by construction and carries no
/// uncertainty. It is the only copy in the tree; a second constant elsewhere is the defect
/// [`light_time_s`] exists to prevent.
pub const SPEED_OF_LIGHT_KM_S: f64 = 299_792.458;

/// One-way light time between two positions in the **same inertial frame**, in seconds.
///
/// The light time is the Euclidean distance divided by [`SPEED_OF_LIGHT_KM_S`]. Both positions must be
/// in the same frame — this function does not rotate either vector, because a rotation applied to one
/// side and not the other is the classic way a comms delay becomes a plausible wrong number.
///
/// Returns `None` when the distance is not strictly positive and finite: a NaN or infinite input, or
/// two coincident positions (no signal can travel a zero distance). Refusing beats a NaN delay that
/// a panel would print as a number.
#[inline]
pub fn light_time_s(from: crate::Vec3, to: crate::Vec3) -> Option<f64> {
    let d = (to - from).norm();
    if crate::is_positive_finite(d) {
        Some(d / SPEED_OF_LIGHT_KM_S)
    } else {
        None
    }
}

/// Round-trip light time: a signal sent from `from` to `to` and the reply back, in seconds.
///
/// This is the quantity a real comms link is sized against — the latency a controller actually feels,
/// not the one-way figure — and it is exactly twice [`light_time_s`]. Returns `None` for the same
/// reasons [`light_time_s`] does.
#[inline]
pub fn round_trip_s(from: crate::Vec3, to: crate::Vec3) -> Option<f64> {
    light_time_s(from, to).map(|t| 2.0 * t)
}

/// The arrival time of a message sent at `send_s` from `from` to `to`, in seconds on the same clock.
///
/// The arrival is `send_s + light_time_s(from, to)`. This is the function the light-cone ledger is
/// built from: a message sent at a known time over a known distance arrives at a time the kernel
/// predicts, and because both the send time and the light time are pure functions of the inputs, the
/// same send produces the same arrival on every platform — the P3 replay claim.
///
/// Returns `None` when the light time cannot be computed (see [`light_time_s`]). A send time that is
/// itself not finite is refused by the same guard, because `send_s + finite` would still be a number
/// that looks like an arrival.
#[inline]
pub fn arrival_s(send_s: f64, from: crate::Vec3, to: crate::Vec3) -> Option<f64> {
    light_time_s(from, to).and_then(|t| {
        let a = send_s + t;
        if a.is_finite() {
            Some(a)
        } else {
            None
        }
    })
}
