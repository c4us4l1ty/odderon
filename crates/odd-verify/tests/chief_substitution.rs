//! WP 1.7 acceptance gate: `oracle_chief_hardware_substitution`.
//!
//! Delegation used to mean "the player's sliders, plus 35 %". That let a
//! player hand-optimise a discipline, tick its AUTO-DELEGATED box, and pay
//! only a margin on hardware the chief would never have chosen. Substitution
//! closes it: a delegated discipline's flown specification **is** the chief's
//! textbook choice, and the verifier refuses a record that claims the bit
//! while carrying anything else.

use odd_contracts::chief_ai::{self, DelegationMask, Department};
use odd_contracts::{Contract, ThermalDesign};
use odd_ephem::Ephemeris;
use odd_identity::License;
use odd_report::{Fdr, FdrError, StageThermalSpec, VentEvent, SCHEMA_VERSION};
use odd_sim::planner::Plan;
use odd_verify::{compute, verify_record, FlightSettings, ThermalInputs};

const EPOCH: i64 = odd_contracts::START_EPOCH;

/// A named edit to the signed thermal spec.
type SpecEdit = (&'static str, Box<dyn Fn(&mut StageThermalSpec)>);

fn tug() -> Contract {
    Contract::get(103).unwrap()
}

/// A player who has hand-worked a lighter, colder stage than any chief.
fn hand_worked() -> ThermalDesign {
    let mut design = ThermalDesign::baseline();
    design.mli_layers = 24;
    design.epsilon_eff = odd_systems::ideal_epsilon_for_layers(24) * 3.0;
    design.strut_conductance_w_k = 0.2;
    design.bus.array_w_at_1au = 2_600.0;
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

/// Fly and sign the stage the bridge would actually build: the substituted
/// engine and thermal design, with the mass that substitution implies.
fn signed_stage(mask: DelegationMask, player: &ThermalDesign) -> (Fdr, License) {
    let eph = Ephemeris::bundled().unwrap();
    let contract = tug();
    let vehicle = contract
        .vehicle_delegated(contract.engine_spec, mask, player)
        .expect("the stage is designable");
    let flown = vehicle.effective;
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
        thermal: Some(ThermalInputs {
            engine_spec: flown.engine,
            design: flown.thermal,
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
        callsign: "CHIEF ORACLE".into(),
        epoch_s: EPOCH,
        duration_us: contract.duration_us,
        initial_mass_kg: vehicle.initial_mass_kg,
        dry_mass_kg: vehicle.dry_mass_kg,
        thrust_n: vehicle.solution.thrust_n,
        specific_impulse_s: vehicle.solution.specific_impulse_s,
        engine: Some(flown.engine),
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
        thermal_specs: vec![thermal_spec(&flown.thermal, &contract, load)],
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
    let license = License::from_entropy([21u8; 16]);
    fdr.sign(&license).unwrap();
    (fdr, license)
}

/// Ticking a discipline's box must replace its hardware, not pad the
/// player's, and the flown vehicle must carry the chief's mass.
#[test]
fn delegation_substitutes_the_chiefs_textbook_hardware() {
    let contract = tug();
    let player = hand_worked();
    let manual = contract
        .vehicle_delegated(contract.engine_spec, DelegationMask::MANUAL, &player)
        .unwrap();
    assert_eq!(manual.effective.thermal.mli_layers, 24);
    assert_eq!(manual.effective.thermal.bus.array_w_at_1au, 2_600.0);

    // Thermal: the chief's blanket and mounts replace the player's, even
    // though the player's were better.
    let thermal = contract
        .vehicle_delegated(
            contract.engine_spec,
            DelegationMask::MANUAL.with(Department::Thermal, true),
            &player,
        )
        .unwrap();
    let choice = chief_ai::thermal_choice(&contract);
    assert_eq!(thermal.effective.thermal.mli_layers, choice.mli_layers);
    assert_eq!(thermal.effective.thermal.shell, choice.shell);
    assert_eq!(
        thermal.effective.thermal.strut_conductance_w_k,
        choice.strut_conductance_w_k
    );
    // The array is Power's, so it survives a thermal-only delegation.
    assert_eq!(thermal.effective.thermal.bus.array_w_at_1au, 2_600.0);
    assert!(
        thermal.dry_mass_kg > manual.dry_mass_kg,
        "the chief's heavier blanket must be carried"
    );

    // Power: the chief sizes the array, the player's value is discarded.
    let power = contract
        .vehicle_delegated(
            contract.engine_spec,
            DelegationMask::MANUAL.with(Department::Power, true),
            &player,
        )
        .unwrap();
    assert_eq!(
        power.effective.thermal.bus.array_w_at_1au,
        chief_ai::power_choice(&contract, &player)
    );
    assert!(power.effective.thermal.bus.array_w_at_1au > 2_600.0);
    assert!(power.dry_mass_kg > manual.dry_mass_kg);

    // GNC: the avionics draw is the chief's, and Power then sizes for it.
    let gnc = DelegationMask::MANUAL
        .with(Department::Gnc, true)
        .with(Department::Power, true);
    let both = contract
        .vehicle_delegated(contract.engine_spec, gnc, &player)
        .unwrap();
    assert_eq!(
        both.effective.thermal.bus.avionics_w,
        chief_ai::gnc_choice(&contract)
    );
    // The chief keeps the qualified array as a floor, so a bigger avionics
    // draw does not shrink it — the array is never lighter than the
    // contract's own, and never lighter than the worst-case load demands.
    assert!(
        both.effective.thermal.bus.array_w_at_1au >= power.effective.thermal.bus.array_w_at_1au
    );
    assert!(
        both.dry_mass_kg > power.dry_mass_kg,
        "the chief's avionics mass is carried"
    );

    // Propulsion: a hand-built engine is replaced by the qualified reference.
    let mut hot_rodded = contract.engine_spec;
    hot_rodded.p_c_pa *= 1.2;
    hot_rodded.r_throat_m *= 1.05;
    let prop = contract
        .vehicle_delegated(
            hot_rodded,
            DelegationMask::MANUAL.with(Department::Propulsion, true),
            &player,
        )
        .unwrap();
    assert_eq!(prop.effective.engine.p_c_pa, contract.engine_spec.p_c_pa);
    assert_eq!(
        prop.effective.engine.r_throat_m,
        contract.engine_spec.r_throat_m
    );
    let rodded_manual = contract
        .vehicle_delegated(hot_rodded, DelegationMask::MANUAL, &player)
        .unwrap();
    assert_ne!(
        prop.engine.thrust_newtons,
        rodded_manual.engine.thrust_newtons
    );

    // Substitution is deterministic and order-independent in the API: the
    // same mask and inputs always produce the same flown design.
    let again = contract
        .vehicle_delegated(contract.engine_spec, DelegationMask::ALL, &player)
        .unwrap();
    let repeat = contract
        .vehicle_delegated(contract.engine_spec, DelegationMask::ALL, &player)
        .unwrap();
    assert_eq!(again.dry_mass_kg.to_bits(), repeat.dry_mass_kg.to_bits());
    println!(
        "tug dry mass: manual {:.3} kg, thermal chief {:.3} kg, all chiefs {:.3} kg",
        manual.dry_mass_kg, thermal.dry_mass_kg, again.dry_mass_kg
    );
}

/// The enforcement half: an honest delegated record verifies, and a record
/// that claims a delegation bit while carrying non-chief hardware is refused
/// by name even after the attacker re-signs it with their own real key.
#[test]
fn oracle_chief_hardware_substitution() {
    let contract = tug();
    let player = hand_worked();
    let mask = DelegationMask::MANUAL
        .with(Department::Thermal, true)
        .with(Department::Power, true);
    let (honest, license) = signed_stage(mask, &player);
    verify_record(&honest).expect("an honestly substituted delegated stage must verify");
    assert_eq!(
        honest.thermal_specs[0].mli_layers,
        chief_ai::thermal_choice(&contract).mli_layers
    );

    // Attack: keep the delegation bits, swap in the player's lighter blanket
    // and smaller array, re-sign. Every edit must be refused by department.
    let edits: [SpecEdit; 4] = [
        ("thinner blanket", Box::new(|s| s.mli_layers = 24)),
        ("colder mounts", Box::new(|s| s.strut_conductance_w_k = 0.2)),
        ("better emittance", Box::new(|s| s.epsilon_eff *= 0.5)),
        ("smaller array", Box::new(|s| s.array_w_at_1au = 2_600.0)),
    ];
    for (name, edit) in edits {
        let mut forged = honest.clone();
        edit(&mut forged.thermal_specs[0]);
        forged.sign(&license).unwrap();
        assert_eq!(forged.verify_signature(), Ok(()));
        let verdict = verify_record(&forged);
        assert!(
            matches!(
                verdict,
                Err(FdrError::DelegationSpecMismatch("thermal"))
                    | Err(FdrError::DelegationSpecMismatch("power"))
            ),
            "{name} must be refused as a delegation deviation, got {:?}",
            verdict.err()
        );
    }
    println!("rejected four re-signed non-chief hardware claims");

    // The same hardware with the bits cleared is a legitimate manual stage:
    // the player is allowed to beat the chief, they just cannot claim the
    // chief did it.
    let (manual, _) = signed_stage(DelegationMask::MANUAL, &player);
    verify_record(&manual).expect("a hand-worked manual stage must still verify");
    assert!(manual.dry_mass_kg < honest.dry_mass_kg);
    println!(
        "manual {:.3} kg beats the chiefs' {:.3} kg, and both verify",
        manual.dry_mass_kg, honest.dry_mass_kg
    );

    // A propulsion claim carrying a non-reference engine is refused too.
    let prop_mask = DelegationMask::MANUAL.with(Department::Propulsion, true);
    let (prop, pl) = signed_stage(prop_mask, &player);
    verify_record(&prop).expect("an honestly substituted engine must verify");
    let mut forged = prop.clone();
    let mut engine = forged.engine.unwrap();
    engine.r_throat_m *= 1.01;
    // Make the forgery self-consistent: recompute the performance the forged
    // geometry actually produces so it clears the engine gates and has to be
    // caught by the delegation rule itself.
    let solved = odd_propulsion::solve_engine(&engine, 0.0).unwrap();
    forged.engine = Some(engine);
    forged.thrust_n = solved.thrust_n;
    forged.specific_impulse_s = solved.specific_impulse_s;
    forged.sign(&pl).unwrap();
    assert!(
        matches!(
            verify_record(&forged),
            Err(FdrError::DelegationSpecMismatch("propulsion"))
        ),
        "a delegated engine must be the chief's reference engine"
    );
    println!("rejected a re-signed non-chief engine claim");
}
