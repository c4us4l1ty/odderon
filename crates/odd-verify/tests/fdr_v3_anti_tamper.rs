//! WP 1.6/1.7 acceptance gate: `oracle_fdr_v3_anti_tamper`.
//!
//! Two independent attacks must fail:
//!
//! 1. **Edit the thermal spec** (fewer MLI layers, a bigger blanket emittance,
//!    a smaller cooler) and the Ed25519 signature breaks — the spec is inside
//!    the signed bytes.
//! 2. **Re-sign a lie**: keep a valid signature but claim a vent history the
//!    hardware does not produce. The replay recomputes boil-off from the
//!    signed spec and rejects the mismatch. This is the no-magic-lookup law
//!    applied to tanks: the client never gets to assert its own loss.

use odd_contracts::{Contract, ThermalDesign};
use odd_ephem::Ephemeris;
use odd_identity::License;
use odd_report::{Fdr, FdrError, StageThermalSpec, VentEvent, SCHEMA_VERSION};
use odd_sim::planner::Plan;
use odd_verify::{compute, verify_record, FlightSettings, ThermalInputs};

/// The contract fixes its own start epoch; a record cannot choose another.
const EPOCH: i64 = odd_contracts::START_EPOCH;

/// A named edit to the signed thermal spec.
type SpecEdit = (&'static str, Box<dyn Fn(&mut StageThermalSpec)>);

fn thermal_spec(design: &ThermalDesign, contract: &Contract) -> StageThermalSpec {
    let load = contract.initial.mass - contract.dry_mass_kg;
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

/// A signed v3 record of a real cryogenic coast, with the vent history the
/// hardware actually produced.
fn signed_tug(design: &ThermalDesign) -> (Fdr, odd_verify::FlightOutcome) {
    let eph = Ephemeris::bundled().unwrap();
    let tug = Contract::get(103).unwrap();
    let load = tug.initial.mass - tug.dry_mass_kg;
    let inputs = ThermalInputs {
        engine_spec: tug.engine_spec,
        design: *design,
        load_kg: load,
    };
    let settings = FlightSettings {
        contract_id: tug.id,
        epoch: EPOCH,
        initial: tug.initial,
        duration_us: tug.duration_us,
        dry_mass: tug.dry_mass_kg,
        engine: tug.engine,
        node_metric: false,
        delegation: odd_contracts::DelegationMask::MANUAL,
        thermal: Some(inputs),
    };
    let plan = Plan::compile(
        tug.initial,
        odd_contracts::EARTH_MU,
        tug.duration_us,
        tug.dry_mass_kg,
        tug.engine,
        &[],
    )
    .unwrap();
    let outcome = compute(&settings, &plan, None, &eph).expect("the coast evaluates");
    let solution = odd_propulsion::solve_engine(&tug.engine_spec, 0.0).unwrap();
    let mut fdr = Fdr {
        schema_version: SCHEMA_VERSION,
        contract_id: tug.id,
        attempt_mode: odd_report::AttemptMode::Ranked,
        callsign: "CRYO ORACLE".into(),
        epoch_s: EPOCH,
        duration_us: tug.duration_us,
        initial_mass_kg: tug.initial.mass,
        dry_mass_kg: tug.dry_mass_kg,
        thrust_n: solution.thrust_n,
        specific_impulse_s: solution.specific_impulse_s,
        engine: Some(tug.engine_spec),
        initial_state: [
            tug.initial.position[0],
            tug.initial.position[1],
            tug.initial.position[2],
            tug.initial.velocity[0],
            tug.initial.velocity[1],
            tug.initial.velocity[2],
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
        thermal_specs: vec![thermal_spec(design, &tug)],
        delegation_mask: 0,
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
        public_key: [0; 32],
        signature: [0; 64],
    };
    fdr.sign(&License::from_entropy([9u8; 16])).unwrap();
    (fdr, outcome)
}

#[test]
fn oracle_fdr_v3_anti_tamper() {
    let design = ThermalDesign::baseline();
    let (honest, outcome) = signed_tug(&design);
    assert_eq!(honest.schema_version, 3);
    assert_eq!(honest.thermal_specs.len(), 1);
    assert!(!honest.vent_events.is_empty(), "the tug must vent");
    println!(
        "honest v3: {} vent events, {:.3} kg lost, digest {}",
        honest.vent_events.len(),
        outcome.thermal.lost_kg,
        odd_report::hex::encode(&honest.digest())
    );

    // --- the honest record verifies, signature AND physics ----------------
    let replayed = verify_record(&honest).expect("an honest v3 record must verify");
    assert_eq!(replayed.vent_events, honest.vent_events);

    // --- attack 1: edit the signed thermal spec ---------------------------
    // Each of these is a physical downgrade or upgrade a player might want to
    // fake. Every one must break the signature.
    let edits: [SpecEdit; 6] = [
        ("fewer MLI layers", Box::new(|s| s.mli_layers = 4)),
        ("better emittance", Box::new(|s| s.epsilon_eff *= 0.5)),
        (
            "colder strut mount",
            Box::new(|s| s.strut_conductance_w_k *= 0.1),
        ),
        (
            "free cryocooler",
            Box::new(|s| {
                s.cryocooler_w = 500.0;
                s.radiator_area_m2 = 20.0;
            }),
        ),
        (
            "higher relief setting",
            Box::new(|s| s.relief_pressure_pa *= 2.0),
        ),
        ("bigger tanks", Box::new(|s| s.tank_volumes_m3[1] *= 1.5)),
    ];
    for (label, edit) in edits {
        let mut tampered = honest.clone();
        edit(&mut tampered.thermal_specs[0]);
        assert_eq!(
            tampered.verify_signature(),
            Err(FdrError::SignatureMismatch),
            "{label} must break the signature"
        );
        assert_ne!(tampered.digest(), honest.digest(), "{label}");
        println!("rejected thermal-spec edit: {label}");
    }

    // --- the delegation mask is signed too --------------------------------
    let mut promoted = honest.clone();
    // Claim the flight was fully manual when a chief sized it (or vice
    // versa): either way the provenance is inside the signed bytes.
    promoted.delegation_mask = 0b0111;
    assert_eq!(
        promoted.verify_signature(),
        Err(FdrError::SignatureMismatch),
        "the delegation bitmask must be signed"
    );

    // --- attack 2: re-sign a fabricated vent history ----------------------
    // The attacker holds a valid license, so the signature is genuine. Only
    // the physics replay can catch this.
    let attacker = License::from_entropy([7u8; 16]);
    let mut fewer = honest.clone();
    fewer.vent_events.pop();
    fewer.sign(&attacker).expect("the attacker can sign");
    assert_eq!(
        fewer.verify_signature(),
        Ok(()),
        "the forgery is well-signed"
    );
    assert!(
        matches!(
            verify_record(&fewer),
            Err(FdrError::PhysicsReplayMismatch {
                field: "vent_events.len",
                ..
            })
        ),
        "a dropped vent event must fail the physics replay, got {:?}",
        verify_record(&fewer).err()
    );

    let mut shifted = honest.clone();
    // Shift the *last* opening: both tanks start at relief, so moving the
    // first would break ordering rather than test the replay comparison.
    shifted.vent_events.last_mut().unwrap().time_us += 1;
    shifted.sign(&attacker).unwrap();
    assert_eq!(shifted.verify_signature(), Ok(()));
    assert!(
        matches!(
            verify_record(&shifted),
            Err(FdrError::PhysicsReplayMismatch {
                field: "vent_events.time_us",
                ..
            })
        ),
        "a one-microsecond shift must fail the replay"
    );

    let mut cooled = honest.clone();
    cooled.vent_events[0].temperature_k -= 1.0;
    cooled.sign(&attacker).unwrap();
    assert_eq!(cooled.verify_signature(), Ok(()));
    assert!(
        matches!(
            verify_record(&cooled),
            Err(FdrError::PhysicsReplayMismatch {
                field: "vent_events.state",
                ..
            })
        ),
        "a fabricated vent temperature must fail the replay"
    );
    println!("rejected three well-signed vent-history forgeries");

    // --- attack 3: re-sign a spec that does not match the claimed history --
    // Downgrade the blanket, keep the old (smaller) loss, re-sign. The
    // signature is valid but the hardware now boils off more.
    let mut downgraded = honest.clone();
    downgraded.thermal_specs[0].mli_layers = 4;
    downgraded.thermal_specs[0].epsilon_eff = odd_systems::ideal_epsilon_for_layers(4) * 4.0;
    downgraded.sign(&attacker).unwrap();
    assert_eq!(downgraded.verify_signature(), Ok(()));
    let verdict = verify_record(&downgraded);
    // A thinner blanket is lighter hardware, so the claimed dry mass no
    // longer matches the signed fit-out: the mass gate catches it before the
    // replay even runs. Either rejection is a refusal; both are asserted so
    // a future change cannot quietly let the downgrade through.
    assert!(
        matches!(
            verdict,
            Err(FdrError::PhysicsReplayMismatch { .. }) | Err(FdrError::EngineMismatch("dry mass"))
        ),
        "a re-signed downgrade must be rejected, got {:?}",
        verdict.err()
    );
    println!(
        "rejected a re-signed blanket downgrade: {:?}",
        verdict.err()
    );
}

/// A v3 record must survive its own transport and still verify. The JSON is
/// only a rendering; the canonical bytes are what is signed.
#[test]
fn v3_survives_canonical_and_json_transport() {
    let (honest, _) = signed_tug(&ThermalDesign::baseline());
    let mut bytes = honest.canonical_bytes();
    bytes.extend_from_slice(&honest.public_key);
    bytes.extend_from_slice(&honest.signature);
    let parsed = Fdr::from_canonical_with_signature(&bytes).expect("canonical round trip");
    assert_eq!(parsed.canonical_bytes(), honest.canonical_bytes());
    assert_eq!(parsed.thermal_specs, honest.thermal_specs);
    assert_eq!(parsed.vent_events, honest.vent_events);
    assert_eq!(parsed.delegation_mask, honest.delegation_mask);
    assert_eq!(parsed.verify_signature(), Ok(()));
    verify_record(&parsed).expect("the parsed record replays");

    let json = honest.to_json();
    assert!(json.contains("\"schema_version\":3"));
    assert!(json.contains("\"thermal_specs\":["));
    assert!(json.contains("\"mli_layers\":20"));
    assert!(json.contains("\"vent_events\":["));
    assert!(json.contains("\"delegation_mask\":0"));
    // A pre-v3 parser must not silently accept a v3 domain.
    assert!(bytes.starts_with(b"odderon-flight-data-recorder-v3\0"));
}

/// Malformed v3 blocks are rejected by name, before any expensive replay.
#[test]
fn malformed_v3_blocks_are_rejected_by_name() {
    let (honest, _) = signed_tug(&ThermalDesign::baseline());
    let cases: Vec<(&str, Fdr)> = vec![
        (
            "delegation bits",
            Fdr {
                delegation_mask: 0b1_0000,
                ..honest.clone()
            },
        ),
        (
            "shell material",
            Fdr {
                thermal_specs: vec![StageThermalSpec {
                    tank_shell_material: 9,
                    ..honest.thermal_specs[0]
                }],
                ..honest.clone()
            },
        ),
        (
            "carnot fraction above one",
            Fdr {
                thermal_specs: vec![StageThermalSpec {
                    cryocooler_carnot_fraction: 1.5,
                    ..honest.thermal_specs[0]
                }],
                ..honest.clone()
            },
        ),
        (
            "vent tank id",
            Fdr {
                vent_events: vec![VentEvent {
                    tank: 2,
                    ..honest.vent_events[0]
                }],
                ..honest.clone()
            },
        ),
    ];
    for (label, fdr) in cases {
        assert!(
            matches!(fdr.validate_shape(), Err(FdrError::Malformed(_))),
            "{label} must be rejected by name, got {:?}",
            fdr.validate_shape()
        );
    }
    // An out-of-order vent history is malformed, not merely wrong.
    let mut unordered = honest.clone();
    if unordered.vent_events.len() >= 2 {
        unordered.vent_events.swap(0, 1);
        if unordered.vent_events[0].time_us > unordered.vent_events[1].time_us {
            assert!(matches!(
                unordered.validate_shape(),
                Err(FdrError::Malformed("vent event order or epoch"))
            ));
        }
    }
}
