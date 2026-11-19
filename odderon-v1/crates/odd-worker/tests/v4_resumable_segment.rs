//! `oracle_true_single_segment_resumption_cost`, authority side.
//!
//! `odd-systems/tests/v4_resumption.rs` proves the *physics* is resumable: a
//! leg propagated from a checkpoint reproduces the single-pass flight and
//! costs a fraction of it. This test proves the *authority* can accept such a
//! segment, and that it refuses everything that is not one.
//!
//! The rules being pinned:
//!
//! * a resumable checkpoint verifies **without** any replay of `[0, t_us)`;
//!   the closure it is handed is asked only for its own window, and the test
//!   records exactly which window that was;
//! * the chain binds: a segment must name the link it follows, so a segment
//!   cannot be re-used in a different chain;
//! * a checkpoint whose tank columns disagree with its own propellant mass is
//!   rejected before any physics runs — two truths about one vehicle;
//! * the 32 KB body cap and the per-key fixed-window budget both fail closed.

use odd_worker::protocol::SegmentCheckpoint;
use odd_worker::segment::{
    validate_resumable, verify_resumable_segment, SegmentRateLimiter, MAX_SEGMENT_BODY_BYTES,
    RATE_LIMIT_REQUESTS, RATE_LIMIT_WINDOW_S,
};
use odd_worker::ServerError;

const DRY: f64 = 5_182.1;

/// A checkpoint that claims to be resumable, with a self-consistent state.
fn checkpoint(t_us: i64, liquid_kg: f64) -> SegmentCheckpoint {
    let mut cp = SegmentCheckpoint::legacy(
        t_us,
        [7.0e6, 0.0, 0.0, 0.0, 7_600.0, 0.0, DRY + liquid_kg],
        liquid_kg,
        [0.0, 0.0],
        [1u8; 32],
    );
    cp.tank_states = Some([[20.0, liquid_kg * 0.84, 0.0], [20.0, liquid_kg * 0.16, 0.0]]);
    cp
}

/// The terminal state a segment claims to have reached, hashed the way the
/// recorder hashes a final state.
fn final_hash(t_s: f64, mass: f64) -> [u8; 32] {
    odd_report::fdr::final_state_hash(t_s, &[7.1e6, 0.0, 0.0, 0.0, 7_700.0, 0.0], mass)
}

#[test]
fn oracle_true_single_segment_resumption_cost() {
    let cp = checkpoint(3 * 86_400 * 1_000_000, 6_400.0);
    assert!(cp.is_resumable());
    validate_resumable(&cp).expect("a self-consistent checkpoint is resumable");

    // --- 1. The authority is asked for the segment only. ---
    let end_us = 4 * 86_400 * 1_000_000;
    let want = final_hash(end_us as f64 / 1e6, DRY + 6_100.0);
    let mut asked: Vec<(f64, f64)> = Vec::new();
    verify_resumable_segment(&cp, None, end_us, &want, |_cp, start_s, end_s| {
        asked.push((start_s, end_s));
        Some(odd_astro::spline::Keyframe {
            time: end_s,
            position: [7.1e6, 0.0, 0.0],
            velocity: [0.0, 7_700.0, 0.0],
            mass: DRY + 6_100.0,
        })
    })
    .expect("a segment that reproduces the signed final state must verify");
    assert_eq!(
        asked.len(),
        1,
        "exactly one propagation, not one per segment"
    );
    // The window starts at the checkpoint, not at the pad. This is the whole
    // claim: `[0, t_us)` was never requested.
    assert_eq!(asked[0].0, cp.t_us as f64 / 1e6);
    assert_eq!(asked[0].1, end_us as f64 / 1e6);

    // --- 2. The chain binds. ---
    let mut linked = cp.clone();
    linked.prev_seg_hash = Some([9u8; 32]);
    verify_resumable_segment(&linked, Some(&[9u8; 32]), end_us, &want, |_c, _s, e| {
        Some(odd_astro::spline::Keyframe {
            time: e,
            position: [7.1e6, 0.0, 0.0],
            velocity: [0.0, 7_700.0, 0.0],
            mass: DRY + 6_100.0,
        })
    })
    .expect("a segment naming its predecessor verifies");
    // Re-used in a different chain, the same segment is refused.
    assert_eq!(
        verify_resumable_segment(&linked, None, end_us, &want, |_c, _s, _e| None),
        Err(ServerError::SegmentMismatch(0)),
        "a segment must not be replayable under a different chain"
    );
    assert_eq!(
        verify_resumable_segment(&cp, Some(&[9u8; 32]), end_us, &want, |_c, _s, _e| None),
        Err(ServerError::SegmentMismatch(0)),
        "a segment that omits its predecessor must be refused"
    );

    // --- 3. Lying about the vehicle is caught before any physics. ---
    let mut inconsistent = cp.clone();
    inconsistent.tank_states = Some([[20.0, 1_000.0, 0.0], [20.0, 1_000.0, 0.0]]);
    assert_eq!(
        validate_resumable(&inconsistent),
        Err(ServerError::SegmentMismatch(0)),
        "tank columns must add up to the propellant the checkpoint claims"
    );
    let mut negative = cp.clone();
    negative.tank_states = Some([[20.0, 6_400.0, -1.0], [20.0, 0.0, 0.0]]);
    assert_eq!(
        validate_resumable(&negative),
        Err(ServerError::SegmentMismatch(0)),
        "a negative vent total is not a state"
    );
    let mut cold = cp.clone();
    cold.tank_states = Some([[0.0, 6_400.0, 0.0], [20.0, 0.0, 0.0]]);
    assert_eq!(
        validate_resumable(&cold),
        Err(ServerError::SegmentMismatch(0)),
        "absolute zero is not a tank temperature"
    );
    let mut v1 = cp.clone();
    v1.tank_states = None;
    assert_eq!(
        validate_resumable(&v1),
        Err(ServerError::SegmentNotResumable),
        "a chain-only v3 checkpoint cannot be resumed"
    );

    // A segment that propagates somewhere else is rejected at the terminal hash.
    assert_eq!(
        verify_resumable_segment(&cp, None, end_us, &want, |_c, _s, e| {
            Some(odd_astro::spline::Keyframe {
                time: e,
                position: [7.1e6, 0.0, 0.0],
                velocity: [0.0, 7_700.0, 0.0],
                mass: DRY + 9_000.0, // not the claimed vehicle
            })
        }),
        Err(ServerError::SegmentMismatch(1))
    );
    // An empty window is not a segment.
    assert_eq!(
        verify_resumable_segment(&cp, None, cp.t_us, &want, |_c, _s, _e| None),
        Err(ServerError::SegmentMismatch(0))
    );

    // --- 4. The envelope fails closed. ---
    // 32 KiB: a full v4 checkpoint is a few hundred bytes, so the cap is
    // generous for real segments and strict about anything else.
    assert_eq!(MAX_SEGMENT_BODY_BYTES, 32 * 1024);
    let wire = serde_shape(&cp);
    assert!(
        wire < MAX_SEGMENT_BODY_BYTES / 8,
        "a resumable checkpoint is {wire} bytes; the cap leaves room for 8 per segment"
    );

    // Fixed-window budget, per key.
    let mut limiter = SegmentRateLimiter::default();
    let key = [3u8; 32];
    for _ in 0..RATE_LIMIT_REQUESTS {
        limiter.charge(key, 1_000).expect("inside the budget");
    }
    assert_eq!(
        limiter.charge(key, 1_000),
        Err(ServerError::RateLimited),
        "the budget must be spent, not elastic"
    );
    limiter
        .charge(key, 1_000 + RATE_LIMIT_WINDOW_S)
        .expect("the next window starts fresh");
    limiter
        .charge([4u8; 32], 1_000)
        .expect("one key's budget must not spend another's");
}

/// Wire size of a checkpoint in the documented JSON form, without a serde
/// dependency: the same field shapes `checkpoint_to_json` emits.
fn serde_shape(cp: &SegmentCheckpoint) -> usize {
    let mut bytes = 0;
    bytes += format!("{{\"t_us\":{},\"state_vector\":[", cp.t_us).len();
    bytes += cp
        .state_vector
        .iter()
        .map(|v| format!("{v},").len())
        .sum::<usize>();
    bytes += format!("],\"prop_kg\":{},\"tank_temps\":[", cp.prop_kg).len();
    bytes += 64;
    bytes += "],\"cum_hash\":\"\",\"tank_states\":[[0,0,0],[0,0,0]],\"cooler_joules\":0.0,\"vent_count\":0}".len();
    bytes
}
