//! WP 1.9 server anti-cheat oracles. These drive the pure `AuthorityEngine`
//! over the in-memory store — the same engine the local HTTP/SQLite and the
//! Cloudflare D1/R2 adapters wrap — so every gate runs in `./scripts/gate.sh`
//! with no cloud credentials.
//!
//! * `oracle_server_anti_hindsight_commitment`
//! * `oracle_server_first_attempt_only_ranked`
//! * `oracle_chunked_segment_verification_parity`
//! * `oracle_devtools_localstorage_tamper_defeated` (receipt layer; the
//!   browser layer is added with the Slice 3 UI)

use odd_contracts::{Contract, EARTH_EQUATORIAL_RADIUS, EARTH_MU};
use odd_ephem::Ephemeris;
use odd_identity::License;
use odd_report::{Fdr, SCHEMA_VERSION};
use odd_sim::mission::{LockedMission, PredictionBaseline};
use odd_sim::planner::{ManeuverNode, Plan};
use odd_verify::{compute, FlightSettings};
use odd_worker::{
    certification_label, AuthorityEngine, CommitRequest, MemoryStore, SegmentCheckpoint,
    ServerError, ServerKey, Store, SubmitRequest,
};

const NOW: i64 = 1_700_000_000;

fn server_key() -> ServerKey {
    ServerKey::from_seed([0x5e; 32])
}

fn engine() -> AuthorityEngine<MemoryStore> {
    AuthorityEngine::new(MemoryStore::new(), server_key())
}

/// The reference two-impulse rescue, compiled for contract 101.
fn rescue_plan(contract: &Contract) -> (Plan, [ManeuverNode; 2]) {
    let r0 = EARTH_EQUATORIAL_RADIUS + 260_000.0;
    let r1 = EARTH_EQUATORIAL_RADIUS + 550_000.0;
    let a = (r0 + r1) * 0.5;
    let coast = std::f64::consts::PI * libm::sqrt(a * a * a / EARTH_MU);
    let v0 = libm::sqrt(EARTH_MU / r0);
    let v1 = libm::sqrt(EARTH_MU / r1);
    let nodes = [
        ManeuverNode {
            epoch_us: 200_000_000,
            tnw_m_s: [v0 * (libm::sqrt(a / r0) - 1.0), 0.0, 0.0],
        },
        ManeuverNode {
            epoch_us: ((200.0 + coast) * 1e6) as i64,
            tnw_m_s: [v1 * (1.0 - libm::sqrt(r0 / a)), 0.0, 0.0],
        },
    ];
    let plan = Plan::compile(
        contract.initial,
        EARTH_MU,
        contract.duration_us,
        contract.dry_mass_kg,
        contract.engine,
        &nodes,
    )
    .expect("the reference rescue compiles");
    (plan, nodes)
}

fn prediction_from_nodes(nodes: &[ManeuverNode; 2]) -> [f64; 6] {
    [
        nodes[0].tnw_m_s[0] + nodes[1].tnw_m_s[0],
        550_000.0,
        550_000.0,
        120.0,
        10_800.0,
        f64::NAN,
    ]
}

fn settings(contract: &Contract) -> FlightSettings {
    FlightSettings {
        contract_id: contract.id,
        epoch: odd_contracts::START_EPOCH,
        initial: contract.initial,
        duration_us: contract.duration_us,
        dry_mass: contract.dry_mass_kg,
        engine: contract.engine,
        node_metric: false,
        thermal: None,
        delegation: odd_contracts::DelegationMask::MANUAL,
    }
}

/// Fly and sign a rescue record whose locked prediction is `pred`. The
/// metrics are taken from the same locked replay the verifier will run, so
/// the record verifies on its own — a commitment failure is therefore
/// isolated from any replay failure.
fn signed_rescue(pred: [f64; 6]) -> (Fdr, License) {
    let eph = Ephemeris::bundled().unwrap();
    let contract = Contract::get(101).unwrap();
    let (plan, _nodes) = rescue_plan(&contract);
    let prediction = PredictionBaseline {
        delta_v_m_s: pred[0],
        apoapsis_altitude_m: pred[1],
        periapsis_altitude_m: pred[2],
        remaining_propellant_kg: pred[3],
        time_of_flight_s: pred[4],
        target_miss_distance_m: if pred[5].is_finite() && pred[5] >= 0.0 {
            Some(pred[5])
        } else {
            None
        },
    };
    let locked = LockedMission::new(plan.clone(), prediction, contract.dry_mass_kg).unwrap();
    let outcome = compute(&settings(&contract), &plan, Some(&locked), &eph).unwrap();
    let commands = _nodes
        .iter()
        .map(|n| odd_report::fdr::Command {
            time_us: n.epoch_us,
            delta_v_tnw: n.tnw_m_s,
            guidance: None,
        })
        .collect();
    let mut fdr = Fdr {
        schema_version: SCHEMA_VERSION,
        contract_id: contract.id,
        attempt_mode: odd_report::AttemptMode::Ranked,
        callsign: "SERVER ORACLE".into(),
        epoch_s: odd_contracts::START_EPOCH,
        duration_us: contract.duration_us,
        initial_mass_kg: contract.initial.mass,
        dry_mass_kg: contract.dry_mass_kg,
        thrust_n: contract.engine.thrust_newtons,
        specific_impulse_s: contract.engine.specific_impulse_seconds,
        engine: Some(contract.engine_spec),
        initial_state: [
            contract.initial.position[0],
            contract.initial.position[1],
            contract.initial.position[2],
            contract.initial.velocity[0],
            contract.initial.velocity[1],
            contract.initial.velocity[2],
        ],
        node_metric: false,
        prediction_baseline: pred,
        commands,
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
        thermal_specs: Vec::new(),
        delegation_mask: 0,
        boil_off_kg: [0.0, 0.0],
        cryocooler_energy_j: 0.0,
        vent_events: Vec::new(),
        public_key: [0; 32],
        signature: [0; 64],
    };
    let license = License::from_entropy([21u8; 16]);
    fdr.sign(&license).unwrap();
    (fdr, license)
}

/// Commit to a record's design and prediction, signing as the pilot.
fn commit_for(
    engine: &mut AuthorityEngine<MemoryStore>,
    fdr: &Fdr,
    license: &License,
) -> odd_worker::CommitTicket {
    let mut req = CommitRequest {
        contract_id: fdr.contract_id,
        pubkey: license.public_key(),
        design_hash: odd_worker::commitment::design_hash(fdr),
        prediction_hash: odd_worker::commitment::prediction_hash(fdr),
        client_sig: [0u8; 64],
    };
    req.client_sig = license.sign(&req.canonical_bytes());
    engine.commit(req, NOW).expect("the commit is accepted")
}

fn submit_request(fdr: &Fdr, ticket: odd_worker::CommitTicket) -> SubmitRequest {
    SubmitRequest {
        fdr_payload: fdr.verify_payload(),
        ticket,
        segments: None,
    }
}

#[test]
fn oracle_server_anti_hindsight_commitment() {
    let mut engine = engine();
    let contract = Contract::get(101).unwrap();
    let (plan, nodes) = rescue_plan(&contract);
    let _ = plan;
    let pred_a = prediction_from_nodes(&nodes);

    // The pilot commits to prediction A before flying.
    let (fdr_a, license) = signed_rescue(pred_a);
    let ticket = commit_for(&mut engine, &fdr_a, &license);

    // Honest submit of A is accepted and certified.
    let honest = engine
        .submit(submit_request(&fdr_a, ticket.clone()), NOW)
        .expect("the honest flight is accepted");
    assert!(honest.accepted);
    let receipt = honest.receipt.expect("a receipt is issued");
    assert!(receipt.is_certified_ranked(&engine.server_pk()));

    // Hindsight: rewrite the prediction to match a different claim and
    // re-sign. The record still verifies on its own physics, but its
    // prediction no longer hashes to the commitment.
    let mut pred_b = pred_a;
    pred_b[1] = 600_000.0; // a different apoapsis claim
    let (fdr_b, license_b) = signed_rescue(pred_b);
    assert_eq!(license_b.public_key(), license.public_key());
    assert!(
        odd_verify::verify_record(&fdr_b).is_ok(),
        "B verifies alone"
    );
    let err = engine
        .submit(submit_request(&fdr_b, ticket), NOW)
        .expect_err("the re-signed prediction must be refused");
    assert_eq!(err, ServerError::PredictionCommitmentMismatch);
}

#[test]
fn oracle_server_first_attempt_only_ranked() {
    let mut engine = engine();
    let contract = Contract::get(101).unwrap();
    let (_plan, nodes) = rescue_plan(&contract);
    let pred = prediction_from_nodes(&nodes);
    let (fdr, license) = signed_rescue(pred);

    // First commit: attempt 1, Ranked.
    let ticket1 = commit_for(&mut engine, &fdr, &license);
    assert_eq!(ticket1.attempt_num, 1);
    assert_eq!(ticket1.mode, odd_worker::AttemptMode::Ranked);

    // A commit→refresh→commit is just a second commit: attempt 2, Practice.
    let ticket2 = commit_for(&mut engine, &fdr, &license);
    assert_eq!(ticket2.attempt_num, 2);
    assert_eq!(ticket2.mode, odd_worker::AttemptMode::Practice);

    let first = engine
        .submit(submit_request(&fdr, ticket1.clone()), NOW)
        .expect("first submit accepted");
    assert_eq!(first.mode, odd_worker::AttemptMode::Ranked);
    assert!(first.elo_after != first.elo_before, "ranked moves Elo");
    assert!(first
        .receipt
        .as_ref()
        .unwrap()
        .is_certified_ranked(&engine.server_pk()));

    let second = engine
        .submit(submit_request(&fdr, ticket2), NOW)
        .expect("second submit accepted");
    assert_eq!(second.mode, odd_worker::AttemptMode::Practice);
    assert_eq!(
        second.elo_after, second.elo_before,
        "practice never moves Elo"
    );
    assert_eq!(second.elo_before, first.elo_after, "rating carries forward");
    let receipt2 = second.receipt.unwrap();
    assert!(receipt2.verify(&engine.server_pk()));
    assert!(!receipt2.is_certified_ranked(&engine.server_pk()));
    assert_eq!(
        certification_label(Some(&receipt2), &engine.server_pk()),
        "SERVER-VERIFIED PRACTICE"
    );

    // Re-submitting the ranked attempt is idempotent: same receipt, no
    // second Elo move.
    let again = engine
        .submit(submit_request(&fdr, ticket1), NOW)
        .expect("re-submit accepted");
    assert_eq!(again.elo_after, first.elo_after);
    assert_eq!(again.fdr_hash, first.fdr_hash);
    assert_eq!(again.receipt, first.receipt);
}

#[test]
fn oracle_chunked_segment_verification_parity() {
    let mut engine = engine();
    let contract = Contract::get(101).unwrap();
    let (plan, nodes) = rescue_plan(&contract);
    let pred = prediction_from_nodes(&nodes);
    let (fdr, license) = signed_rescue(pred);
    let ticket = commit_for(&mut engine, &fdr, &license);

    // Build an honest checkpoint chain by sampling the authoritative flight.
    let eph = Ephemeris::bundled().unwrap();
    let prediction = PredictionBaseline {
        delta_v_m_s: pred[0],
        apoapsis_altitude_m: pred[1],
        periapsis_altitude_m: pred[2],
        remaining_propellant_kg: pred[3],
        time_of_flight_s: pred[4],
        target_miss_distance_m: None,
    };
    let locked = LockedMission::new(plan.clone(), prediction, contract.dry_mass_kg).unwrap();
    let outcome = compute(&settings(&contract), &plan, Some(&locked), &eph).unwrap();
    let eval_us = (outcome.report[29] * 1e6).round() as i64;
    let times = [eval_us / 3, 2 * eval_us / 3, eval_us];
    let samples: Vec<(i64, [f64; 7], f64, [f64; 2])> = times
        .iter()
        .map(|&t_us| {
            let s = outcome.trajectory.sample(t_us as f64 / 1e6).unwrap();
            (
                t_us,
                [
                    s.position[0],
                    s.position[1],
                    s.position[2],
                    s.velocity[0],
                    s.velocity[1],
                    s.velocity[2],
                    s.mass,
                ],
                s.mass - contract.dry_mass_kg,
                [0.0, 0.0],
            )
        })
        .collect();
    let chain = odd_worker::segment::build_chain(&samples);

    // Chunked and full submissions must agree on hash, verdict and receipt.
    let full = engine
        .submit(submit_request(&fdr, ticket.clone()), NOW)
        .expect("full submit accepted");
    let mut chunked_req = submit_request(&fdr, ticket.clone());
    chunked_req.segments = Some(chain.clone());
    let chunked = engine
        .submit(chunked_req, NOW)
        .expect("chunked submit accepted");
    assert_eq!(chunked.fdr_hash, full.fdr_hash);
    assert_eq!(chunked.score_s.to_bits(), full.score_s.to_bits());
    assert_eq!(chunked.mode, full.mode);
    assert_eq!(chunked.receipt, full.receipt);

    // A one-bit edit to an intermediate checkpoint fails at that boundary,
    // even though the terminal state and the downstream hashes are untouched.
    let mut tampered: Vec<SegmentCheckpoint> = chain.clone();
    let bit = tampered[1].state_vector[0].to_bits() ^ 1;
    tampered[1].state_vector[0] = f64::from_bits(bit);
    let mut bad_req = submit_request(&fdr, ticket);
    bad_req.segments = Some(tampered);
    let err = engine
        .submit(bad_req, NOW)
        .expect_err("a tampered segment must be refused");
    assert_eq!(err, ServerError::SegmentMismatch(1));
}

#[test]
fn oracle_devtools_localstorage_tamper_defeated() {
    let mut engine = engine();
    let contract = Contract::get(101).unwrap();
    let (_plan, nodes) = rescue_plan(&contract);
    let pred = prediction_from_nodes(&nodes);
    let (fdr, license) = signed_rescue(pred);
    let ticket = commit_for(&mut engine, &fdr, &license);
    let response = engine
        .submit(submit_request(&fdr, ticket), NOW)
        .expect("submit accepted");
    let receipt = response.receipt.expect("a receipt is issued");
    let pk = engine.server_pk();

    // The true, server-certified rating reads as certified.
    assert_eq!(
        certification_label(Some(&receipt), &pk),
        "★ CERTIFIED SERVER-RANKED RECEIPT"
    );
    assert!(receipt.payload.elo_after() < 2999.0);

    // A DevTools edit writes 2999 into localStorage. With no receipt for that
    // number, the label must be UNVERIFIED — the stored value is never an
    // input to certification.
    assert_eq!(
        certification_label(None, &pk),
        "UNVERIFIED LOCAL / PRACTICE"
    );

    // forging a receipt payload that claims 2999 without the server key
    // cannot produce a valid signature.
    let mut forged = receipt.clone();
    forged.payload.elo_after_bits = 2999.0f64.to_bits();
    assert!(!forged.verify(&pk));
    assert_eq!(
        certification_label(Some(&forged), &pk),
        "UNVERIFIED LOCAL / PRACTICE"
    );

    // On sync the authority's stored rating is the truth, not 2999.
    let stored = engine.store().load_player(&license.public_key()).unwrap();
    assert_eq!(stored.elo.to_bits(), response.elo_after.to_bits());
    assert!(stored.elo < 2999.0);
}
