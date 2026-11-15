//! WP 1.7 acceptance gate: `oracle_delegation_efficiency_cap`.
//!
//! A fully manual stack may reach Seff = 1.0. A stack with three of four
//! disciplines delegated must be capped at 0.65, and the cap must propagate
//! through the weighted score into a **hand-checked** Elo update. Every
//! expected number below is computed in the test from the published formula,
//! not copied from the implementation's output.

use odd_contracts::chief_ai::{
    self, DelegationMask, Department, DisciplineMass, COST_MARGIN, MASS_MARGIN, SEFF_CAP,
};
use odd_contracts::{score, Contract, MissionScore};

/// Blueprint §6.6: Kp = 40 for the player, Km = 8 for the mission.
const K_PLAYER: f64 = 40.0;
const K_MISSION: f64 = 8.0;

/// Standard Elo, written out here independently of `odd-app::debrief`.
fn hand_elo(player: f64, mission: f64, s: f64) -> (f64, f64) {
    let expected = 1.0 / (1.0 + 10f64.powf((mission - player) / 400.0));
    (
        player + K_PLAYER * (s - expected),
        mission + K_MISSION * (expected - s),
    )
}

fn hardware() -> [DisciplineMass; 4] {
    [
        DisciplineMass {
            department: Department::Propulsion,
            base_kg: 2_372.0,
        },
        DisciplineMass {
            department: Department::Thermal,
            base_kg: 1_200.0,
        },
        DisciplineMass {
            department: Department::Power,
            base_kg: 300.0,
        },
        DisciplineMass {
            department: Department::Gnc,
            base_kg: 180.0,
        },
    ]
}

/// A perfect flight: objective met, prediction exact, reference delta-v.
fn perfect_score(mask: DelegationMask) -> MissionScore {
    let mut s = MissionScore {
        objectives: 1.0,
        prediction: 1.0,
        efficiency: 1.0,
        s: 1.0,
    };
    chief_ai::cap_score(&mut s, mask);
    s
}

#[test]
fn oracle_delegation_efficiency_cap() {
    // --- manual: uncapped -------------------------------------------------
    let manual = perfect_score(DelegationMask::MANUAL);
    assert_eq!(
        manual.efficiency, 1.0,
        "a manual stack must reach Seff = 1.0"
    );
    assert!((manual.s - 1.0).abs() < 1e-12);

    // --- three of four delegated: capped at 0.65 --------------------------
    let three = DelegationMask::MANUAL
        .with(Department::Propulsion, true)
        .with(Department::Thermal, true)
        .with(Department::Power, true);
    assert_eq!(three.count(), 3);
    assert!(!three.contains(Department::Gnc));
    let delegated = perfect_score(three);
    assert_eq!(delegated.efficiency, SEFF_CAP);
    assert!(delegated.efficiency <= 0.65);
    // S = 0.50·1 + 0.25·1 + 0.25·0.65 = 0.9125, by hand.
    let expected_s = 0.50 + 0.25 + 0.25 * 0.65;
    assert!((delegated.s - expected_s).abs() < 1e-12);
    assert!((delegated.s - 0.9125).abs() < 1e-12);
    println!(
        "manual S {:.6} vs 3-of-4 delegated S {:.6}",
        manual.s, delegated.s
    );

    // --- the cap must reach Elo, not just the component -------------------
    // Player 1420 against a 1500-rated mission.
    let (p_manual, m_manual) = hand_elo(1420.0, 1500.0, manual.s);
    let (p_deleg, m_deleg) = hand_elo(1420.0, 1500.0, delegated.s);
    let expected_win = 1.0 / (1.0 + 10f64.powf((1500.0 - 1420.0) / 400.0));
    // Hand arithmetic: E = 1/(1+10^0.2) = 0.3868631798468569.
    assert!((expected_win - 0.386_863_179_846_856_9).abs() < 1e-15);
    assert!((p_manual - (1420.0 + 40.0 * (1.0 - expected_win))).abs() < 1e-9);
    assert!((p_deleg - (1420.0 + 40.0 * (0.9125 - expected_win))).abs() < 1e-9);
    // The delegated flight gains strictly less rating for the same objective.
    assert!(p_deleg < p_manual);
    let difference = p_manual - p_deleg;
    // Exactly Kp · 0.25 · (1 − 0.65) = 40 · 0.0875 = 3.5 points.
    assert!((difference - 3.5).abs() < 1e-9, "gap {difference} != 3.5");
    // And the mission's rating moves the other way, by Km/Kp as much.
    assert!((m_manual - m_deleg + difference * K_MISSION / K_PLAYER).abs() < 1e-9);
    println!(
        "Elo: manual {p_manual:.6} ({:+.6}), delegated {p_deleg:.6} ({:+.6}), gap {difference:.6}",
        p_manual - 1420.0,
        p_deleg - 1420.0
    );

    // --- the production scorer agrees with the hand formula ---------------
    let (p_prod, m_prod) =
        odd_app::debrief::elo_update(1420.0, 1500.0, delegated.s, K_PLAYER, K_MISSION);
    assert!((p_prod - p_deleg).abs() < 1e-12);
    assert!((m_prod - m_deleg).abs() < 1e-12);
}

/// One delegated discipline is enough to trigger the cap: the ceiling is per
/// stack, not an average over disciplines.
#[test]
fn any_delegation_triggers_the_cap() {
    for department in Department::ALL {
        let mask = DelegationMask::MANUAL.with(department, true);
        let s = perfect_score(mask);
        assert_eq!(
            s.efficiency,
            SEFF_CAP,
            "{} alone must cap efficiency",
            department.label()
        );
        assert_eq!(mask.efficiency_cap(), SEFF_CAP);
    }
    assert_eq!(DelegationMask::MANUAL.efficiency_cap(), 1.0);
    assert_eq!(DelegationMask::ALL.efficiency_cap(), SEFF_CAP);
}

/// The margins are exactly +35 % mass and +30 % cost on the delegated
/// disciplines only, and the badge names the chief and the kilograms.
#[test]
fn margins_are_exact_and_visible() {
    let hw = hardware();
    let three = DelegationMask::MANUAL
        .with(Department::Propulsion, true)
        .with(Department::Thermal, true)
        .with(Department::Power, true);
    let costs = [100.0, 40.0, 30.0, 20.0];
    let out = chief_ai::apply(three, &hw, &costs);
    // 0.35 · (2372 + 1200 + 300) = 0.35 · 3872 = 1355.2 kg, by hand.
    let expected_mass = 0.35 * (2_372.0 + 1_200.0 + 300.0);
    assert!((out.mass_penalty_kg - expected_mass).abs() < 1e-9);
    assert!((out.mass_penalty_kg - 1_355.2).abs() < 1e-9);
    // Cost: (100+40+30)·1.30 + 20 = 221 + 20 = 241 over a 190 baseline.
    let expected_cost = (170.0 * 1.30 + 20.0) / 190.0;
    assert!((out.cost_multiplier - expected_cost).abs() < 1e-12);
    assert!((MASS_MARGIN - 0.35).abs() < 1e-15);
    assert!((COST_MARGIN - 0.30).abs() < 1e-15);
    // GNC stayed manual: no margin, and its badge says so.
    let gnc = out
        .decisions
        .iter()
        .find(|d| d.department == Department::Gnc)
        .unwrap();
    assert_eq!(gnc.margin_kg, 0.0);
    assert_eq!(gnc.cost_multiplier, 1.0);
    assert!(gnc.badge.contains("[MANUAL]"));
    let thermal = out
        .decisions
        .iter()
        .find(|d| d.department == Department::Thermal)
        .unwrap();
    assert!(thermal.badge.contains("AI Thermal Chief"));
    assert!(thermal.badge.contains("420 kg"), "{}", thermal.badge);
    println!("{}", thermal.badge);
}

/// The bitmask survives a round trip through the wire byte, and illegal bits
/// are rejected rather than silently ignored.
#[test]
fn the_mask_is_a_frozen_wire_byte() {
    for byte in 0u8..=255 {
        let mask = DelegationMask(byte);
        if byte <= 0b1111 {
            assert!(mask.is_valid());
            assert_eq!(mask.count(), byte.count_ones());
            for department in Department::ALL {
                assert_eq!(
                    mask.contains(department),
                    byte & (1 << (department as u8)) != 0
                );
            }
        } else {
            assert!(!mask.is_valid(), "byte {byte:#06b} must be rejected");
        }
    }
}

/// Delegation changes the score, never the physics: the same commanded
/// flight scores differently but its objective component is untouched.
#[test]
fn delegation_caps_efficiency_without_touching_objectives() {
    let contract = Contract::get(101).unwrap();
    // Fly the reference Hohmann exactly: a perfect circular rescue.
    let r = odd_contracts::EARTH_EQUATORIAL_RADIUS + 550_000.0;
    let state = odd_astro::spline::Keyframe {
        time: 5_400.0,
        position: [r, 0.0, 0.0],
        velocity: [0.0, libm::sqrt(odd_contracts::EARTH_MU / r), 0.0],
        mass: contract.dry_mass_kg + 50.0,
    };
    // A locked prediction that matches the flown result, so the score is a
    // real number rather than NaN and the cap has something to bite on.
    let prediction = odd_sim::mission::PredictionBaseline {
        delta_v_m_s: contract.reference_delta_v_m_s,
        apoapsis_altitude_m: 550_000.0,
        periapsis_altitude_m: 550_000.0,
        remaining_propellant_kg: 50.0,
        time_of_flight_s: 5_400.0,
        target_miss_distance_m: None,
    };
    let base = score(
        &contract,
        state,
        5_400.0,
        prediction,
        contract.reference_delta_v_m_s,
        false,
    )
    .unwrap();
    let mut capped = base;
    chief_ai::cap_score(&mut capped, DelegationMask::ALL);
    assert!(base.s.is_finite(), "the scenario must produce a real score");
    assert_eq!(
        base.objectives, capped.objectives,
        "the AI must not change what the flight achieved"
    );
    assert_eq!(base.prediction, capped.prediction);
    assert!(
        base.efficiency > SEFF_CAP,
        "the manual flight must beat the cap"
    );
    assert!(capped.efficiency <= SEFF_CAP);
    assert!(capped.s < base.s, "delegation must cost score");
    println!(
        "real rescue: manual S {:.6} -> delegated S {:.6}",
        base.s, capped.s
    );
}
