//! Local score rules are explicit game rules, not persisted Elo updates.
use odd_contracts::{score, Contract};
use odd_sim::mission::PredictionBaseline;
#[test]
fn impact_and_fuel_failures_score_zero() {
    let c = Contract::get(103).unwrap();
    let p = PredictionBaseline {
        delta_v_m_s: 3240.0,
        apoapsis_altitude_m: 380_000_000.0,
        periapsis_altitude_m: 300_000.0,
        remaining_propellant_kg: 3000.0,
        time_of_flight_s: 400_000.0,
        target_miss_distance_m: None,
    };
    let mut state = c.initial;
    state.mass = c.dry_mass_kg - 1.0;
    assert_eq!(score(&c, state, 100.0, p, 3240.0, false).unwrap().s, 0.0);
    assert_eq!(score(&c, c.initial, 100.0, p, 3240.0, true).unwrap().s, 0.0);
}
