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

use odd_contracts::Contract;
use odd_ephem::Ephemeris;
use odd_sim::mission::{LockedMission, PredictionBaseline};
use odd_verify::compute;
use odd_worker::{certification_label, SegmentCheckpoint, ServerError, Store};

mod common;
use common::*;

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

    let mut practice_fdr = fdr.clone();
    practice_fdr.attempt_mode = odd_worker::AttemptMode::Practice;
    practice_fdr.sign(&license).unwrap();
    let second = engine
        .submit(submit_request(&practice_fdr, ticket2), NOW)
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

#[test]
fn retry_cannot_attach_the_old_receipt_to_a_different_record() {
    let mut engine = engine();
    let (fdr, license) = signed_rescue(prediction_a());
    let ticket = commit_for(&mut engine, &fdr, &license);
    let first = engine
        .submit(submit_request(&fdr, ticket.clone()), NOW)
        .unwrap();
    let mut other = fdr.clone();
    // Same design and prediction, different signed metadata; replay still
    // succeeds, but it is not the record this attempt already certified.
    other.callsign = "DIFFERENT".into();
    other.sign(&license).unwrap();
    assert!(odd_verify::verify_record(&other).is_ok());
    assert_eq!(
        engine
            .submit(submit_request(&other, ticket.clone()), NOW)
            .unwrap_err(),
        ServerError::AttemptAlreadySubmitted
    );
    assert_eq!(
        engine
            .submit(submit_request(&fdr, ticket), NOW)
            .unwrap()
            .receipt,
        first.receipt
    );
}

#[test]
fn ticket_must_be_stored_and_record_mode_must_match() {
    let mut original = engine();
    let (fdr, license) = signed_rescue(prediction_a());
    let ticket = commit_for(&mut original, &fdr, &license);
    let mut other = engine(); // same key, but no committed ticket in this database
    assert_eq!(
        other
            .submit(submit_request(&fdr, ticket.clone()), NOW)
            .unwrap_err(),
        ServerError::CommitmentMissing
    );
    let mut wrong = fdr.clone();
    wrong.attempt_mode = odd_worker::AttemptMode::Practice;
    wrong.sign(&license).unwrap();
    assert_eq!(
        original
            .submit(submit_request(&wrong, ticket), NOW)
            .unwrap_err(),
        ServerError::AttemptModeMismatch
    );
}
