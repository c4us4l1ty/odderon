//! WP 1.9 server anti-cheat oracles. These drive the pure `AuthorityEngine`
//! over the in-memory store — the same engine the local HTTP/SQLite and the
//! Cloudflare D1/R2 adapters wrap — so every gate runs in `./scripts/gate.sh`
//! with no cloud credentials.
//!
//! * `oracle_server_anti_hindsight_commitment`
//! * `oracle_server_first_attempt_only_ranked`
//! * `oracle_chunked_segment_verification_parity`
//! * `oracle_devtools_localstorage_tamper_defeated` (receipt layer;
//!   `tests/browser/authority.mjs` also exercises the browser layer)

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

    // This fixture is generated offline. The test proves that the ticket
    // binds prediction A; it cannot prove a pilot did not simulate earlier.
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
fn server_rejects_new_legacy_schema_certificates() {
    let mut engine = engine();
    let (mut fdr, license) = signed_rescue(prediction_a());
    fdr.schema_version = odd_report::ENGINE_SCHEMA_VERSION;
    fdr.sign(&license).unwrap();
    assert!(
        odd_verify::verify_record(&fdr).is_ok(),
        "old records remain independently replayable"
    );
    let ticket = commit_for(&mut engine, &fdr, &license);
    let error = engine
        .submit(submit_request(&fdr, ticket), NOW)
        .unwrap_err();
    assert_eq!(
        error,
        ServerError::MalformedFdr("server requires schema 3".into())
    );
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
fn ranked_counter_overflow_fails_without_rating_or_receipt() {
    let (fdr, license) = signed_rescue(prediction_a());
    let mut store = odd_worker::MemoryStore::new();
    store
        .save_player(
            &license.public_key(),
            &odd_worker::PlayerRow {
                elo: 1200.0,
                ranked_count: u32::MAX,
                callsign: None,
            },
        )
        .unwrap();
    let mut engine = odd_worker::AuthorityEngine::new(store, server_key());
    let ticket = commit_for(&mut engine, &fdr, &license);
    assert!(matches!(
        engine.submit(submit_request(&fdr, ticket), NOW),
        Err(ServerError::Storage(_))
    ));
    let player = engine.store().load_player(&license.public_key()).unwrap();
    assert_eq!(player.ranked_count, u32::MAX);
    assert_eq!(player.elo, 1200.0);
    assert!(engine.store().load_fdr(&fdr.digest()).unwrap().is_none());
}

#[test]
fn non_finite_stored_ratings_never_mint_a_receipt() {
    let (fdr, license) = signed_rescue(prediction_a());
    for (player_elo, mission_elo) in [(f64::INFINITY, 800.0), (1200.0, f64::NEG_INFINITY)] {
        let mut store = odd_worker::MemoryStore::new();
        store
            .save_player(
                &license.public_key(),
                &odd_worker::PlayerRow {
                    elo: player_elo,
                    ranked_count: 0,
                    callsign: None,
                },
            )
            .unwrap();
        store.save_mission_elo(101, mission_elo).unwrap();
        let mut engine = odd_worker::AuthorityEngine::new(store, server_key());
        let ticket = commit_for(&mut engine, &fdr, &license);
        assert!(matches!(
            engine.submit(submit_request(&fdr, ticket), NOW),
            Err(ServerError::Storage(_))
        ));
        assert_eq!(
            engine
                .store()
                .load_player(&license.public_key())
                .unwrap()
                .ranked_count,
            0
        );
        assert!(engine.store().load_fdr(&fdr.digest()).unwrap().is_none());
    }
}

#[test]
fn equivalent_payload_encodings_do_not_collide_but_other_keys_do() {
    let mut engine = engine();
    let (mut fdr, license) = signed_rescue(prediction_a());
    // Consume the first ticket, then submit the same signed practice flight
    // twice under different tickets. The parser accepts both a fragment and
    // the bare base64url encoding of the same canonical bytes.
    let _ = commit_for(&mut engine, &fdr, &license);
    fdr.attempt_mode = odd_worker::AttemptMode::Practice;
    fdr.sign(&license).unwrap();
    let ticket2 = commit_for(&mut engine, &fdr, &license);
    let first = engine
        .submit(submit_request(&fdr, ticket2.clone()), NOW)
        .unwrap();
    let ticket3 = commit_for(&mut engine, &fdr, &license);
    let mut alternate = submit_request(&fdr, ticket3);
    alternate.fdr_payload = alternate
        .fdr_payload
        .trim_start_matches("#verify=")
        .to_owned();
    let second = engine
        .submit(alternate, NOW)
        .expect("equivalent encoding is not a hash collision");
    assert_eq!(second.fdr_hash, first.fdr_hash);
    assert_eq!(second.attempt_num, 3);
    let old_retry = engine.submit(submit_request(&fdr, ticket2), NOW).unwrap();
    assert_eq!(old_retry.receipt, first.receipt);

    // The body hash does not include the public key. A different signer of
    // the same body must never overwrite another pilot's public flight.
    let other = odd_identity::License::from_entropy([22u8; 16]);
    let mut copied = fdr.clone();
    copied.sign(&other).unwrap();
    assert_eq!(copied.digest(), fdr.digest());
    let _ = commit_for(&mut engine, &copied, &other);
    let other_ticket = commit_for(&mut engine, &copied, &other);
    assert_eq!(
        engine
            .submit(submit_request(&copied, other_ticket), NOW)
            .unwrap_err(),
        ServerError::FdrHashCollision
    );
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
    let mut bad_req = submit_request(&fdr, ticket.clone());
    bad_req.segments = Some(tampered);
    let err = engine
        .submit(bad_req, NOW)
        .expect_err("a tampered segment must be refused");
    assert_eq!(err, ServerError::SegmentMismatch(1));

    // Hash chains are unkeyed: a cheater can recompute every link after
    // modifying a field. The authority must compare those fields with the
    // replay, not merely check the links.
    for field in ["propellant", "tank temperature", "time"] {
        let mut forged = chain.clone();
        match field {
            "propellant" => forged[1].prop_kg += 1.0,
            "tank temperature" => forged[1].tank_temps[0] = 25.0,
            "time" => forged[1].t_us = forged[0].t_us,
            _ => unreachable!(),
        }
        let mut previous = None;
        for checkpoint in &mut forged {
            checkpoint.cum_hash = odd_worker::segment::link_hash(previous.as_ref(), checkpoint);
            previous = Some(checkpoint.cum_hash);
        }
        let mut request = submit_request(&fdr, ticket.clone());
        request.segments = Some(forged);
        assert_eq!(
            engine.submit(request, NOW).unwrap_err(),
            ServerError::SegmentMismatch(1),
            "{field} must be checked even after rehashing"
        );
    }
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

#[test]
fn server_rejects_committed_forged_cryogenic_proofs() {
    use odd_contracts::ThermalDesign;
    use odd_propulsion::{EngineSpec, Propellant};
    use odd_report::StageThermalSpec;

    // Start with a real locked rescue and fit a cryogenic methalox engine.
    // Fly it once to obtain an honest score and thermal history; the forged
    // records below are then committed and signed by their own pilot.
    let (mut fdr, license) = signed_rescue(prediction_a());
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
    let design = ThermalDesign::baseline();
    fdr.engine = Some(spec);
    fdr.initial_mass_kg = vehicle.initial_mass_kg;
    fdr.dry_mass_kg = vehicle.dry_mass_kg;
    fdr.thrust_n = vehicle.engine.thrust_newtons;
    fdr.specific_impulse_s = vehicle.engine.specific_impulse_seconds;
    let stage = design
        .stage(&contract, &spec, fdr.initial_mass_kg - fdr.dry_mass_kg)
        .unwrap();
    fdr.thermal_specs = vec![StageThermalSpec {
        stage: 0,
        tank_volumes_m3: [stage.oxidizer.volume_m3, stage.fuel.volume_m3],
        propellant_id: spec.propellant as u8,
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
    }];
    let replay = odd_verify::replay_flight(&odd_verify::spec_from_record(&fdr)).unwrap();
    fdr.metrics = replay.report;
    fdr.final_state_hash = replay.final_state_hash;
    fdr.boil_off_kg = replay.boil_off_kg;
    fdr.cryocooler_energy_j = replay.cryocooler_energy_j;
    fdr.vent_events = replay.vent_events;
    fdr.sign(&license).unwrap();
    assert!(fdr.metrics[4].is_finite());
    assert!(odd_verify::verify_record(&fdr).is_ok());

    for forgery in ["omitted tank", "invalid blanket", "false tank volume"] {
        let mut forged = fdr.clone();
        match forgery {
            "omitted tank" => {
                forged.thermal_specs.clear();
                forged.vent_events.clear();
                forged.boil_off_kg = [0.0, 0.0];
                forged.cryocooler_energy_j = 0.0;
            }
            "invalid blanket" => {
                forged.thermal_specs[0].epsilon_eff = 0.000_001;
                forged.vent_events.clear();
                forged.boil_off_kg = [0.0, 0.0];
                forged.cryocooler_energy_j = 0.0;
            }
            "false tank volume" => forged.thermal_specs[0].tank_volumes_m3[1] *= 1.5,
            _ => unreachable!(),
        }
        forged.sign(&license).unwrap();
        let mut authority = engine();
        let ticket = commit_for(&mut authority, &forged, &license);
        assert!(
            matches!(
                authority.submit(submit_request(&forged, ticket), NOW),
                Err(ServerError::ReplayRejected(_))
            ),
            "a valid commitment and pilot signature must not certify {forgery}"
        );
        assert_eq!(
            authority
                .store()
                .load_player(&license.public_key())
                .unwrap()
                .ranked_count,
            0
        );
    }

    // A signer can forge the thermal environment rather than the tank.
    // Recompute every metric and vent event for a one-kelvin support; no
    // cooling hardware, electrical draw or mass was added to obtain it.
    for field in ["structure", "sink"] {
        let mut cold_support = fdr.clone();
        match field {
            "structure" => cold_support.thermal_specs[0].structure_temperature_k = 1.0,
            "sink" => cold_support.thermal_specs[0].sink_temperature_k = 1.0,
            _ => unreachable!(),
        }
        let replay =
            odd_verify::replay_flight(&odd_verify::spec_from_record(&cold_support)).unwrap();
        cold_support.metrics = replay.report;
        cold_support.final_state_hash = replay.final_state_hash;
        cold_support.boil_off_kg = replay.boil_off_kg;
        cold_support.cryocooler_energy_j = replay.cryocooler_energy_j;
        cold_support.vent_events = replay.vent_events;
        cold_support.sign(&license).unwrap();
        assert_eq!(cold_support.verify_signature(), Ok(()));
        if field == "structure" {
            let loss = cold_support.boil_off_kg.iter().sum::<f64>();
            let baseline = fdr.boil_off_kg.iter().sum::<f64>();
            println!(
                "forged 1 K support: loss {baseline:.6} -> {loss:.6} kg without added hardware"
            );
            assert!(
                loss < baseline,
                "the invented support must demonstrably reduce loss"
            );
        }
        let mut authority = engine();
        let ticket = commit_for(&mut authority, &cold_support, &license);
        assert!(
            matches!(
                authority.submit(submit_request(&cold_support, ticket), NOW),
                Err(ServerError::ReplayRejected(_))
            ),
            "replayed but invented {field} temperature must not earn a receipt"
        );
        assert_eq!(
            authority
                .store()
                .load_player(&license.public_key())
                .unwrap()
                .ranked_count,
            0
        );
    }

    let mut supercritical = fdr.clone();
    supercritical.thermal_specs[0].relief_pressure_pa = 5_000_000.0;
    supercritical.sign(&license).unwrap();
    assert!(
        odd_verify::certify_thermal(&supercritical).is_err(),
        "a relief isotherm above the liquid's critical temperature cannot be certified"
    );
}

#[test]
fn committed_flight_plan_cannot_be_changed_after_launch() {
    let (original, pilot) = signed_rescue(prediction_a());
    let mut authority = engine();
    let ticket = commit_for(&mut authority, &original, &pilot);

    // A malicious client can compile, replay and sign a different plan
    // while keeping exactly the same committed vehicle and prediction.
    let mut later = original.clone();
    later.commands[0].delta_v_tnw[0] += 1.0;
    let replay = odd_verify::replay_flight(&odd_verify::spec_from_record(&later)).unwrap();
    later.metrics = replay.report;
    later.final_state_hash = replay.final_state_hash;
    later.boil_off_kg = replay.boil_off_kg;
    later.cryocooler_energy_j = replay.cryocooler_energy_j;
    later.vent_events = replay.vent_events;
    later.sign(&pilot).unwrap();
    assert!(
        odd_verify::verify_record(&later).is_ok(),
        "the changed plan is independently valid"
    );
    assert_eq!(
        authority
            .submit(submit_request(&later, ticket), NOW)
            .unwrap_err(),
        ServerError::DesignCommitmentMismatch,
        "a signed launch ticket must bind the maneuver plan as well as the vehicle"
    );
    assert_eq!(
        authority
            .store()
            .load_player(&pilot.public_key())
            .unwrap()
            .ranked_count,
        0
    );
}

#[test]
fn launch_hash_binds_command_order_epochs_vectors_and_guidance() {
    let (original, _) = signed_rescue(prediction_a());
    let reference = odd_worker::commitment::design_hash(&original);
    for field in ["epoch", "vector", "guidance", "order", "count"] {
        let mut changed = original.clone();
        match field {
            "epoch" => changed.commands[0].time_us += 1,
            "vector" => changed.commands[0].delta_v_tnw[2] = 0.5,
            "guidance" => changed.commands[0].guidance = Some([1.0; 6]),
            "order" => changed.commands.reverse(),
            "count" => {
                changed.commands.pop();
            }
            _ => unreachable!(),
        }
        assert_ne!(
            odd_worker::commitment::design_hash(&changed),
            reference,
            "{field}"
        );
    }
    let mut outcome_only = original;
    outcome_only.metrics[4] = 0.5;
    assert_eq!(
        odd_worker::commitment::design_hash(&outcome_only),
        reference,
        "outcomes are verified by replay, not known at launch"
    );
}
