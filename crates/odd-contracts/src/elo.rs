//! Canonical Astro-Elo mechanics (blueprint §6.6).
//!
//! This is the single authoritative home of the rating math the server
//! applies. The browser keeps a local copy for practice feedback
//! (`odd_app::debrief::elo_update` and `web/identity.js`); a parity gate in
//! `tests/elo_parity.rs` pins this function to the client's bit-for-bit so the
//! two can never drift. Only the server's result is certified.
//!
//! * Expected score `E = 1 / (1 + 10^((R_m − R_p)/400))`.
//! * Player update `R_p' = R_p + K_p·(S − E)`, `K_p = 40` for the first 20
//!   ranked missions then `20`.
//! * Mission update `R_m' = R_m + K_m·(E − S)`, `K_m = 8` — missions drift
//!   slowly toward their true difficulty.

/// Every pilot starts here, and a mission with no measured rating falls back
/// to its contract's seeded difficulty rather than to this number.
pub const START_ELO: f64 = 1200.0;
/// Provisional K factor for a pilot's first ranked missions.
pub const K_PLAYER_PROVISIONAL: f64 = 40.0;
/// Established K factor once the provisional window closes.
pub const K_PLAYER_ESTABLISHED: f64 = 20.0;
/// Number of ranked missions that use the provisional K factor.
pub const PROVISIONAL_RANKED: u32 = 20;
/// Missions move an eighth as fast as pilots.
pub const K_MISSION: f64 = 8.0;

/// The K factor for a pilot who has already completed `ranked_missions`
/// ranked attempts. The count is the number *before* the attempt being
/// scored, so attempt 1..=20 use the provisional factor.
pub fn k_player(ranked_missions: u32) -> f64 {
    if ranked_missions < PROVISIONAL_RANKED {
        K_PLAYER_PROVISIONAL
    } else {
        K_PLAYER_ESTABLISHED
    }
}

/// Standard Elo update with explicit K factors. Returns `(player, mission)`.
/// Identical to `odd_app::debrief::elo_update`; the parity gate keeps them so.
pub fn elo_update(
    player: f64,
    mission: f64,
    score: f64,
    k_player: f64,
    k_mission: f64,
) -> (f64, f64) {
    let expected = 1.0 / (1.0 + libm::pow(10.0, (mission - player) / 400.0));
    (
        player + k_player * (score - expected),
        mission + k_mission * (expected - score),
    )
}

/// The expected-score term alone, exposed so a receipt audit can recompute
/// the update without re-deriving the logistic.
pub fn expected_score(player: f64, mission: f64) -> f64 {
    1.0 / (1.0 + libm::pow(10.0, (mission - player) / 400.0))
}

/// Apply one ranked result with the canonical K policy. `ranked_before` is
/// the pilot's ranked-mission count prior to this attempt. Practice attempts
/// must never call this — the engine only invokes it for `AttemptMode::Ranked`.
pub fn apply_ranked(player: f64, mission: f64, score: f64, ranked_before: u32) -> (f64, f64) {
    elo_update(player, mission, score, k_player(ranked_before), K_MISSION)
}
