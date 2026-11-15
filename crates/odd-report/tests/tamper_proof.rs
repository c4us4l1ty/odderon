//! oracle_tamper_proof_fdr_verification (canonical half).
//!
//! The physics half lives in odd-view, which owns the integrator that must
//! reproduce the report. Here we prove the container itself: every field a
//! reader acts on is signed, one bit anywhere is rejected with a specific
//! error, and the transport forms (JSON, #verify= fragment, QR) round-trip.
use odd_identity::License;
use odd_report::dossier::{BurnRow, Geometry};
use odd_report::{dossier, AttemptMode, Command, Fdr, FdrError, METRIC_COUNT, SCHEMA_VERSION};

fn flight() -> Fdr {
    let mut metrics = [f64::NAN; METRIC_COUNT];
    metrics[0] = 8_638.043_279_093_412;
    metrics[1] = 389_522_542.501_982_3;
    metrics[4] = 0.912_345_678_901_234;
    metrics[5] = 3240.0;
    metrics[24] = 6.964_879e-6;
    metrics[29] = 472_983.798_253;
    metrics[57] = 3.321_004_231_882_19;
    for mask in 0..16 {
        metrics[41 + mask] = 380_000_000.0 + mask as f64 * 1234.5;
    }
    Fdr {
        schema_version: SCHEMA_VERSION,
        contract_id: 101,
        attempt_mode: AttemptMode::Ranked,
        callsign: "GATE PILOT".into(),
        epoch_s: 820_497_600,
        duration_us: 10_800_000_000,
        initial_mass_kg: 1200.0,
        dry_mass_kg: 1020.0,
        thrust_n: 2400.0,
        specific_impulse_s: 310.0,
        initial_state: [6_638_137.0, 0.0, 0.0, 0.0, 7_748.541_233_1, 0.0],
        node_metric: false,
        prediction_baseline: [180.0, 550_000.0, 260_000.0, 120.0, 10_800.0, f64::NAN],
        commands: vec![
            Command {
                time_us: 200_000_000,
                delta_v_tnw: [81.509_425_05, 0.0, 0.0],
                guidance: None,
            },
            Command {
                time_us: 2_995_257_948,
                delta_v_tnw: [98.4, 0.0, 0.0],
                guidance: Some([6_928_137.0, 0.0, 0.0, 0.0, 7_500.0, 0.0]),
            },
        ],
        metrics,
        final_state_hash: odd_report::fdr::final_state_hash(
            10_800.0,
            &[6_928_200.0, 12.0, -3.0, -1.0, 7_500.1, 0.5],
            1080.5,
        ),
        elo_before: 1200.0,
        elo_after: 1203.5,
        public_key: [0; 32],
        signature: [0; 64],
    }
}

fn signed() -> (Fdr, License) {
    let license = License::from_entropy(core::array::from_fn(|i| (i as u8) * 11 + 5));
    let mut fdr = flight();
    fdr.sign(&license).unwrap();
    (fdr, license)
}

#[test]
fn a_signed_recorder_verifies_and_survives_every_transport() {
    let (fdr, _) = signed();
    assert_eq!(fdr.verify_signature(), Ok(()));

    // #verify= fragment and the QR payload are the same bytes.
    let payload = fdr.verify_payload();
    assert!(payload.starts_with("#verify="));
    let restored = Fdr::from_verify_payload(&payload).unwrap();
    assert_eq!(restored, fdr);
    assert_eq!(restored.verify_signature(), Ok(()));
    assert_eq!(restored.canonical_bytes(), fdr.canonical_bytes());

    // The QR must be able to carry a realistic recorder.
    let qr = odd_report::qr::Qr::new(&payload).expect("a real recorder must fit in a QR");
    assert!(qr.size >= 21);
    assert!(!qr.runs.is_empty());
}

#[test]
fn one_ulp_of_any_metric_breaks_the_signature() {
    let (fdr, _) = signed();
    for index in 0..METRIC_COUNT {
        if !fdr.metrics[index].is_finite() {
            continue;
        }
        let mut tampered = fdr.clone();
        tampered.metrics[index] = f64::from_bits(fdr.metrics[index].to_bits() + 1);
        assert_ne!(
            tampered.metrics[index], fdr.metrics[index],
            "field {index} must actually change"
        );
        assert_eq!(
            tampered.verify_signature(),
            Err(FdrError::SignatureMismatch),
            "one ULP in metric {index} must be rejected"
        );
    }
}

#[test]
fn one_microsecond_of_command_timing_breaks_the_signature() {
    let (fdr, _) = signed();
    for index in 0..fdr.commands.len() {
        let mut tampered = fdr.clone();
        tampered.commands[index].time_us += 1;
        assert_eq!(
            tampered.verify_signature(),
            Err(FdrError::SignatureMismatch),
            "one microsecond on command {index} must be rejected"
        );
    }
}

#[test]
fn every_other_signed_field_is_also_covered() {
    let (fdr, _) = signed();
    type Mutation = (&'static str, Box<dyn Fn(&mut Fdr)>);
    let mutations: Vec<Mutation> = vec![
        ("contract", Box::new(|f| f.contract_id = 102)),
        ("mode", Box::new(|f| f.attempt_mode = AttemptMode::Practice)),
        ("callsign", Box::new(|f| f.callsign = "SOMEONE ELSE".into())),
        ("epoch", Box::new(|f| f.epoch_s += 1)),
        ("duration", Box::new(|f| f.duration_us += 1)),
        ("initial mass", Box::new(|f| f.initial_mass_kg += 1e-9)),
        ("dry mass", Box::new(|f| f.dry_mass_kg += 1e-9)),
        ("thrust", Box::new(|f| f.thrust_n += 1e-9)),
        ("isp", Box::new(|f| f.specific_impulse_s += 1e-9)),
        (
            "initial state",
            Box::new(|f| f.initial_state[4] = f64::from_bits(f.initial_state[4].to_bits() + 1)),
        ),
        ("metric selection", Box::new(|f| f.node_metric = true)),
        ("prediction", Box::new(|f| f.prediction_baseline[1] += 1e-9)),
        (
            "delta-v",
            Box::new(|f| f.commands[0].delta_v_tnw[0] += 1e-12),
        ),
        ("guidance", Box::new(|f| f.commands[1].guidance = None)),
        ("terminal hash", Box::new(|f| f.final_state_hash[0] ^= 1)),
        ("elo before", Box::new(|f| f.elo_before += 0.1)),
        ("elo after", Box::new(|f| f.elo_after += 0.1)),
        ("command order", Box::new(|f| f.commands.reverse())),
        (
            "command removed",
            Box::new(|f| {
                f.commands.pop();
            }),
        ),
    ];
    for (label, mutate) in mutations {
        let mut tampered = fdr.clone();
        mutate(&mut tampered);
        assert_eq!(
            tampered.verify_signature(),
            Err(FdrError::SignatureMismatch),
            "{label} must be inside the signed bytes"
        );
    }
}

#[test]
fn a_different_signer_cannot_adopt_the_recorder() {
    let (fdr, _) = signed();
    let other = License::from_entropy([9u8; 16]);
    let mut tampered = fdr.clone();
    tampered.public_key = other.public_key();
    assert_eq!(
        tampered.verify_signature(),
        Err(FdrError::SignatureMismatch)
    );
    // Re-signing with the other key produces a valid document that clearly
    // identifies the other pilot, which is the intended behaviour.
    tampered.sign(&other).unwrap();
    assert_eq!(tampered.verify_signature(), Ok(()));
    assert_eq!(tampered.public_key, other.public_key());
}

#[test]
fn replay_comparison_names_the_first_differing_field() {
    let (fdr, _) = signed();
    assert_eq!(fdr.compare_metrics(&fdr.metrics), Ok(()));
    let mut replayed = fdr.metrics;
    replayed[24] = f64::from_bits(replayed[24].to_bits() + 1);
    assert_eq!(
        fdr.compare_metrics(&replayed),
        Err(FdrError::PhysicsReplayMismatch {
            field: "metrics",
            index: 24
        })
    );
    // NaN is undefined, not a value: two undefined fields agree.
    replayed = fdr.metrics;
    replayed[3] = f64::from_bits(0x7ff8_0000_0000_0001);
    assert!(fdr.metrics[3].is_nan());
    assert_eq!(fdr.compare_metrics(&replayed), Ok(()));
    // Signed zero is a real difference and must not be normalized away.
    replayed = fdr.metrics;
    replayed[3] = 0.0;
    let mut zeroed = fdr.clone();
    zeroed.metrics[3] = -0.0;
    assert_eq!(
        zeroed.compare_metrics(&replayed),
        Err(FdrError::PhysicsReplayMismatch {
            field: "metrics",
            index: 3
        })
    );
    let mut hash = fdr.final_state_hash;
    hash[31] ^= 1;
    assert_eq!(
        fdr.compare_final_state(&hash),
        Err(FdrError::FinalStateMismatch)
    );
    assert_eq!(fdr.compare_final_state(&fdr.final_state_hash), Ok(()));
}

#[test]
fn malformed_and_future_documents_are_rejected_by_name() {
    let (fdr, _) = signed();
    let mut future = fdr.clone();
    future.schema_version = SCHEMA_VERSION + 1;
    assert_eq!(
        future.verify_signature(),
        Err(FdrError::UnsupportedSchema(SCHEMA_VERSION + 1))
    );
    let bytes = fdr.canonical_bytes();
    assert_eq!(
        Fdr::from_canonical_with_signature(&bytes[..bytes.len() - 1]),
        Err(FdrError::Malformed("truncated"))
    );
    let mut extended = bytes.clone();
    extended.extend_from_slice(&fdr.public_key);
    extended.extend_from_slice(&fdr.signature);
    extended.push(0);
    assert_eq!(
        Fdr::from_canonical_with_signature(&extended),
        Err(FdrError::Malformed("trailing bytes"))
    );
    assert_eq!(
        Fdr::from_canonical_with_signature(b"not a recorder at all"),
        Err(FdrError::Malformed("not an Odderon flight data recorder"))
    );
    assert!(Fdr::from_verify_payload("#verify=!!!!").is_err());
    let mut bad = fdr.clone();
    bad.commands[0].time_us = 0;
    assert_eq!(
        bad.verify_signature(),
        Err(FdrError::Malformed("command epoch outside the flight"))
    );
}

#[test]
fn canonical_form_is_independent_of_json_formatting() {
    let (fdr, _) = signed();
    let json = fdr.to_json();
    assert!(json.contains("\"schema_version\":1"));
    assert!(json.contains("\"attempt_mode\":\"Ranked\""));
    assert!(json.contains("\"command_timeline\":["));
    // Undefined metrics travel as null, and the reader maps null back to NaN.
    assert!(json.contains("null"));
    // Round-trip every number through its decimal form: the printed form must
    // parse back to identical bits, or JSON transport would lose precision.
    for value in fdr.metrics.iter().filter(|v| v.is_finite()) {
        let printed = value.to_string();
        let parsed: f64 = printed.parse().unwrap();
        assert_eq!(parsed.to_bits(), value.to_bits(), "{printed}");
    }
    // Re-encoding a parsed document yields the same canonical bytes.
    let restored = Fdr::from_verify_payload(&fdr.verify_payload()).unwrap();
    assert_eq!(restored.to_json(), json);
}

#[test]
fn oracle_odd_report_deterministic_dossier() {
    let (fdr, _) = signed();
    let geometry = Geometry {
        level1: (0..=180)
            .map(|i| {
                let angle = i as f64 * core::f64::consts::TAU / 180.0;
                [6_638_137.0 * angle.cos(), 6_638_137.0 * angle.sin()]
            })
            .collect(),
        level3: (0..=180)
            .map(|i| {
                let angle = i as f64 * core::f64::consts::TAU / 180.0;
                let radius = 6_638_137.0 + 290_000.0 * (angle / 2.0).sin();
                [radius * angle.cos(), radius * angle.sin()]
            })
            .collect(),
        burns: vec![BurnRow {
            start_s: 200.0,
            duration_s: 41.5,
            delta_v_tnw: [81.509_425_05, 0.0, 0.0],
            sweep_rad: 0.021,
        }],
        earth_radius_m: 6_378_137.0,
    };
    let payload = fdr.verify_payload();
    let first = dossier::html(&fdr, &geometry, &payload).unwrap();
    let second = dossier::html(&fdr, &geometry, &payload).unwrap();
    assert_eq!(
        first, second,
        "two runs must be byte identical, including the QR matrix"
    );
    assert_eq!(
        odd_report::qr::Qr::new(&payload)
            .unwrap()
            .svg("verification-qr"),
        odd_report::qr::Qr::new(&payload)
            .unwrap()
            .svg("verification-qr")
    );
    for marker in [
        "1. Flight identification",
        "2. Trajectory reconstruction",
        "3. Prediction versus",
        "4. Independent verification",
    ] {
        assert!(first.contains(marker), "missing section: {marker}");
    }
    // The payload embedded in the document must still verify.
    let extracted = first
        .split("verify_payload   ")
        .nth(1)
        .unwrap()
        .split("</pre>")
        .next()
        .unwrap();
    let decoded = Fdr::from_verify_payload(extracted).unwrap();
    assert_eq!(decoded.verify_signature(), Ok(()));
    assert_eq!(decoded, fdr);
}

#[test]
fn non_finite_values_are_refused_by_name_not_lost_in_transport() {
    // JSON prints every non-finite number as null and the reader maps null
    // back to NaN. An infinity would therefore verify locally and then fail
    // with SignatureMismatch after its own round trip, so it must be refused
    // at signing time instead.
    let license = odd_identity::License::from_entropy([5u8; 16]);
    type Case = (&'static str, fn(&mut Fdr));
    let cases: Vec<Case> = vec![
        ("infinite metric", |f| f.metrics[1] = f64::INFINITY),
        ("negative infinite metric", |f| {
            f.metrics[2] = f64::NEG_INFINITY
        }),
        ("infinite prediction", |f| {
            f.prediction_baseline[1] = f64::INFINITY
        }),
        ("infinite mass", |f| f.initial_mass_kg = f64::INFINITY),
        ("NaN mass", |f| f.dry_mass_kg = f64::NAN),
        ("NaN thrust", |f| f.thrust_n = f64::NAN),
        ("infinite elo", |f| f.elo_after = f64::INFINITY),
        ("NaN initial state", |f| f.initial_state[4] = f64::NAN),
        ("NaN delta-v", |f| f.commands[0].delta_v_tnw[0] = f64::NAN),
        ("infinite guidance", |f| {
            f.commands[1].guidance = Some([f64::INFINITY; 6])
        }),
    ];
    for (label, mutate) in cases {
        let mut fdr = flight();
        mutate(&mut fdr);
        assert!(
            matches!(fdr.sign(&license), Err(FdrError::Malformed(_))),
            "{label} must be refused with a named Malformed error"
        );
    }
    // NaN remains legal exactly where the format documents an undefined value.
    let mut fdr = flight();
    fdr.metrics[3] = f64::NAN;
    fdr.prediction_baseline[5] = f64::NAN;
    fdr.sign(&license).unwrap();
    assert_eq!(fdr.verify_signature(), Ok(()));
    let json = fdr.to_json();
    assert!(json.contains("null"), "undefined values travel as null");
    let restored = Fdr::from_verify_payload(&fdr.verify_payload()).unwrap();
    assert_eq!(restored.verify_signature(), Ok(()));
    assert_eq!(restored.to_json(), json);
}

#[test]
fn a_maximal_guided_timeline_still_prints_a_dossier() {
    // Twelve guided commands is the planner's ceiling and produces a payload
    // larger than QR Medium capacity. The dossier must still print, and must
    // state the error correction level it actually used.
    let mut fdr = flight();
    fdr.commands = (0..12)
        .map(|i| Command {
            time_us: 1_600_000_000 + i * 600_000_000,
            delta_v_tnw: [270.123_456_789, 0.0, 0.0],
            guidance: Some([6_578_137.123, -123.456, 78.9, 12.34, 7_784.567_89, -0.001]),
        })
        .collect();
    fdr.duration_us = 600_000_000_000;
    let license = odd_identity::License::from_entropy([6u8; 16]);
    fdr.sign(&license).unwrap();
    let payload = fdr.verify_payload();
    assert!(
        payload.len() > 2331,
        "this case must exceed Medium capacity, got {}",
        payload.len()
    );
    let qr = odd_report::qr::Qr::new(&payload).expect("a maximal flight must still print");
    assert_eq!(qr.ecc_label(), "L");
    let geometry = Geometry {
        level1: vec![[6_578_137.0, 0.0], [0.0, 6_578_137.0]],
        level3: vec![[6_578_137.0, 0.0], [10.0, 6_578_200.0]],
        burns: vec![],
        earth_radius_m: 6_378_137.0,
    };
    let document = dossier::html(&fdr, &geometry, &payload).unwrap();
    assert!(
        document.contains("data-qr-ecc=\"L\""),
        "the SVG must state the level it used"
    );
    assert!(
        document.contains("qr_ecc           L"),
        "the printed block must agree"
    );
    assert_eq!(document, dossier::html(&fdr, &geometry, &payload).unwrap());
    // And the payload still decodes back to the same signed flight.
    assert_eq!(Fdr::from_verify_payload(&payload).unwrap(), fdr);
}
