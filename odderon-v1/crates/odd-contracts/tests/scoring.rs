//! Mission score rules are explicit game rules; the authority applies Elo.
use odd_contracts::{
    score, Contract, DelegationMask, ThermalDesign, EARTH_EQUATORIAL_RADIUS, EARTH_MU,
};
use odd_sim::mission::{apsides, PredictionBaseline};
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

/// Pin the current scoring limitation rather than silently changing signed
/// v3 outcomes. The UI and dossier disclose this difference; correcting it
/// requires a versioned scoring migration.
#[test]
fn frozen_score_uses_catalog_dry_mass_for_fuel_and_prediction() {
    let c = Contract::get(103).unwrap();
    let mut thin = ThermalDesign::baseline();
    thin.mli_layers = 4;
    thin.epsilon_eff = 4.0 * odd_systems::ideal_epsilon_for_layers(4);
    let fitted = c
        .vehicle_delegated(c.engine_spec, DelegationMask::MANUAL, &thin)
        .unwrap();
    assert!(fitted.dry_mass_kg < c.dry_mass_kg);

    let mut state = c.initial;
    state.mass = (fitted.dry_mass_kg + c.dry_mass_kg) / 2.0;
    assert!(state.mass > fitted.dry_mass_kg && state.mass < c.dry_mass_kg);
    let ap = apsides(state, EARTH_MU, EARTH_EQUATORIAL_RADIUS).unwrap();
    let mut prediction = PredictionBaseline {
        delta_v_m_s: 3_240.0,
        apoapsis_altitude_m: ap.apoapsis_altitude_m,
        periapsis_altitude_m: ap.periapsis_altitude_m,
        remaining_propellant_kg: state.mass - fitted.dry_mass_kg,
        time_of_flight_s: 100.0,
        target_miss_distance_m: None,
    };
    assert_eq!(
        score(&c, state, 100.0, prediction, 3_240.0, false)
            .unwrap()
            .s,
        0.0,
        "the frozen fuel threshold uses catalog dry mass, not this lighter fit"
    );

    state.mass = c.dry_mass_kg + 10.0;
    prediction.remaining_propellant_kg = state.mass - c.dry_mass_kg;
    let catalog_prediction = score(&c, state, 100.0, prediction, 3_240.0, false).unwrap();
    assert_eq!(catalog_prediction.prediction, 1.0);
    prediction.remaining_propellant_kg = state.mass - fitted.dry_mass_kg;
    let fitted_prediction = score(&c, state, 100.0, prediction, 3_240.0, false).unwrap();
    assert!(fitted_prediction.prediction < catalog_prediction.prediction);
}
