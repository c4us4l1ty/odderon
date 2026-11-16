#![cfg(feature = "local")]
//! The local HTTP adapter carries the authority end to end: the same
//! commit→submit flow the browser uses, over JSON, with no cloud credentials.
//! Driven both socket-free through `route` and through a real TCP round trip.

#[path = "common/mod.rs"]
mod common;

use common::*;
use odd_worker::http::{
    self, commit_to_json, json::Json, receipt_from_json, route, submit_to_json, ticket_from_json,
};
use odd_worker::{AuthorityEngine, MemoryStore};

fn commit_json(
    engine: &mut AuthorityEngine<MemoryStore>,
    fdr: &odd_report::Fdr,
    license: &odd_identity::License,
) -> Json {
    let mut req = odd_worker::CommitRequest {
        contract_id: fdr.contract_id,
        pubkey: license.public_key(),
        design_hash: odd_worker::commitment::design_hash(fdr),
        prediction_hash: odd_worker::commitment::prediction_hash(fdr),
        client_sig: [0u8; 64],
    };
    req.client_sig = license.sign(&req.canonical_bytes());
    let resp = route(
        engine,
        "POST",
        "/api/v1/commit",
        &commit_to_json(&req).to_string(),
        NOW,
    );
    assert_eq!(resp.status, 200, "{}", resp.body);
    Json::parse(&resp.body).unwrap()
}

#[test]
fn a_ranked_flight_commits_and_submits_over_http_json() {
    let mut engine = AuthorityEngine::new(MemoryStore::new(), server_key());
    let (fdr, license) = signed_rescue(prediction_a());
    let ticket_json = commit_json(&mut engine, &fdr, &license);
    let ticket = ticket_from_json(&ticket_json).unwrap();
    assert_eq!(ticket.attempt_num, 1);

    let req = submit_request(&fdr, ticket);
    let resp = route(
        &mut engine,
        "POST",
        "/api/v1/submit",
        &submit_to_json(&req).to_string(),
        NOW,
    );
    assert_eq!(resp.status, 200, "{}", resp.body);
    let body = Json::parse(&resp.body).unwrap();
    assert_eq!(body.get("accepted").unwrap(), &Json::Bool(true));
    let receipt = receipt_from_json(body.get("receipt").unwrap()).unwrap();
    assert!(receipt.is_certified_ranked(&engine.server_pk()));
    let elo_after = body.get("elo_after").unwrap().as_f64().unwrap();
    assert!(elo_after != 1200.0);
}

#[test]
fn hindsight_is_refused_over_http_with_a_stable_code() {
    let mut engine = AuthorityEngine::new(MemoryStore::new(), server_key());
    let (fdr_a, license) = signed_rescue(prediction_a());
    let ticket = ticket_from_json(&commit_json(&mut engine, &fdr_a, &license)).unwrap();

    let mut pred_b = prediction_a();
    pred_b[1] = 600_000.0;
    let (fdr_b, _) = signed_rescue(pred_b);
    let req = submit_request(&fdr_b, ticket);
    let resp = route(
        &mut engine,
        "POST",
        "/api/v1/submit",
        &submit_to_json(&req).to_string(),
        NOW,
    );
    assert_eq!(resp.status, 400);
    let body = Json::parse(&resp.body).unwrap();
    assert_eq!(
        body.get("code").unwrap(),
        &Json::Str("PredictionCommitmentMismatch".into())
    );
}

#[test]
fn a_second_commit_is_practice_over_http() {
    let mut engine = AuthorityEngine::new(MemoryStore::new(), server_key());
    let (fdr, license) = signed_rescue(prediction_a());
    let first = ticket_from_json(&commit_json(&mut engine, &fdr, &license)).unwrap();
    let second = ticket_from_json(&commit_json(&mut engine, &fdr, &license)).unwrap();
    assert_eq!(first.mode, odd_worker::AttemptMode::Ranked);
    assert_eq!(second.attempt_num, 2);
    assert_eq!(second.mode, odd_worker::AttemptMode::Practice);
}

#[test]
fn a_real_tcp_request_reaches_the_engine() {
    use std::io::{Read, Write};
    use std::net::TcpStream;

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let engine = AuthorityEngine::new(MemoryStore::new(), server_key());
    std::thread::spawn(move || {
        let _ = http::serve_listener(listener, engine);
    });

    let (fdr, license) = signed_rescue(prediction_a());
    let mut req = odd_worker::CommitRequest {
        contract_id: fdr.contract_id,
        pubkey: license.public_key(),
        design_hash: odd_worker::commitment::design_hash(&fdr),
        prediction_hash: odd_worker::commitment::prediction_hash(&fdr),
        client_sig: [0u8; 64],
    };
    req.client_sig = license.sign(&req.canonical_bytes());
    let body = commit_to_json(&req).to_string();
    let request = format!(
        "POST /api/v1/commit HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        body.len(),
        body
    );
    let mut stream = TcpStream::connect(addr).unwrap();
    stream.write_all(request.as_bytes()).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    let (_, json_body) = response.split_once("\r\n\r\n").unwrap();
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
    let ticket = ticket_from_json(&Json::parse(json_body).unwrap()).unwrap();
    assert_eq!(ticket.attempt_num, 1);
    assert_eq!(ticket.mode, odd_worker::AttemptMode::Ranked);
}

fn callsign_body(name: &str, license: &odd_identity::License) -> String {
    let mut req = odd_worker::CallsignRequest {
        name: name.into(),
        pubkey: license.public_key(),
        signature: [0; 64],
    };
    req.signature = license.sign(&req.canonical_bytes());
    format!(
        r#"{{"name":"{}","pubkey":"{}","sig":"{}"}}"#,
        req.name,
        odd_report::hex::encode(&req.pubkey),
        odd_report::hex::encode(&req.signature)
    )
}

#[test]
fn callsigns_are_signed_case_insensitive_and_first_come_first_served() {
    let mut engine = AuthorityEngine::new(MemoryStore::new(), server_key());
    let alice = odd_identity::License::from_entropy([1; 16]);
    let bob = odd_identity::License::from_entropy([2; 16]);
    let valid = callsign_body("Star-Pilot", &alice);
    let r = route(&mut engine, "POST", "/api/v1/callsign", &valid, NOW);
    assert_eq!(r.status, 200, "{}", r.body);
    assert_eq!(
        Json::parse(&r.body).unwrap().get("name"),
        Some(&Json::Str("STAR-PILOT".into()))
    );
    assert_eq!(
        route(&mut engine, "POST", "/api/v1/callsign", &valid, NOW).status,
        200
    );
    let taken = route(
        &mut engine,
        "POST",
        "/api/v1/callsign",
        &callsign_body("star-pilot", &bob),
        NOW,
    );
    assert_eq!(
        Json::parse(&taken.body).unwrap().get("code"),
        Some(&Json::Str("CallsignTaken".into()))
    );
    let forged = valid.replace("Star-Pilot", "New-Pilot");
    let forged = route(&mut engine, "POST", "/api/v1/callsign", &forged, NOW);
    assert_eq!(
        Json::parse(&forged.body).unwrap().get("code"),
        Some(&Json::Str("InvalidClientSignature".into()))
    );
    let invalid = route(
        &mut engine,
        "POST",
        "/api/v1/callsign",
        &callsign_body("bad!", &alice),
        NOW,
    );
    assert_eq!(
        Json::parse(&invalid.body).unwrap().get("code"),
        Some(&Json::Str("InvalidCallsign".into()))
    );
}

#[test]
fn flown_receipt_is_publicly_retrievable_and_listed_on_ranked_board() {
    let mut engine = AuthorityEngine::new(MemoryStore::new(), server_key());
    let (fdr, license) = signed_rescue(prediction_a());
    route(
        &mut engine,
        "POST",
        "/api/v1/callsign",
        &callsign_body("ACE", &license),
        NOW,
    );
    let ticket = ticket_from_json(&commit_json(&mut engine, &fdr, &license)).unwrap();
    let submit = route(
        &mut engine,
        "POST",
        "/api/v1/submit",
        &submit_to_json(&submit_request(&fdr, ticket)).to_string(),
        NOW,
    );
    assert_eq!(submit.status, 200, "{}", submit.body);
    let submitted = Json::parse(&submit.body).unwrap();
    let hash = odd_report::hex::encode(&fdr.digest());
    let public = route(
        &mut engine,
        "GET",
        &format!("/api/v1/verify/{hash}"),
        "",
        NOW,
    );
    assert_eq!(public.status, 200);
    let public = Json::parse(&public.body).unwrap();
    assert_eq!(
        public.get("fdr_payload"),
        Some(&Json::Str(fdr.verify_payload()))
    );
    assert_eq!(public.get("receipt"), submitted.get("receipt"));
    assert!(receipt_from_json(public.get("receipt").unwrap())
        .unwrap()
        .verify(&engine.server_pk()));
    let board = route(
        &mut engine,
        "GET",
        "/api/v1/leaderboard?contract=101&page=1",
        "",
        NOW,
    );
    let board = Json::parse(&board.body).unwrap();
    let rows = board.as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get("callsign"), Some(&Json::Str("ACE".into())));
    assert_eq!(rows[0].get("fdr_hash"), Some(&Json::Str(hash)));
    let page2 = route(
        &mut engine,
        "GET",
        "/api/v1/leaderboard?contract=101&page=2",
        "",
        NOW,
    );
    assert!(Json::parse(&page2.body)
        .unwrap()
        .as_array()
        .unwrap()
        .is_empty());
    assert_eq!(
        route(&mut engine, "GET", "/api/v1/leaderboard?page=0", "", NOW).status,
        400
    );
    assert_eq!(
        route(&mut engine, "GET", "/api/v1/verify/00", "", NOW).status,
        400
    );
    let player = route(
        &mut engine,
        "GET",
        &format!(
            "/api/v1/player/{}",
            odd_report::hex::encode(&license.public_key())
        ),
        "",
        NOW,
    );
    assert_eq!(
        Json::parse(&player.body).unwrap().get("elo"),
        submitted.get("elo_after")
    );
}

#[test]
fn real_server_rejects_oversized_body_and_ambiguous_length() {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        let _ = http::serve_listener(
            listener,
            AuthorityEngine::new(MemoryStore::new(), server_key()),
        );
    });
    let send = |request: &str| {
        let mut stream = std::net::TcpStream::connect(addr).unwrap();
        stream.write_all(request.as_bytes()).unwrap();
        let mut reply = String::new();
        stream.read_to_string(&mut reply).unwrap();
        reply
    };
    let huge = format!(
        "POST /api/v1/commit HTTP/1.1\r\nHost: x\r\nContent-Length: {}\r\n\r\n",
        http::MAX_BODY_BYTES + 1
    );
    assert!(send(&huge).starts_with("HTTP/1.1 413"));
    assert!(send(
        "POST /api/v1/commit HTTP/1.1\r\nHost: x\r\nContent-Length: 0\r\nContent-Length: 1\r\n\r\n"
    )
    .starts_with("HTTP/1.1 400"));
    assert!(Json::parse(&format!("{}0{}", "[".repeat(1000), "]".repeat(1000))).is_none());
}
