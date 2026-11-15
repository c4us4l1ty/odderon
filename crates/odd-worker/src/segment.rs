//! Chunked hash-chain verification (blueprint §6.4).
//!
//! A long mission can be verified in pieces: the FDR carries a checkpoint
//! every ~30 sim-days, each chaining a cumulative hash over every prior
//! checkpoint. The authority recomputes the chain in O(1) per segment and
//! binds the terminal checkpoint to the same `final_state_hash` the full
//! replay produces. A one-bit edit to an intermediate checkpoint diverges
//! the chain at that segment's own boundary, so the rejection names where
//! the tamper happened rather than only failing at the end.
//!
//! The cloud adapter may additionally replay each segment's physics inside
//! its per-request CPU budget; the chain is what guarantees those pieces
//! compose to the identical terminal state.

use crate::protocol::{put_f64, put_i64, Hash, SegmentCheckpoint, ServerError};
use sha2::{Digest, Sha256};

/// Domain separator for the first link of a segment chain.
pub const SEGMENT_DOMAIN: &[u8] = b"odderon-segment-chain-v1";

/// The canonical bytes one checkpoint contributes to the chain.
pub fn checkpoint_bytes(cp: &SegmentCheckpoint) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + 7 * 8 + 8 + 2 * 8);
    put_i64(&mut out, cp.t_us);
    for v in &cp.state_vector {
        put_f64(&mut out, *v);
    }
    put_f64(&mut out, cp.prop_kg);
    for t in &cp.tank_temps {
        put_f64(&mut out, *t);
    }
    out
}

/// Recompute the cumulative hash up to and including checkpoint `index`,
/// given the previous link (`None` for the first).
pub fn link_hash(prev: Option<&Hash>, cp: &SegmentCheckpoint) -> Hash {
    let mut hasher = Sha256::new();
    match prev {
        None => hasher.update(SEGMENT_DOMAIN),
        Some(h) => hasher.update(h),
    }
    hasher.update(checkpoint_bytes(cp));
    hasher.finalize().into()
}

/// Verify a presented chain against the authoritative flight.
///
/// `sample` returns the replayed trajectory's state at a mission time. Each
/// checkpoint must (a) carry the correct cumulative hash link and (b) match
/// the authoritative state at its own `t_us` bit-for-bit. The terminal
/// checkpoint must additionally reproduce `final_state_hash`. Returns
/// `SegmentMismatch(i)` at the first checkpoint that fails either check, so
/// a one-bit edit — even one whose downstream hashes were recomputed — is
/// caught at its own boundary by the physics sample. An empty chain is
/// rejected: a chunked submission must carry at least its terminal point.
pub fn verify_chain<F>(
    checkpoints: &[SegmentCheckpoint],
    final_state_hash: &Hash,
    sample: F,
) -> Result<(), ServerError>
where
    F: Fn(f64) -> Option<odd_astro::spline::Keyframe>,
{
    if checkpoints.is_empty() {
        return Err(ServerError::SegmentMismatch(0));
    }
    let mut prev: Option<Hash> = None;
    for (i, cp) in checkpoints.iter().enumerate() {
        let expected = link_hash(prev.as_ref(), cp);
        if expected != cp.cum_hash {
            return Err(ServerError::SegmentMismatch(i));
        }
        prev = Some(expected);
        // The checkpoint must be the state the authority actually flew at
        // this instant, not a plausible-looking fabrication.
        let t = cp.t_us as f64 / 1e6;
        let Some(state) = sample(t) else {
            return Err(ServerError::SegmentMismatch(i));
        };
        let authoritative = [
            state.position[0],
            state.position[1],
            state.position[2],
            state.velocity[0],
            state.velocity[1],
            state.velocity[2],
            state.mass,
        ];
        for (claimed, actual) in cp.state_vector.iter().zip(authoritative.iter()) {
            if claimed.to_bits() != actual.to_bits() {
                return Err(ServerError::SegmentMismatch(i));
            }
        }
    }
    // The terminal checkpoint must be the flown final state, so the chain
    // composes to exactly the hash the full replay produced.
    let last = checkpoints.last().expect("non-empty");
    let state6 = [
        last.state_vector[0],
        last.state_vector[1],
        last.state_vector[2],
        last.state_vector[3],
        last.state_vector[4],
        last.state_vector[5],
    ];
    let terminal =
        odd_report::fdr::final_state_hash(last.t_us as f64 / 1e6, &state6, last.state_vector[6]);
    if &terminal != final_state_hash {
        return Err(ServerError::SegmentMismatch(checkpoints.len() - 1));
    }
    Ok(())
}

/// Build a honest checkpoint chain from a flown trajectory's sampled states.
/// The recorder uses this to attach segments; the test oracle uses it to
/// produce a chain that must verify, then tampers one link.
pub fn build_chain(samples: &[(i64, [f64; 7], f64, [f64; 2])]) -> Vec<SegmentCheckpoint> {
    let mut prev: Option<Hash> = None;
    let mut out = Vec::with_capacity(samples.len());
    for (t_us, state_vector, prop_kg, tank_temps) in samples {
        let mut cp = SegmentCheckpoint {
            t_us: *t_us,
            state_vector: *state_vector,
            prop_kg: *prop_kg,
            tank_temps: *tank_temps,
            cum_hash: [0u8; 32],
        };
        cp.cum_hash = link_hash(prev.as_ref(), &cp);
        prev = Some(cp.cum_hash);
        out.push(cp);
    }
    out
}
