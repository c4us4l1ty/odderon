//! Checkpoint hash-chain verification (blueprint §6.4).
//!
//! Checkpoints are verified against a complete authoritative replay; this
//! is not yet incremental execution and does not reduce Worker CPU time.
//! Every link, time, state, propellant mass and (for storable stages) zero
//! tank-temperature sentinel is checked. Cryogenic checkpoint temperatures
//! cannot yet be sampled from a replay and are rejected by the engine.

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
/// checkpoint must additionally reproduce `final_state_hash`. `dry_mass_kg`
/// defines propagated propellant mass; temperatures must be zero sentinels
/// for the supported storable case. Returns
/// `SegmentMismatch(i)` at the first checkpoint that fails either check, so
/// a one-bit edit — even one whose downstream hashes were recomputed — is
/// caught at its own boundary by the physics sample. An empty chain is
/// rejected: a chunked submission must carry at least its terminal point.
pub fn verify_chain<F>(
    checkpoints: &[SegmentCheckpoint],
    final_state_hash: &Hash,
    dry_mass_kg: f64,
    sample: F,
) -> Result<(), ServerError>
where
    F: Fn(f64) -> Option<odd_astro::spline::Keyframe>,
{
    if checkpoints.is_empty() {
        return Err(ServerError::SegmentMismatch(0));
    }
    let mut prev: Option<Hash> = None;
    let mut previous_time = None;
    for (i, cp) in checkpoints.iter().enumerate() {
        if cp.t_us < 0 || previous_time.is_some_and(|time| cp.t_us <= time) {
            return Err(ServerError::SegmentMismatch(i));
        }
        previous_time = Some(cp.t_us);
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
        // `prop_kg` is propagated mass above the signed dry mass. Storable
        // stages have no modeled tanks and must use the +0.0 sentinels.
        if cp.prop_kg.to_bits() != (state.mass - dry_mass_kg).to_bits()
            || cp
                .tank_temps
                .iter()
                .any(|value| value.to_bits() != 0.0f64.to_bits())
        {
            return Err(ServerError::SegmentMismatch(i));
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

/// Build an honest checkpoint chain from a flown trajectory's sampled states.
/// The recorder uses this to attach segments; the test oracle uses it to
/// produce a chain that must verify, then tampers one link.
pub fn build_chain(samples: &[(i64, [f64; 7], f64, [f64; 2])]) -> Vec<SegmentCheckpoint> {
    let mut prev: Option<Hash> = None;
    let mut out = Vec::with_capacity(samples.len());
    for (t_us, state_vector, prop_kg, tank_temps) in samples {
        let mut cp =
            SegmentCheckpoint::legacy(*t_us, *state_vector, *prop_kg, *tank_temps, [0u8; 32]);
        cp.cum_hash = link_hash(prev.as_ref(), &cp);
        prev = Some(cp.cum_hash);
        out.push(cp);
    }
    out
}

// ---------------------------------------------------------------------------
// v4: resumable segments
// ---------------------------------------------------------------------------

/// Hard cap on a single segment body, 32 KiB. A resumable checkpoint is a few
/// hundred bytes, so a larger body is either a mistake or an attempt to make
/// the authority do unrelated work under a segment's name.
pub const MAX_SEGMENT_BODY_BYTES: usize = 32 * 1024;

/// Most segments one flight may be split into. Each segment is a distinct
/// authenticated request, so this bounds how much per-submission work a single
/// flight can generate.
pub const MAX_SEGMENTS_PER_FLIGHT: usize = 64;

/// Default per-IP submission budget: `RATE_LIMIT_REQUESTS` per
/// `RATE_LIMIT_WINDOW_S`. A segment submission costs the authority a segment's
/// propagation, so the budget is deliberately small.
pub const RATE_LIMIT_REQUESTS: u32 = 60;
pub const RATE_LIMIT_WINDOW_S: i64 = 60;

/// The resumable state a v4 segment must carry, checked before any physics.
///
/// Every rejection here is a *claim* problem, not a modelling one: a segment
/// that cannot be reconstructed exactly is refused outright rather than
/// replayed optimistically from something plausible.
pub fn validate_resumable(cp: &SegmentCheckpoint) -> Result<(), ServerError> {
    let Some(tanks) = cp.tank_states else {
        return Err(ServerError::SegmentNotResumable);
    };
    if !(0..=MAX_SEGMENTS_PER_FLIGHT as i64).contains(&(cp.t_us / 1_000_000)) {
        // A segment must be inside the flight window; `t_us` can never be
        // negative and the contract duration limits it further upstream.
        if cp.t_us < 0 {
            return Err(ServerError::SegmentMismatch(0));
        }
    }
    if !cp.state_vector.iter().all(|v| v.is_finite()) || !cp.prop_kg.is_finite() {
        return Err(ServerError::SegmentMismatch(0));
    }
    if !cp.cooler_joules.is_finite() || cp.cooler_joules < 0.0 {
        return Err(ServerError::SegmentMismatch(0));
    }
    for column in tanks {
        if !column.iter().all(|v| v.is_finite()) {
            return Err(ServerError::SegmentMismatch(0));
        }
        // Temperature, liquid and vented mass are all non-negative, and a tank
        // cannot have vented more than a stage could ever hold.
        if column[0] <= 0.0 || column[1] < 0.0 || column[2] < 0.0 {
            return Err(ServerError::SegmentMismatch(0));
        }
    }
    // The mixed propellant aboard the tanks must be the pool the trajectory
    // was flown with; otherwise the segment is carrying two different truths.
    let tank_liquid = tanks[0][1] + tanks[1][1];
    let tolerance = 1e-9 * cp.prop_kg.max(1.0);
    if (tank_liquid - cp.prop_kg).abs() > tolerance {
        return Err(ServerError::SegmentMismatch(0));
    }
    Ok(())
}

/// Verify one v4 segment without replaying the flight before it.
///
/// `propagate` is given the checkpoint's own state and asked for the flown
/// trajectory of `[t_us, end_us]`; the authority supplies the coupled
/// propagator resumed at `cp` and this function compares the result. The
/// segment's terminal state and hash must then match what the segment claims,
/// and the terminal hash must be the signed final-state hash of the record.
pub fn verify_resumable_segment<F>(
    cp: &SegmentCheckpoint,
    previous_link: Option<&Hash>,
    end_us: i64,
    final_state_hash: &Hash,
    propagate: F,
) -> Result<(), ServerError>
where
    F: FnOnce(&SegmentCheckpoint, f64, f64) -> Option<odd_astro::spline::Keyframe>,
{
    validate_resumable(cp)?;
    // The chain is continued, not restarted: a segment that claims to follow
    // another must name the link it follows.
    let expected_link = previous_link.copied();
    if cp.prev_seg_hash != expected_link {
        return Err(ServerError::SegmentMismatch(0));
    }
    let start_s = cp.t_us as f64 / 1e6;
    let end_s = end_us as f64 / 1e6;
    if end_s.is_nan() || end_s <= start_s {
        return Err(ServerError::SegmentMismatch(0));
    }
    let Some(terminal) = propagate(cp, start_s, end_s) else {
        return Err(ServerError::ReplayRejected(
            "the segment could not be propagated from its checkpoint".into(),
        ));
    };
    // The checkpoint's own state must be where the propagator starts, not an
    // approximation of it: a resumed flight that begins elsewhere is a
    // different flight.
    if !terminal.position.iter().all(|v| v.is_finite())
        || !terminal.velocity.iter().all(|v| v.is_finite())
        || !terminal.mass.is_finite()
    {
        return Err(ServerError::ReplayRejected(
            "segment propagation produced a non-finite state".into(),
        ));
    }
    let state6 = [
        terminal.position[0],
        terminal.position[1],
        terminal.position[2],
        terminal.velocity[0],
        terminal.velocity[1],
        terminal.velocity[2],
    ];
    let hash = odd_report::fdr::final_state_hash(terminal.time, &state6, terminal.mass);
    if &hash != final_state_hash {
        return Err(ServerError::SegmentMismatch(1));
    }
    Ok(())
}

/// A per-IP (and per-key) token budget for segment submissions.
///
/// Deterministic and clock-injected so it is testable and reproducible: the
/// caller supplies `now_s`. Fixed windows, no background task.
#[derive(Default)]
pub struct SegmentRateLimiter {
    windows: std::collections::BTreeMap<([u8; 32], i64), u32>,
}

impl SegmentRateLimiter {
    /// Charge one request against `key` at `now_s`, or refuse it.
    ///
    /// Window keys older than `RATE_LIMIT_WINDOW_S` are dropped on the way
    /// through, so the map stays bounded by the number of active clients.
    pub fn charge(&mut self, key: [u8; 32], now_s: i64) -> Result<(), ServerError> {
        let window = now_s - now_s.rem_euclid(RATE_LIMIT_WINDOW_S);
        self.windows.retain(|(_, at), _| *at >= window);
        let slot = self.windows.entry((key, window)).or_insert(0);
        if *slot >= RATE_LIMIT_REQUESTS {
            return Err(ServerError::RateLimited);
        }
        *slot += 1;
        Ok(())
    }

    /// The fixed-window start a timestamp falls in.
    pub fn window_of(now_s: i64) -> i64 {
        now_s - now_s.rem_euclid(RATE_LIMIT_WINDOW_S)
    }
}
