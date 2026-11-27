//! The protégé's teaching arc: the progression that turns a junior who replays your errors into
//! a colleague who can catch what you missed.
//!
//! ## Why this is the rung after the protégé
//!
//! The protégé (`GAME-DESIGN-BIBLE.md` §5.6) already exists: a junior who replays your last
//! failure, asks you to diagnose it, and records a self-explanation when you answer correctly. That
//! is the *first* rung — the novice who asks "why did this go wrong?". The teaching arc is the
//! *progression*: as the player answers correctly more often, the junior's questions deepen, and the
//! things they can notice become sharper. The mechanic is **learning by teaching** (§6, row 13):
//! preparing to teach improves the teacher, and the protégé's growing competence is the visible
//! sign of that improvement.
//!
//! This module owns the *logic* of the arc — which tier the protégé is in, how deep their next
//! question should be, and what they can notice — as pure functions of the reputation count and the
//! kernel's own readings. The page owns the *strings*: it maps the kernel's tier and depth codes to
//! the question text and the diagnosis options. The kernel never produces a sentence; it produces the
//! *decision* that determines which sentences are correct.
//!
//! ## The arc
//!
//! Four tiers, each a threshold on the reputation count (the number of correct briefings):
//!
//! | Tier | Name | Reputation | Question depth | Can notice |
//! | --- | --- | --- | --- | --- |
//! | 0 | novice | 0–1 | verdict (what kind of flight) | nothing |
//! | 1 | junior | 2–4 | parameter (which input was binding) | a mismatched record |
//! | 2 | analyst | 5–9 | margin (how far short, and of what) | a stale record |
//! | 3 | peer | 10+ | counterfactual (what would have changed it) | a tampered record |
//!
//! The thresholds are pinned and folded into [`protege_design_hash`], so a replay bundle can detect
//! a stale arc model. The question depth is a pure function of the tier and the kernel's own
//! readings for the error entry (the verdict and apogee), so a third party can re-derive which
//! depth the protégé should ask at.

use odderon_dmath::is_finite;

use crate::rocket::RocketVerdict;

/// The reputation threshold for tier 1 (junior): 2 or more correct briefings.
pub const TIER1_REPUTATION: u32 = 2;

/// The reputation threshold for tier 2 (analyst): 5 or more correct briefings.
pub const TIER2_REPUTATION: u32 = 5;

/// The reputation threshold for tier 3 (peer): 10 or more correct briefings.
pub const TIER3_REPUTATION: u32 = 10;

/// The protégé's tier, from the reputation count (correct briefings answered).
///
/// A pure function of the reputation count: 0 for novice (0–1), 1 for junior (2–4), 2 for analyst
/// (5–9), 3 for peer (10+). Returns 0 for a non-finite reputation (the page passes a count, so this
/// is a guard against a corrupted state, not a normal input).
pub fn protege_tier(reputation: u32) -> u32 {
    if reputation >= TIER3_REPUTATION {
        3
    } else if reputation >= TIER2_REPUTATION {
        2
    } else if reputation >= TIER1_REPUTATION {
        1
    } else {
        0
    }
}

/// The name of a tier, as a stable code the page maps to a label.
///
/// 0 = novice, 1 = junior, 2 = analyst, 3 = peer. Returns 0 for an out-of-range tier (a guard,
/// not a normal input).
pub fn protege_tier_name(tier: u32) -> u32 {
    match tier {
        0 => 0,
        1 => 1,
        2 => 2,
        3 => 3,
        _ => 0,
    }
}

/// The depth of the protégé's next question, from the tier and the kernel's readings for the error
/// entry.
///
/// The depth is a pure function of the tier and the error entry's verdict and apogee — the same
/// readings the protégé's basic diagnosis already branches on. At higher tiers the protégé asks
/// about more specific aspects of the failure:
///
/// * **0 — verdict** (tier 0): "what kind of flight was this?" The answer is the verdict.
/// * **1 — parameter** (tier 1): "which input was binding?" The answer is the parameter that, if
///   changed, would have flipped the verdict.
/// * **2 — margin** (tier 2): "how far short, and of what?" The answer is the margin to the
///   Kármán line or to orbital velocity, whichever the failure was short of.
/// * **3 — counterfactual** (tier 3): "what would have changed it?" The answer is the minimal
///   change to the launch parameters that would have succeeded.
///
/// The depth never exceeds the tier: a novice asks verdict questions, a peer asks counterfactuals.
/// A non-finite apogee (a refused launch with no apogee reading) caps the depth at 1, because the
/// margin and counterfactual questions need an apogee to be meaningful.
pub fn protege_question_depth(verdict: u32, apogee_km: f64, tier: u32) -> u32 {
    // The depth is the tier, capped by what the readings support. A refused launch (verdict 3)
    // has no apogee, so the margin and counterfactual questions cannot be asked — the depth is
    // capped at 1 (parameter: "which input was out of range?").
    if verdict == RocketVerdict::Refused as u32 || !is_finite(apogee_km) {
        return if tier >= 1 { 1 } else { 0 };
    }
    // The depth is the tier, capped at 3.
    if tier >= 3 {
        3
    } else {
        tier
    }
}

/// What the protégé can notice at a given tier, as a stable code the page maps to a label.
///
/// 0 = nothing (the protégé has nothing to flag), 1 = a mismatched record (the protégé notices a
/// record that does not verify against the kernel), 2 = a stale record (the protégé notices a
/// record whose design hash does not match the current kernel), 3 = a tampered record (the
/// protégé notices a record whose hash does not match its content).
///
/// The notice level never exceeds the tier: a novice notices nothing, a peer notices tampering.
pub fn protege_notice_level(tier: u32) -> u32 {
    match tier {
        0 => 0,
        1 => 1,
        2 => 2,
        3 => 3,
        _ => 0,
    }
}

/// The deterministic design hash of the protégé arc's constants.
///
/// Folds in the three tier thresholds, so a replay bundle or an Akashi entry can detect a stale
/// arc model. A pure function: the same kernel produces the same hash on every platform, every run.
pub fn protege_design_hash() -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut mix = |v: u32| {
        let bits = (v as f64).to_bits();
        hash ^= bits;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    };
    mix(TIER1_REPUTATION);
    mix(TIER2_REPUTATION);
    mix(TIER3_REPUTATION);
    hash
}
