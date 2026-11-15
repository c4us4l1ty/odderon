//! The portable replay path must verify a real flight with no browser in the
//! process, and must still catch a re-signed forgery.
//!
//! These are the same two gates the wasm bridge runs, executed natively
//! through `odd_verify` alone. If someone later re-implements replay inside
//! the browser bridge, this test keeps the headless path honest.
use odd_contracts::Contract;
use odd_report::{AttemptMode, Command, Fdr, FdrError, SCHEMA_VERSION};
use odd_verify::{FlightSpec, REPORT_LEN};

/// A two-burn Contract 101 rescue, expressed only as recorder fields.
fn rescue_spec() -> FlightSpec {
    let contract = Contract::get(101).unwrap();
    let mu = odd_contracts::EARTH_MU;
    let r0 = odd_contracts::EARTH_EQUATORIAL_RADIUS + 260_000.0;
    let r1 = odd_contracts::EARTH_EQUATORIAL_RADIUS + 550_000.0;
    let a = (r0 + r1) * 0.5;
    let coast = std::f64::consts::PI * libm::sqrt(a * a * a / mu);
    let dv1 = libm::sqrt(mu / r0) * (libm::sqrt(r1 / a) - 1.0);
    let dv2 = libm::sqrt(mu / r1) * (1.0 - libm::sqrt(r0 / a));
    FlightSpec {
        thermal: None,
        contract_id: 101,
        epoch_s: odd_contracts::START_EPOCH,
        duration_us: contract.duration_us,
        initial_mass_kg: contract.initial.mass,
        dry_mass_kg: contract.dry_mass_kg,
        thrust_n: contract.engine.thrust_newtons,
        specific_impulse_s: contract.engine.specific_impulse_seconds,
        initial_state: [
            contract.initial.position[0],
            contract.initial.position[1],
            contract.initial.position[2],
            contract.initial.velocity[0],
            contract.initial.velocity[1],
            contract.initial.velocity[2],
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
    }
}

fn recorder(spec: &FlightSpec) -> Fdr {
    let outcome = odd_verify::replay_flight(spec).expect("the reference rescue must replay");
    let mut fdr = Fdr {
        schema_version: SCHEMA_VERSION,
        thermal_specs: Vec::new(),
        delegation_mask: 0,
        vent_events: Vec::new(),
        contract_id: spec.contract_id,
        attempt_mode: AttemptMode::Ranked,
        callsign: "HEADLESS".into(),
        epoch_s: spec.epoch_s,
        duration_us: spec.duration_us,
        initial_mass_kg: spec.initial_mass_kg,
        dry_mass_kg: spec.dry_mass_kg,
        thrust_n: spec.thrust_n,
        specific_impulse_s: spec.specific_impulse_s,
        engine: Some(Contract::get(101).unwrap().engine_spec),
        initial_state: spec.initial_state,
        node_metric: spec.node_metric,
        prediction_baseline: spec.prediction,
        commands: spec
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
        elo_after: 1204.0,
        public_key: [0; 32],
        signature: [0; 64],
    };
    fdr.sign(&odd_identity::License::from_entropy([7u8; 16]))
        .expect("shape must be valid");
    fdr
}

#[test]
fn headless_verifier_accepts_a_real_flight() {
    let fdr = recorder(&rescue_spec());
    let outcome = odd_verify::verify_record(&fdr).expect("clean record must verify");
    assert_eq!(outcome.report.len(), REPORT_LEN);
    assert!(
        outcome.report[4] > 0.80,
        "rescue score {}",
        outcome.report[4]
    );
    assert_eq!(
        outcome.report[30], 0.0,
        "the reference rescue must not impact"
    );
    // The dossier figure comes out of the same replay, not a second run.
    assert!(outcome.geometry.level3.len() > 100);
    assert_eq!(outcome.geometry.burns.len(), 2);
    // A transport round trip is the same signed flight.
    let parsed = Fdr::from_verify_payload(&fdr.verify_payload()).unwrap();
    assert_eq!(parsed.canonical_bytes(), fdr.canonical_bytes());
    assert!(odd_verify::verify_record(&parsed).is_ok());
}

#[test]
fn headless_verifier_rejects_a_resigned_forgery() {
    let spec = rescue_spec();
    let mut fdr = recorder(&spec);
    // Claim one extra metre of apoapsis, then sign it with the real key: the
    // signature gate passes and only physics can catch it.
    fdr.metrics[1] += 1.0;
    fdr.sign(&odd_identity::License::from_entropy([7u8; 16]))
        .unwrap();
    assert_eq!(fdr.verify_signature(), Ok(()));
    match odd_verify::verify_record(&fdr) {
        Err(FdrError::PhysicsReplayMismatch { field, index }) => {
            assert_eq!(field, "metrics");
            assert_eq!(index, 1);
        }
        Ok(_) => panic!("a re-signed metric forgery must fail physics"),
        Err(other) => panic!("expected a physics mismatch, got {other}"),
    }
}

#[test]
fn headless_verifier_rejects_a_broken_signature() {
    let mut fdr = recorder(&rescue_spec());
    fdr.metrics[1] = f64::from_bits(fdr.metrics[1].to_bits() + 1);
    match odd_verify::verify_record(&fdr) {
        Err(FdrError::SignatureMismatch) => {}
        Ok(_) => panic!("one ULP must break the signature"),
        Err(other) => panic!("expected SignatureMismatch, got {other}"),
    }
}
