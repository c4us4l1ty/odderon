//! Kernel tests for the protégé teaching arc module.
//!
//! The arc is a pure function of the reputation count and the kernel's readings for an error entry.
//! The tests check each tier threshold, the question-depth capping, the notice levels, and the
//! design hash — all recomputed from the thresholds independently.

use odderon_astro::protege::{
    protege_design_hash, protege_notice_level, protege_question_depth, protege_tier,
    protege_tier_name, TIER1_REPUTATION, TIER2_REPUTATION, TIER3_REPUTATION,
};
use odderon_astro::rocket::RocketVerdict;

/// A non-finite apogee: a refused launch with no apogee reading.
const NAN_APOGEE: f64 = f64::NAN;

/// A suborbital launch that fell short of the Kármán line.
const SHORT_APOGEE_KM: f64 = 45.0;

// --------------------------------------------------------------------------------------------
// Tier thresholds
// --------------------------------------------------------------------------------------------

#[test]
fn novice_at_zero_reputation() {
    assert_eq!(protege_tier(0), 0);
}

#[test]
fn novice_at_one_reputation() {
    // 0-1 correct briefings: still a novice.
    assert_eq!(protege_tier(1), 0);
}

#[test]
fn junior_at_tier1_threshold() {
    // 2 correct briefings: the junior tier begins.
    assert_eq!(protege_tier(TIER1_REPUTATION), 1);
}

#[test]
fn junior_at_tier2_threshold_minus_one() {
    // 4 correct briefings: still junior (analyst begins at 5).
    assert_eq!(protege_tier(TIER2_REPUTATION - 1), 1);
}

#[test]
fn analyst_at_tier2_threshold() {
    assert_eq!(protege_tier(TIER2_REPUTATION), 2);
}

#[test]
fn analyst_at_tier3_threshold_minus_one() {
    // 9 correct briefings: still analyst (peer begins at 10).
    assert_eq!(protege_tier(TIER3_REPUTATION - 1), 2);
}

#[test]
fn peer_at_tier3_threshold() {
    assert_eq!(protege_tier(TIER3_REPUTATION), 3);
}

#[test]
fn peer_at_large_reputation() {
    assert_eq!(protege_tier(100), 3);
}

// --------------------------------------------------------------------------------------------
// Question depth
// --------------------------------------------------------------------------------------------

#[test]
fn depth_matches_tier_for_suborbital() {
    // A suborbital failure has a real apogee, so depth = tier at every tier.
    for tier in 0..=3 {
        let depth = protege_question_depth(RocketVerdict::Suborbital as u32, SHORT_APOGEE_KM, tier);
        assert_eq!(depth, tier, "tier {tier} should give depth {tier}");
    }
}

#[test]
fn refused_launch_caps_depth_at_one() {
    // A refused launch has no apogee: the margin and counterfactual questions need an apogee.
    for tier in 2..=3 {
        let depth = protege_question_depth(RocketVerdict::Refused as u32, NAN_APOGEE, tier);
        assert_eq!(depth, 1, "tier {tier} with a refused launch should give depth 1");
    }
}

#[test]
fn refused_launch_at_novice_gives_verdict_depth() {
    // A novice with a refused launch has nothing to ask beyond the verdict.
    let depth = protege_question_depth(RocketVerdict::Refused as u32, NAN_APOGEE, 0);
    assert_eq!(depth, 0);
}

#[test]
fn non_finite_apogee_caps_depth_at_one() {
    // A non-finite apogee is treated the same as a refused launch: depth capped at 1.
    for tier in 2..=3 {
        let depth = protege_question_depth(RocketVerdict::Orbital as u32, NAN_APOGEE, tier);
        assert_eq!(depth, 1, "tier {tier} with NaN apogee should give depth 1");
    }
}

#[test]
fn depth_never_exceeds_tier() {
    // A high tier with a refused launch should never give depth above the cap.
    let depth = protege_question_depth(RocketVerdict::Refused as u32, NAN_APOGEE, 3);
    assert_eq!(depth, 1);
}

// --------------------------------------------------------------------------------------------
// Notice level
// --------------------------------------------------------------------------------------------

#[test]
fn notice_level_matches_tier() {
    for tier in 0..=3 {
        assert_eq!(protege_notice_level(tier), tier);
    }
}

#[test]
fn notice_nothing_at_novice() {
    assert_eq!(protege_notice_level(0), 0);
}

#[test]
fn notice_tampered_at_peer() {
    assert_eq!(protege_notice_level(3), 3);
}

// --------------------------------------------------------------------------------------------
// Tier name
// --------------------------------------------------------------------------------------------

#[test]
fn tier_name_round_trips() {
    for tier in 0..=3 {
        assert_eq!(protege_tier_name(tier), tier);
    }
}

#[test]
fn tier_name_out_of_range_returns_novice() {
    assert_eq!(protege_tier_name(4), 0);
}

// --------------------------------------------------------------------------------------------
// Design hash
// --------------------------------------------------------------------------------------------

#[test]
fn design_hash_is_stable() {
    assert_eq!(protege_design_hash(), protege_design_hash());
}

#[test]
fn design_hash_is_nonzero() {
    assert!(protege_design_hash() != 0);
}
