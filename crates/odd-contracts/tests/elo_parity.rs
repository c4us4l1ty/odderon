//! The server's canonical Elo (odd_contracts::elo) must agree bit-for-bit
//! with the client's local copy (odd_app::debrief::elo_update). Only the
//! server's result is certified, but a drift between the two would make the
//! practice feedback lie about what the ranked update will do.

use odd_contracts::elo;

#[test]
fn canonical_elo_matches_the_client_bit_for_bit() {
    for player in [800.0, 1200.0, 1420.0, 1650.0, 2400.0] {
        for mission in [800.0, 1250.0, 1500.0, 1650.0] {
            for score in [0.0, 0.25, 0.5, 0.65, 0.8148623316575985, 1.0] {
                for (kp, km) in [(40.0, 8.0), (20.0, 8.0)] {
                    let server = elo::elo_update(player, mission, score, kp, km);
                    let client = odd_app::debrief::elo_update(player, mission, score, kp, km);
                    assert_eq!(server.0.to_bits(), client.0.to_bits());
                    assert_eq!(server.1.to_bits(), client.1.to_bits());
                }
            }
        }
    }
}

#[test]
fn the_k_policy_switches_after_twenty_ranked_missions() {
    assert_eq!(elo::k_player(0), 40.0);
    assert_eq!(elo::k_player(19), 40.0);
    assert_eq!(elo::k_player(20), 20.0);
    assert_eq!(elo::k_player(1000), 20.0);
    assert_eq!(elo::START_ELO, 1200.0);
    assert_eq!(elo::K_MISSION, 8.0);
}

#[test]
fn a_perfect_ranked_win_moves_the_pilot_up_and_the_mission_down() {
    let (p, m) = elo::apply_ranked(1200.0, 1650.0, 1.0, 0);
    assert!(p > 1200.0);
    assert!(m < 1650.0);
    // Zero-sum in K-weighted terms: the pilot gains Kp·(1−E), the mission
    // loses Km·(1−E), so the ratio of the moves is Kp/Km.
    let e = elo::expected_score(1200.0, 1650.0);
    assert!(((p - 1200.0) / (1650.0 - m) - 40.0 / 8.0).abs() < 1e-9);
    assert!((p - (1200.0 + 40.0 * (1.0 - e))).abs() < 1e-12);
}
