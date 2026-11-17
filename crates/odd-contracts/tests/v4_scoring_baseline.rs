//! What the v4 score is measured against, and what that costs a redesign.
//!
//! `score_delegated` reads `contract.dry_mass_kg` — the **catalog baseline**
//! vehicle — for two of its five terms:
//!
//! * the fuel-failure threshold, `state.mass >= contract.dry_mass_kg`;
//! * the prediction term, `|predicted_remaining − (state.mass − contract.dry_mass_kg)|`.
//!
//! The v4 flight itself is flown at the **fitted** dry mass, which differs
//! whenever the player strips or buys insulation, a cooler, a radiator, an
//! array or a chief's margin. So the score reasons about a vehicle that is not
//! the one that flew.
//!
//! These tests measure the consequence in both directions, because the error
//! does not have one sign:
//!
//! * **Heavier** than catalog (e.g. the recommended thermal card): the term
//!   `mass − catalog_dry` overstates the propellant aboard, so a correctly
//!   predicted flight is scored as if the prediction were wrong.
//! * **Lighter** than catalog (e.g. stripping the baseline blanket): the term
//!   understates it, and the fuel threshold can fail while propellant remains —
//!   the hazard the design notes warned about, confirmed reachable here.
//!
//! Fixing either rescales signed results, so it is a versioned scoring
//! migration, not a bug fix. These tests pin today's behaviour so that
//! migration has a baseline to prove itself against.

use odd_contracts::EARTH_EQUATORIAL_RADIUS;
use odd_contracts::{Contract, DelegationMask, MissionScore, Objective, ThermalDesign};
use odd_sim::mission::PredictionBaseline;

/// A prediction that names a plausible mission, used only to hold the
/// non-mass terms still between cases.
fn prediction(remaining_propellant_kg: f64) -> PredictionBaseline {
    PredictionBaseline {
        delta_v_m_s: 3_240.0,
        apoapsis_altitude_m: 380_000_000.0 - EARTH_EQUATORIAL_RADIUS,
        periapsis_altitude_m: 200_000.0,
        remaining_propellant_kg,
        time_of_flight_s: 6.0 * 86_400.0,
        target_miss_distance_m: None,
    }
}

/// A terminal state at the tug's target radius. The velocity is deliberately
/// not circular: only the mass terms and the fuel gate are read here, and a
/// genuine 380,000 km circular speed would be meaningless at that radius.
fn state_with(dry: f64, remaining: f64) -> odd_astro::spline::Keyframe {
    let r = EARTH_EQUATORIAL_RADIUS + 380_000_000.0;
    odd_astro::spline::Keyframe {
        time: 0.0,
        position: [r, 0.0, 0.0],
        velocity: [0.0, 1.0, 0.0],
        mass: dry + remaining,
    }
}

fn score(
    contract: &Contract,
    dry: f64,
    remaining: f64,
    prediction: PredictionBaseline,
) -> MissionScore {
    odd_contracts::score_delegated(
        contract,
        state_with(dry, remaining),
        6.0 * 86_400.0,
        prediction,
        contract.reference_delta_v_m_s,
        false,
        DelegationMask::MANUAL,
    )
    .expect("the terms evaluate")
}

/// The fitted dry mass for a given blanket, from the shipped mass accounting.
fn fitted_dry(layers: u32) -> f64 {
    let contract = Contract::get(103).unwrap();
    let mut design = ThermalDesign::baseline();
    design.mli_layers = layers;
    design.epsilon_eff = 4.0 * odd_systems::ideal_epsilon_for_layers(layers.max(1));
    contract
        .vehicle_delegated(contract.engine_spec, DelegationMask::MANUAL, &design)
        .unwrap()
        .dry_mass_kg
}

/// A correctly predicted flight is penalised purely for carrying more hardware.
///
/// Both cases fly the same trajectory, reach the same instant with the same
/// propellant, and predict that propellant exactly. Only the dry mass the
/// catalogue supplies to the score differs.
#[test]
fn carrying_more_hardware_lowers_the_prediction_term_for_an_identical_flight() {
    let contract = Contract::get(103).unwrap();
    let catalog = contract.dry_mass_kg;
    let heavier = catalog + 264.0; // ~the recommended thermal card
    let remaining = 1_000.0;
    let honest = prediction(remaining);

    let baseline = score(&contract, catalog, remaining, honest);
    let redesigned = score(&contract, heavier, remaining, honest);

    println!(
        "catalog dry {catalog:.3}: prediction {:.9}\n\
         fitted dry {heavier:.3}: prediction {:.9}  (ratio {:.6})",
        baseline.prediction,
        redesigned.prediction,
        redesigned.prediction / baseline.prediction
    );

    // The same prediction, the same propellant, a heavier stage: the score
    // falls, because `state.mass − catalog_dry` invents the mass difference as
    // leftover propellant the pilot failed to predict.
    assert!(
        redesigned.prediction < baseline.prediction,
        "a heavier fitted stage is scored as a worse prediction even though the \
         prediction is exactly right: {:.9} vs {:.9}",
        redesigned.prediction,
        baseline.prediction
    );
    // The objective term is a function of the orbit alone and must not move.
    assert_eq!(
        redesigned.objectives.to_bits(),
        baseline.objectives.to_bits(),
        "the objective term must not depend on dry mass"
    );
}

/// Stripping the baseline blanket makes the stage lighter than the catalog
/// vehicle, and then the fuel gate can fail with propellant still aboard.
///
/// This is the hazard `docs/software-design.md` described. It is reachable.
#[test]
fn a_lighter_stage_can_be_declared_out_of_fuel_while_fuel_remains() {
    let contract = Contract::get(103).unwrap();
    let catalog = contract.dry_mass_kg;
    let lighter = fitted_dry(0);
    assert!(
        lighter < catalog,
        "the zero-layer stage must be lighter than the catalogue vehicle: \
         {lighter:.3} vs {catalog:.3} kg"
    );
    let gap = catalog - lighter;

    // Still flying, with less propellant aboard than the mass the score is
    // comparing against. The vehicle is fine; the score is not.
    let remaining = gap / 2.0;
    let result = score(&contract, lighter, remaining, prediction(remaining));
    println!(
        "fitted {lighter:.3} kg + {remaining:.3} kg aboard vs catalog {catalog:.3} kg \
         -> total score {:.6}",
        result.s
    );
    assert!(
        result.s == 0.0,
        "the catalog threshold should declare this flight a fuel failure"
    );

    // One kilogram more clears it, which shows the failure is a threshold
    // artefact and not a physical limit.
    let cleared = score(&contract, lighter, gap + 1.0, prediction(gap + 1.0));
    assert!(
        cleared.s > 0.0,
        "the same vehicle with 1 kg more scores above zero, got {:.6}",
        cleared.s
    );
}

/// The objective term never reads dry mass, whatever the stage weighs.
#[test]
fn the_objective_term_depends_on_the_orbit_and_not_the_vehicle() {
    let contract = Contract::get(103).unwrap();
    assert_eq!(contract.objective, Objective::TransLunarApoapsis);
    let catalog = contract.dry_mass_kg;
    let reference = score(&contract, catalog, 5_000.0, prediction(5_000.0));
    for delta in [-300.0, -14.0, 0.0, 264.0, 900.0] {
        let other = score(&contract, catalog + delta, 5_000.0, prediction(5_000.0));
        assert_eq!(
            other.objectives.to_bits(),
            reference.objectives.to_bits(),
            "the objective term moved when dry mass changed by {delta} kg"
        );
    }
}
