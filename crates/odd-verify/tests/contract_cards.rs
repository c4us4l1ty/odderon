//! Slice 1 acceptance: the multi-day contracts must be physically consistent
//! with the boil-off model and completable in **both** hand-sized and
//! fully-delegated modes.
//!
//! The tank card a contract recommends is a design input, not a fudge
//! factor: it is sized from the heat the model actually integrates over that
//! contract's own reference flight, and the gate below re-derives the
//! consequence (kilograms lost, cooler joules, delta-v margin) every run.

use odd_contracts::{Contract, DelegationMask, ThermalDesign};
use odd_ephem::Ephemeris;
use odd_verify::{compute, FlightSettings, ThermalInputs};

const EPOCH: i64 = odd_contracts::START_EPOCH;

/// Fly the contract's own reference flight and return the thermal outcome
/// and the ideal delta-v the stage can actually deliver.
fn fly(contract: &Contract, mask: DelegationMask, design: &ThermalDesign) -> (f64, f64, f64, f64) {
    let eph = Ephemeris::bundled().unwrap();
    let vehicle = contract
        .vehicle_delegated(contract.engine_spec, mask, design)
        .expect("every contract must be designable in both modes");
    let mut initial = contract.initial;
    initial.mass = vehicle.initial_mass_kg;
    let load = vehicle.initial_mass_kg - vehicle.dry_mass_kg;
    let flown = vehicle.effective;
    let settings = FlightSettings {
        contract_id: contract.id,
        epoch: EPOCH,
        initial,
        duration_us: contract.duration_us,
        dry_mass: vehicle.dry_mass_kg,
        engine: vehicle.engine,
        node_metric: contract.id == 102,
        delegation: mask,
        thermal: Some(ThermalInputs {
            engine_spec: flown.engine,
            design: flown.thermal,
            load_kg: load,
        }),
    };
    let plan = if contract.id == 103 {
        let split = odd_contracts::split_tug(3, 3_240.0, 1_600.0, &eph)
            .expect("the reference three-kick plan targets");
        odd_sim::planner::Plan::compile_guided(
            initial,
            odd_contracts::EARTH_MU,
            contract.duration_us,
            vehicle.dry_mass_kg,
            vehicle.engine,
            &split.nodes,
            &split.guidance,
        )
        .expect("the reference kicks compile")
    } else {
        odd_sim::planner::Plan::compile(
            initial,
            odd_contracts::EARTH_MU,
            contract.duration_us,
            vehicle.dry_mass_kg,
            vehicle.engine,
            &[],
        )
        .expect("the coast compiles")
    };
    let outcome = compute(&settings, &plan, None, &eph).expect("the reference flight evaluates");
    let ve = vehicle.engine.specific_impulse_seconds * odd_core::consts::STANDARD_GRAVITY_M_S2;
    let dv_ideal = ve * libm::log(vehicle.initial_mass_kg / vehicle.dry_mass_kg);
    (
        outcome.thermal.lost_kg,
        outcome.thermal.cooler_energy_j,
        dv_ideal,
        vehicle.dry_mass_kg,
    )
}

/// Both multi-day contracts must be completable by hand and by the chiefs,
/// with propellant margin, and their cryogenic loss must stay inside the
/// card's stated budget.
#[test]
fn oracle_multiday_contracts_are_solvable_in_both_modes() {
    for id in [102, 103] {
        let contract = Contract::get(id).unwrap();
        let card = contract.thermal_card();
        for (name, mask) in [
            ("manual", DelegationMask::MANUAL),
            ("delegated", DelegationMask::ALL),
        ] {
            let (lost, joules, dv, dry) = fly(&contract, mask, &card);
            let load = contract.propellant_budget_kg;
            println!(
                "contract {id} {name}: dry {dry:.1} kg, ideal Δv {dv:.1} m/s (needs {:.1}), \
                 boil-off {lost:.3} kg = {:.3} % of load, cooler {:.3} MJ",
                contract.reference_delta_v_m_s,
                100.0 * lost / load,
                joules / 1e6
            );
            assert!(
                dv > contract.reference_delta_v_m_s,
                "contract {id} must be completable in {name} mode: {dv:.1} vs {:.1} m/s",
                contract.reference_delta_v_m_s
            );
            // A 15 % propellant margin is the card's promise: the mission is
            // still flyable after the coast's losses are accounted for.
            assert!(
                dv * 0.85 > contract.reference_delta_v_m_s,
                "contract {id} {name} must keep a 15 % Δv margin"
            );
            if id == 102 {
                assert_eq!(lost, 0.0, "a storable stage cannot boil off");
                assert_eq!(joules, 0.0, "a storable stage needs no cryocooler");
            } else {
                assert!(
                    lost < 0.02 * load,
                    "contract {id} {name} must lose under 2 % of its load, lost {lost:.1} kg"
                );
            }
        }
    }
}

/// The recommended card must be a real, massed design — not a free upgrade.
#[test]
fn the_recommended_card_is_paid_for_and_beatable() {
    let tug = Contract::get(103).unwrap();
    let card = tug.thermal_card();
    let plain = tug.vehicle(tug.engine_spec).unwrap();
    let fitted = tug
        .vehicle_delegated(tug.engine_spec, DelegationMask::MANUAL, &card)
        .unwrap();
    println!(
        "tug card costs {:.3} kg of fit-out (dry {:.1} -> {:.1} kg)",
        fitted.thermal_fitout_kg, plain.dry_mass_kg, fitted.dry_mass_kg
    );
    assert!(fitted.thermal_fitout_kg > 0.0);
    assert!(fitted.dry_mass_kg > plain.dry_mass_kg);
    // Storables get no cryogenic card at all: there is nothing to insulate.
    for id in [101, 102] {
        let c = Contract::get(id).unwrap();
        assert_eq!(c.thermal_card(), ThermalDesign::baseline());
    }
}
