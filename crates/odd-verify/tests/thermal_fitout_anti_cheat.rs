//! Cryogenic hardware must be paid for in dry mass, and the verifier must
//! recompute that mass from the signed fit-out.
//!
//! Before this gate, `Contract::vehicle` charged only the engine and the
//! airframe: a player could bolt on 400 m² of radiator and sixty blanket
//! layers, get zero boil-off, and carry none of the kilograms. Active cooling
//! was free, so the cryogenic design lever had no trade-off. These tests pin
//! both halves: the mass is charged, and a record cannot claim the hardware
//! without carrying it.

use odd_contracts::{Contract, DelegationMask, Department, ThermalDesign};
use odd_ephem::Ephemeris;
use odd_identity::License;
use odd_report::{Fdr, FdrError, StageThermalSpec, VentEvent, SCHEMA_VERSION};
use odd_sim::planner::Plan;
use odd_verify::{compute, verify_record, FlightSettings, ThermalInputs};

const EPOCH: i64 = odd_contracts::START_EPOCH;

fn tug() -> Contract {
    Contract::get(103).unwrap()
}

/// A design with real active cooling fitted.
fn cooled_design() -> ThermalDesign {
    let mut design = ThermalDesign::baseline();
    design.mli_layers = 40;
    design.epsilon_eff = odd_systems::ideal_epsilon_for_layers(40) * 4.0;
    design.radiator_area_m2 = 40.0;
    design.cryocooler_lift_w = 400.0;
    design
}

fn thermal_spec(design: &ThermalDesign, contract: &Contract, load: f64) -> StageThermalSpec {
    let stage = design
        .stage(contract, &contract.engine_spec, load)
        .expect("the tug is cryogenic");
    StageThermalSpec {
        stage: 0,
        tank_volumes_m3: [stage.oxidizer.volume_m3, stage.fuel.volume_m3],
        propellant_id: contract.engine_spec.propellant as u8,
        mli_layers: design.mli_layers,
        epsilon_eff: design.epsilon_eff,
        tank_shell_material: design.shell,
        absorptivity: design.absorptivity,
        outer_epsilon: design.outer_epsilon,
        strut_conductance_w_k: design.strut_conductance_w_k,
        relief_pressure_pa: design.relief_pressure_pa,
        radiator_area_m2: design.radiator_area_m2,
        radiator_epsilon: design.radiator_epsilon,
        cryocooler_w: design.cryocooler_lift_w,
        cryocooler_carnot_fraction: design.cryocooler_carnot_fraction,
        array_w_at_1au: design.bus.array_w_at_1au,
        avionics_w: design.bus.avionics_w,
        electric_propulsion_w: design.bus.electric_propulsion_w,
        structure_temperature_k: design.structure_temperature_k,
        sink_temperature_k: design.sink_temperature_k,
    }
}

/// Fly a real coast on the stage this design produces, and sign it honestly.
fn signed_stage(design: &ThermalDesign, mask: DelegationMask) -> (Fdr, License) {
    let eph = Ephemeris::bundled().unwrap();
    let contract = tug();
    let vehicle = contract
        .vehicle_delegated(contract.engine_spec, mask, design)
        .expect("the stage is designable");
    let mut initial = contract.initial;
    initial.mass = vehicle.initial_mass_kg;
    let load = vehicle.initial_mass_kg - vehicle.dry_mass_kg;
    let settings = FlightSettings {
        contract_id: contract.id,
        epoch: EPOCH,
        initial,
        duration_us: contract.duration_us,
        dry_mass: vehicle.dry_mass_kg,
        engine: vehicle.engine,
        node_metric: false,
        delegation: mask,
        revision: odd_core::PhysicsRevision::CoupledFitted,
        thermal: Some(ThermalInputs {
            engine_spec: vehicle.effective.engine,
            design: vehicle.effective.thermal,
            load_kg: load,
        }),
    };
    let plan = Plan::compile(
        initial,
        odd_contracts::EARTH_MU,
        contract.duration_us,
        vehicle.dry_mass_kg,
        vehicle.engine,
        &[],
    )
    .unwrap();
    let outcome = compute(&settings, &plan, None, &eph).expect("the coast evaluates");
    let mut fdr = Fdr {
        schema_version: SCHEMA_VERSION,
        contract_id: contract.id,
        attempt_mode: odd_report::AttemptMode::Ranked,
        callsign: "FITOUT ORACLE".into(),
        epoch_s: EPOCH,
        duration_us: contract.duration_us,
        initial_mass_kg: vehicle.initial_mass_kg,
        dry_mass_kg: vehicle.dry_mass_kg,
        thrust_n: vehicle.solution.thrust_n,
        specific_impulse_s: vehicle.solution.specific_impulse_s,
        engine: Some(contract.engine_spec),
        initial_state: [
            initial.position[0],
            initial.position[1],
            initial.position[2],
            initial.velocity[0],
            initial.velocity[1],
            initial.velocity[2],
        ],
        node_metric: false,
        prediction_baseline: [f64::NAN; 6],
        commands: Vec::new(),
        metrics: outcome.report,
        final_state_hash: odd_report::fdr::final_state_hash(
            outcome.report[29],
            &[
                outcome.final_state.position[0],
                outcome.final_state.position[1],
                outcome.final_state.position[2],
                outcome.final_state.velocity[0],
                outcome.final_state.velocity[1],
                outcome.final_state.velocity[2],
            ],
            outcome.final_state.mass,
        ),
        elo_before: 1200.0,
        elo_after: 1200.0,
        thermal_specs: vec![thermal_spec(&vehicle.effective.thermal, &contract, load)],
        delegation_mask: mask.0,
        boil_off_kg: outcome.thermal.tank_lost_kg,
        cryocooler_energy_j: outcome.thermal.cooler_energy_j,
        vent_events: outcome
            .thermal
            .vents
            .iter()
            .map(|event| VentEvent {
                time_us: event.time_us,
                tank: event.tank,
                pressure_pa: event.pressure_pa,
                temperature_k: event.temperature_k,
            })
            .collect(),
        flown_burn_windows: vec![],
        flameout_time_s: None,
        propellant_remaining_kg: None,
        public_key: [0; 32],
        signature: [0; 64],
    };
    let license = License::from_entropy([7u8; 16]);
    fdr.sign(&license).unwrap();
    (fdr, license)
}

/// Fitting a heavier blanket and a radiator must cost dry mass, and the
/// charge must equal the hardware the thermal model actually integrates.
#[test]
fn cryogenic_hardware_is_charged_to_the_stage() {
    let contract = tug();
    let baseline = ThermalDesign::baseline();
    let plain = contract.vehicle(contract.engine_spec).unwrap();
    assert_eq!(
        plain.thermal_fitout_kg, 0.0,
        "the contract baseline must stay bit-identical for existing certificates"
    );
    assert_eq!(plain.dry_mass_kg, contract.dry_mass_kg);

    let cooled = contract
        .vehicle_delegated(
            contract.engine_spec,
            DelegationMask::MANUAL,
            &cooled_design(),
        )
        .unwrap();
    // 20 extra layers at 0.0125 kg/m² plus 40 m² of panel at 4 kg/m².
    let radius = libm::cbrt(3.0 * contract.tank_volume_m3 / (4.0 * std::f64::consts::PI));
    let area = 4.0 * std::f64::consts::PI * radius * radius;
    let expected = 20.0 * 0.0125 * area + 40.0 * 4.0;
    println!(
        "fit-out: {:.3} kg (expected {:.3}), dry {:.3} -> {:.3} kg",
        cooled.thermal_fitout_kg, expected, plain.dry_mass_kg, cooled.dry_mass_kg
    );
    assert!((cooled.thermal_fitout_kg - expected).abs() < 1e-9);
    assert!((cooled.dry_mass_kg - plain.dry_mass_kg - expected).abs() < 1e-9);

    // A lighter blanket is genuinely lighter — the trade-off runs both ways.
    let mut thin = baseline;
    thin.mli_layers = 4;
    thin.epsilon_eff = odd_systems::ideal_epsilon_for_layers(4) * 4.0;
    let thin_vehicle = contract
        .vehicle_delegated(contract.engine_spec, DelegationMask::MANUAL, &thin)
        .unwrap();
    assert!(thin_vehicle.thermal_fitout_kg < 0.0);
    assert!(thin_vehicle.dry_mass_kg < plain.dry_mass_kg);

    // A storable stage has no fit-out and must not be charged for one.
    let rescue = Contract::get(101).unwrap();
    let storable = rescue
        .vehicle_delegated(rescue.engine_spec, DelegationMask::ALL, &cooled_design())
        .unwrap();
    assert_eq!(storable.thermal_fitout_kg, 0.0);
    assert!(
        (storable.dry_mass_kg
            - rescue.vehicle(rescue.engine_spec).unwrap().dry_mass_kg
            - storable.delegation_margin_kg)
            .abs()
            < 1e-9,
        "only the chief's margin may change a storable stage"
    );
}

/// The exploit this gate exists for: claim active cooling, keep the light
/// baseline dry mass, re-sign. The verifier recomputes the fit-out mass from
/// the signed spec and must refuse.
#[test]
fn oracle_cooling_cannot_be_claimed_without_its_mass() {
    let design = cooled_design();
    let (honest, license) = signed_stage(&design, DelegationMask::MANUAL);
    verify_record(&honest).expect("an honestly massed cooled stage must verify");

    let contract = tug();
    let light = contract.vehicle(contract.engine_spec).unwrap();
    let mut cheating = honest.clone();
    cheating.dry_mass_kg = light.dry_mass_kg;
    cheating.initial_mass_kg = light.initial_mass_kg;
    cheating.sign(&license).unwrap();
    assert_eq!(cheating.verify_signature(), Ok(()));
    let verdict = verify_record(&cheating);
    assert!(
        matches!(verdict, Err(FdrError::EngineMismatch("dry mass"))),
        "free cooling must be rejected at the mass gate, got {:?}",
        verdict.err()
    );
    println!(
        "rejected a re-signed claim of {:.1} kg of cooling hardware carried for free",
        honest.dry_mass_kg - light.dry_mass_kg
    );

    // The same attack on the delegation margin: drop the mask's mass but keep
    // the score the heavy stage earned.
    let mask = DelegationMask::MANUAL.with(Department::Thermal, true);
    let (delegated, dl) = signed_stage(&design, mask);
    verify_record(&delegated).expect("an honestly massed delegated stage must verify");
    let mut shed = delegated.clone();
    shed.dry_mass_kg -= delegated.dry_mass_kg - honest.dry_mass_kg;
    shed.sign(&dl).unwrap();
    assert!(
        matches!(
            verify_record(&shed),
            Err(FdrError::EngineMismatch("dry mass"))
        ),
        "a chief's margin cannot be signed away"
    );
    println!("rejected a re-signed attempt to shed the chief's margin");
}
