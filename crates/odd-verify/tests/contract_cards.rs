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
use odd_sim::mission::{LockedMission, PredictionBaseline};
use odd_verify::{compute, FlightSettings, ThermalInputs};

const EPOCH: i64 = odd_contracts::START_EPOCH;

/// Fly an objective-reaching plan on the actual assembled stage. Return the
/// measured thermal loss and a post-hoc ideal delta-v bound; boil-off itself
/// is not fed back into the currently frozen trajectory model.
fn fly(
    contract: &Contract,
    mask: DelegationMask,
    design: &ThermalDesign,
) -> (f64, f64, f64, f64, f64) {
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
        let mut stage = *contract;
        stage.initial = initial;
        stage.dry_mass_kg = vehicle.dry_mass_kg;
        stage.engine = vehicle.engine;
        odd_contracts::split_tug_for_stage(
            &stage,
            3,
            if mask == DelegationMask::ALL {
                3_252.0
            } else {
                3_240.0
            },
            1_600.0,
            &eph,
        )
        .expect("the reference three-kick plan targets this stage")
        .plan
    } else {
        odd_sim::planner::Plan::compile(
            initial,
            odd_contracts::EARTH_MU,
            contract.duration_us,
            vehicle.dry_mass_kg,
            vehicle.engine,
            &contract.example_nodes(),
        )
        .expect("the circularization compiles")
    };
    // Fix the prediction before evaluation. The objective component is
    // independent of prediction accuracy, but using the real locked-flight
    // path tests the same score the authority will replay.
    let predicted_altitude = if contract.id == 102 {
        800_000.0
    } else {
        380_000_000.0 - odd_contracts::EARTH_EQUATORIAL_RADIUS
    };
    let lock = LockedMission::new(
        plan.clone(),
        PredictionBaseline {
            delta_v_m_s: plan.requested_delta_v_m_s,
            apoapsis_altitude_m: predicted_altitude,
            periapsis_altitude_m: if contract.id == 102 {
                800_000.0
            } else {
                200_000.0
            },
            remaining_propellant_kg: load * 0.1,
            time_of_flight_s: contract.duration_us as f64 / 1e6,
            target_miss_distance_m: None,
        },
        vehicle.dry_mass_kg,
    )
    .unwrap();
    let outcome =
        compute(&settings, &plan, Some(&lock), &eph).expect("the reference flight evaluates");
    let report = &outcome.report;
    assert_eq!(report[30], 0.0, "the flight must not enter the atmosphere");
    assert!(
        report[0] >= vehicle.dry_mass_kg,
        "the flown stage must not exhaust its propellant"
    );
    assert!(
        report[35] >= 0.8,
        "contract {} mask {} must reach the catalog objective, got {}",
        contract.id,
        mask.0,
        report[35]
    );
    if contract.id == 102 {
        assert!(report[3] < 0.005, "sun-synchronous orbit must be circular");
        assert!((report[1] - 800_000.0).abs() <= 10_000.0);
        assert!((report[2] - 800_000.0).abs() <= 10_000.0);
        assert!((report[26].to_degrees() - 98.60).abs() <= 0.10);
        assert!((report[27].to_degrees() - 4.93).abs() <= 0.15);
    } else {
        let radius = report[1] + odd_contracts::EARTH_EQUATORIAL_RADIUS;
        assert!((radius - 380_000_000.0).abs() <= 15_000_000.0);
        assert!(report[2] >= odd_contracts::ENTRY_ALTITUDE);
    }
    println!(
        "flight {} mask {}: apo {:.3} km, peri {:.3} km, objective {:.4}",
        contract.id,
        mask.0,
        report[1] / 1000.0,
        report[2] / 1000.0,
        report[35]
    );
    let ve = vehicle.engine.specific_impulse_seconds * odd_core::consts::STANDARD_GRAVITY_M_S2;
    // Deduct the entire measured coast loss before applying the rocket
    // equation. This is an idealized post-hoc margin, not feedback into the
    // trajectory, a per-tank burn accounting, or a real-world guarantee.
    let dv_after_loss =
        ve * libm::log((vehicle.initial_mass_kg - outcome.thermal.lost_kg) / vehicle.dry_mass_kg);
    (
        outcome.thermal.lost_kg,
        outcome.thermal.cooler_energy_j,
        dv_after_loss,
        vehicle.dry_mass_kg,
        plan.requested_delta_v_m_s,
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
            let (lost, joules, dv, dry, commanded) = fly(&contract, mask, &card);
            let load = contract.propellant_budget_kg;
            println!(
                "contract {id} {name}: dry {dry:.1} kg, ideal Δv after loss {dv:.1} m/s (commanded {commanded:.1}), \
                 boil-off {lost:.3} kg = {:.3} % of load, cooler {:.3} MJ",
                100.0 * lost / load,
                joules / 1e6
            );
            assert!(
                dv > commanded,
                "contract {id} {name} has insufficient post-loss ideal Δv: {dv:.1} vs {commanded:.1} m/s"
            );
            // This checks an ideal Δv margin after subtracting the measured
            // loss, not a 15% propellant margin or coupled-flight proof.
            assert!(
                dv * 0.85 > commanded,
                "contract {id} {name} must keep a 15% ideal Δv margin after loss"
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

/// Preserve the original cooled-tug physics pin on its original maneuver
/// inputs. The UI targeter now uses the assembled mass rather than the
/// catalog baseline, so its *new* trajectory has a different thermal result;
/// changing the input must not silently rebaseline this fixed regression.
#[test]
fn baseline_targeted_cooled_tug_preserves_thermal_physics_pin() {
    let contract = Contract::get(103).unwrap();
    let eph = Ephemeris::bundled().unwrap();
    let mut design = ThermalDesign::baseline();
    design.mli_layers = 40;
    design.epsilon_eff = 4.0 * odd_systems::ideal_epsilon_for_layers(40);
    design.radiator_area_m2 = 40.0;
    design.cryocooler_lift_w = 400.0;
    design.bus.array_w_at_1au = 16_000.0;
    let vehicle = contract
        .vehicle_delegated(contract.engine_spec, DelegationMask::MANUAL, &design)
        .unwrap();
    let mut initial = contract.initial;
    initial.mass = vehicle.initial_mass_kg;
    let baseline = odd_contracts::split_tug(3, 3_240.0, 1_600.0, &eph).unwrap();
    let plan = odd_sim::planner::Plan::compile_guided(
        initial,
        odd_contracts::EARTH_MU,
        contract.duration_us,
        vehicle.dry_mass_kg,
        vehicle.engine,
        &baseline.nodes,
        &baseline.guidance,
    )
    .unwrap();
    let settings = FlightSettings {
        contract_id: contract.id,
        epoch: EPOCH,
        initial,
        duration_us: contract.duration_us,
        dry_mass: vehicle.dry_mass_kg,
        engine: vehicle.engine,
        node_metric: false,
        delegation: DelegationMask::MANUAL,
        thermal: Some(ThermalInputs {
            engine_spec: vehicle.effective.engine,
            design: vehicle.effective.thermal,
            load_kg: vehicle.propellant_kg,
        }),
    };
    let result = compute(&settings, &plan, None, &eph).unwrap();
    assert_eq!(format!("{:.6}", result.thermal.lost_kg), "3.843961");
    assert_eq!(
        format!("{:.6}", result.thermal.cooler_energy_j / 1e6),
        "4209.826545"
    );
}
