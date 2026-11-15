//! `oracle_fdr_engine_recomputation_anti_cheat`.
//!
//! A record signs the engine *design*. The verifier solves that design and
//! refuses any recorded performance the design cannot produce, even when the
//! signature is perfectly valid because the player signed the lie themselves.
use odd_contracts::Contract;
use odd_propulsion::{EngineSpec, Propellant};
use odd_report::{AttemptMode, Command, Fdr, FdrError, LEGACY_SCHEMA_VERSION, SCHEMA_VERSION};
use odd_verify::FlightSpec;

/// A rescue flown with a player-designed methalox engine instead of the
/// contract's storable baseline.
fn custom() -> (EngineSpec, FlightSpec) {
    let contract = Contract::get(101).unwrap();
    let spec = EngineSpec {
        propellant: Propellant::LoxLch4,
        of_ratio: 3.4,
        p_c_pa: 6.0e6,
        r_throat_m: 0.012,
        r_exit_m: 0.012 * libm::sqrt(70.0),
        length_fraction: 0.85,
    };
    let vehicle = contract.vehicle(spec).unwrap();
    let mu = odd_contracts::EARTH_MU;
    let r0 = odd_contracts::EARTH_EQUATORIAL_RADIUS + 260_000.0;
    let r1 = odd_contracts::EARTH_EQUATORIAL_RADIUS + 550_000.0;
    let a = (r0 + r1) * 0.5;
    let coast = std::f64::consts::PI * libm::sqrt(a * a * a / mu);
    let dv1 = libm::sqrt(mu / r0) * (libm::sqrt(r1 / a) - 1.0);
    let dv2 = libm::sqrt(mu / r1) * (1.0 - libm::sqrt(r0 / a));
    (
        spec,
        FlightSpec {
            thermal: None,
            contract_id: 101,
            epoch_s: odd_contracts::START_EPOCH,
            duration_us: contract.duration_us,
            initial_mass_kg: vehicle.initial_mass_kg,
            dry_mass_kg: vehicle.dry_mass_kg,
            thrust_n: vehicle.engine.thrust_newtons,
            specific_impulse_s: vehicle.engine.specific_impulse_seconds,
            initial_state: [
                contract.initial.position[0],
                0.0,
                0.0,
                0.0,
                libm::sqrt(mu / r0),
                0.0,
            ],
            node_metric: false,
            prediction: [180.0, 550_000.0, 260_000.0, 120.0, 10_800.0, f64::NAN],
            commands: vec![
                (200_000_000, [dv1, 0.0, 0.0], None),
                (
                    ((200.0 + coast) * 1e6).round() as i64,
                    [dv2, 0.0, 0.0],
                    None,
                ),
            ],
        },
    )
}

fn record(spec: EngineSpec, flight: &FlightSpec) -> Fdr {
    let outcome = odd_verify::replay_flight(flight).expect("custom-engine flight replays");
    let mut fdr = Fdr {
        schema_version: SCHEMA_VERSION,
        thermal_specs: Vec::new(),
        delegation_mask: 0,
        vent_events: Vec::new(),
        contract_id: flight.contract_id,
        attempt_mode: AttemptMode::Ranked,
        callsign: "ENGINE GATE".into(),
        epoch_s: flight.epoch_s,
        duration_us: flight.duration_us,
        initial_mass_kg: flight.initial_mass_kg,
        dry_mass_kg: flight.dry_mass_kg,
        thrust_n: flight.thrust_n,
        specific_impulse_s: flight.specific_impulse_s,
        engine: Some(spec),
        initial_state: flight.initial_state,
        node_metric: flight.node_metric,
        prediction_baseline: flight.prediction,
        commands: flight
            .commands
            .iter()
            .map(|(time_us, tnw, guidance)| Command {
                time_us: *time_us,
                delta_v_tnw: *tnw,
                guidance: *guidance,
            })
            .collect(),
        metrics: outcome.report,
        final_state_hash: outcome.final_state_hash,
        elo_before: 1200.0,
        elo_after: 1203.0,
        public_key: [0; 32],
        signature: [0; 64],
    };
    sign(&mut fdr);
    fdr
}

fn sign(fdr: &mut Fdr) {
    fdr.sign(&odd_identity::License::from_entropy([31u8; 16]))
        .expect("record shape must be valid");
}

/// `oracle_fdr_engine_recomputation_anti_cheat`: a valid signed record with a
/// player-designed engine verifies through the same portable replay a Worker
/// or CLI would run.
#[test]
fn oracle_fdr_engine_recomputation_anti_cheat() {
    let (spec, flight) = custom();
    let fdr = record(spec, &flight);
    let solved = odd_verify::certify_engine(&fdr)
        .unwrap()
        .expect("engine certified");
    println!(
        "custom methalox: {:.1} N, {:.2} s, {:.2} kg engine, stage {:.2} kg",
        solved.thrust_n, solved.specific_impulse_s, solved.dry_mass_kg, flight.initial_mass_kg
    );
    assert!(odd_verify::verify_record(&fdr).is_ok());
    // Transport round trip keeps the design bit for bit.
    let parsed = Fdr::from_verify_payload(&fdr.verify_payload()).unwrap();
    assert_eq!(parsed.engine, Some(spec));
    assert!(odd_verify::verify_record(&parsed).is_ok());
}

#[test]
fn claiming_more_isp_than_the_design_produces_is_rejected() {
    let (spec, flight) = custom();
    let mut fdr = record(spec, &flight);
    fdr.specific_impulse_s += 5.0;
    sign(&mut fdr);
    assert_eq!(fdr.verify_signature(), Ok(()));
    assert_eq!(
        odd_verify::verify_record(&fdr).err(),
        Some(FdrError::EngineMismatch("specific impulse"))
    );
}

#[test]
fn claiming_more_thrust_or_a_lighter_engine_is_rejected() {
    let (spec, flight) = custom();
    let mut fdr = record(spec, &flight);
    fdr.thrust_n *= 1.05;
    sign(&mut fdr);
    assert_eq!(
        odd_verify::verify_record(&fdr).err(),
        Some(FdrError::EngineMismatch("thrust"))
    );

    let mut fdr = record(spec, &flight);
    fdr.dry_mass_kg -= 50.0;
    sign(&mut fdr);
    assert_eq!(
        odd_verify::verify_record(&fdr).err(),
        Some(FdrError::EngineMismatch("dry mass"))
    );

    let mut fdr = record(spec, &flight);
    fdr.initial_mass_kg += 5_000.0;
    sign(&mut fdr);
    assert_eq!(
        odd_verify::verify_record(&fdr).err(),
        Some(FdrError::EngineMismatch("propellant load"))
    );
}

#[test]
fn swapping_in_a_better_engine_after_the_flight_is_rejected() {
    let (spec, flight) = custom();
    let mut fdr = record(spec, &flight);
    // Same claimed performance, different signed design: a bigger throat.
    fdr.engine = Some(EngineSpec {
        r_throat_m: spec.r_throat_m * 1.2,
        r_exit_m: spec.r_exit_m * 1.2,
        ..spec
    });
    sign(&mut fdr);
    assert_eq!(fdr.verify_signature(), Ok(()));
    assert_eq!(
        odd_verify::verify_record(&fdr).err(),
        Some(FdrError::EngineMismatch("thrust"))
    );
}

#[test]
fn legacy_records_still_verify_but_are_labelled() {
    let contract = Contract::get(101).unwrap();
    let (_, mut flight) = custom();
    // Fly the baseline stage the way schema 1 did: bare thrust and Isp.
    flight.initial_mass_kg = contract.initial.mass;
    flight.dry_mass_kg = contract.dry_mass_kg;
    flight.thrust_n = contract.engine.thrust_newtons;
    flight.specific_impulse_s = contract.engine.specific_impulse_seconds;
    let mut fdr = record(contract.engine_spec, &flight);
    fdr.schema_version = LEGACY_SCHEMA_VERSION;
    fdr.engine = None;
    sign(&mut fdr);
    assert!(fdr.is_legacy());
    assert!(odd_verify::certify_engine(&fdr).unwrap().is_none());
    assert!(odd_verify::verify_record(&fdr).is_ok());
    let parsed = Fdr::from_verify_payload(&fdr.verify_payload()).unwrap();
    assert!(parsed.is_legacy());
    assert_eq!(parsed.engine, None);
    assert!(odd_verify::verify_record(&parsed).is_ok());
    // A legacy document cannot be upgraded into a certified one by
    // relabelling: schema 2 requires a design, so it cannot even be signed.
    let mut upgraded = parsed.clone();
    upgraded.schema_version = SCHEMA_VERSION;
    assert!(upgraded
        .sign(&odd_identity::License::from_entropy([31u8; 16]))
        .is_err());
}

/// A ranked record must start where the contract says it starts. Otherwise a
/// player can sign a flight that begins already on the target orbit, do
/// nothing, and collect a perfect rescue score with a valid signature and a
/// perfectly honest replay.
#[test]
fn a_ranked_record_cannot_start_somewhere_easier() {
    let (spec, mut flight) = custom();
    let mu = odd_contracts::EARTH_MU;
    let target = odd_contracts::EARTH_EQUATORIAL_RADIUS + 550_000.0;
    // Start on the target orbit, coast, and claim the rescue was achieved.
    flight.initial_state = [target, 0.0, 0.0, 0.0, libm::sqrt(mu / target), 0.0];
    flight.commands = vec![(200_000_000, [0.001, 0.0, 0.0], None)];
    flight.prediction = [0.001, 550_000.0, 550_000.0, 180.0, 10_800.0, f64::NAN];
    let fdr = record(spec, &flight);
    fdr.verify_signature()
        .expect("the cheater signed it themselves");
    let err = odd_verify::verify_record(&fdr).err();
    println!("forged-start verdict: {err:?}");
    assert!(
        matches!(err, Some(FdrError::ContractMismatch("initial state"))),
        "starting off-contract must be refused, got {err:?}"
    );
}

/// Every framing condition the contract fixes must be enforced, not just the
/// starting orbit: a longer flight, a different epoch or the easier scoring
/// metric are all ways to make a contract cheaper than it is.
#[test]
fn every_contract_framing_condition_is_enforced() {
    let (spec, flight) = custom();
    for (field, mutate) in [("epoch", 0usize), ("duration", 1), ("objective metric", 2)] {
        let mut forged = flight.clone();
        match mutate {
            0 => forged.epoch_s += 86_400,
            1 => forged.duration_us *= 2,
            _ => forged.node_metric = !forged.node_metric,
        }
        // Rebuild metrics honestly so only the framing condition is wrong.
        let Some(outcome) = odd_verify::replay_flight(&forged) else {
            continue; // An illegal frame is refused at compile time, also fine.
        };
        let mut fdr = record(spec, &flight);
        fdr.epoch_s = forged.epoch_s;
        fdr.duration_us = forged.duration_us;
        fdr.node_metric = forged.node_metric;
        fdr.metrics = outcome.report;
        fdr.final_state_hash = outcome.final_state_hash;
        sign(&mut fdr);
        fdr.verify_signature().expect("self-signed");
        assert!(
            matches!(odd_verify::verify_record(&fdr).err(), Some(FdrError::ContractMismatch(f)) if f == field),
            "{field} must be refused, got {:?}",
            odd_verify::verify_record(&fdr).err()
        );
    }
}
