//! WebAssembly boundary for a transactional authority adapter.
//!
//! The host supplies a *database snapshot*, never client-provided rating
//! claims. This module runs the same HTTP router and AuthorityEngine as the
//! native server and returns its proposed writes. The host MUST commit those
//! writes atomically only if the snapshot's revision still matches; see the
//! Cloudflare adapter. An uncommitted proposal is never a server response.

use odd_report::fdr::Fdr;
use odd_worker::http::{self, json::Json};
use odd_worker::{
    AttemptMode, AuthorityEngine, Commitment, MemoryStore, PlayerRow, ServerKey, ServerReceipt,
    Store, StoredFdr,
};
use std::collections::BTreeMap;
use std::sync::Mutex;

const MAX_INPUT: usize = 1024 * 1024;
static INPUT: Mutex<Vec<u8>> = Mutex::new(Vec::new());
static OUTPUT: Mutex<Vec<u8>> = Mutex::new(Vec::new());

fn obj(items: impl IntoIterator<Item = (&'static str, Json)>) -> Json {
    Json::Obj(items.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
}
fn string(value: &Json) -> Option<&str> {
    if let Json::Str(s) = value {
        Some(s)
    } else {
        None
    }
}
fn get_str<'a>(value: &'a Json, key: &str) -> Option<&'a str> {
    string(value.get(key)?)
}
fn hex32(s: &str) -> Option<[u8; 32]> {
    odd_report::hex::decode(s)?.try_into().ok()
}
fn mode(n: u32) -> Option<AttemptMode> {
    AttemptMode::from_u8(u8::try_from(n).ok()?)
}
fn optional<'a>(value: &'a Json, key: &str) -> Option<&'a Json> {
    value.get(key).filter(|v| !matches!(v, Json::Null))
}
fn bytes(v: &[u8]) -> Json {
    Json::Str(odd_report::hex::encode(v))
}
fn num(n: impl Into<f64>) -> Json {
    Json::Num(n.into())
}

fn player_json(player: &PlayerRow) -> Json {
    obj([
        ("elo", Json::Num(player.elo)),
        ("ranked", num(player.ranked_count)),
        (
            "callsign",
            player.callsign.clone().map(Json::Str).unwrap_or(Json::Null),
        ),
    ])
}

/// Inspect only the canonical FDR header and signature. This lets the host
/// load an existing content-addressed row before attempting a submission.
fn inspect(root: &Json) -> Result<Json, &'static str> {
    let payload = get_str(root, "payload").ok_or("missing payload")?;
    if payload.len() > http::MAX_BODY_BYTES {
        return Err("payload too large");
    }
    let fdr = Fdr::from_verify_payload(payload).map_err(|_| "malformed recorder")?;
    fdr.verify_signature()
        .map_err(|_| "invalid recorder signature")?;
    Ok(obj([
        ("status", num(200)),
        ("hash", bytes(&fdr.digest())),
        ("delegation_mask", num(fdr.delegation_mask)),
    ]))
}

/// Reconstitute the few rows this operation can touch. The host supplies
/// these from durable storage, not from the request body. Malformed or
/// internally inconsistent snapshots fail closed instead of resetting Elo.
fn load_snapshot(root: &Json, key: &ServerKey) -> Result<MemoryStore, &'static str> {
    let snapshot = root.get("snapshot").ok_or("missing snapshot")?;
    let subject = root.get("subject").ok_or("missing subject")?;
    let pk =
        hex32(get_str(subject, "pubkey").ok_or("missing public key")?).ok_or("bad public key")?;
    let contract = subject
        .get("contract")
        .and_then(Json::as_u32)
        .ok_or("bad contract")?;
    let mut store = MemoryStore::new();

    if let Some(owner) = optional(snapshot, "owner") {
        let name = get_str(owner, "name").ok_or("bad owner")?;
        let owner_pk = hex32(get_str(owner, "pubkey").ok_or("bad owner")?).ok_or("bad owner")?;
        store
            .register_callsign(name, &owner_pk)
            .map_err(|_| "bad owner")?;
    }
    if let Some(player) = optional(snapshot, "player") {
        let elo = player
            .get("elo")
            .and_then(Json::as_f64)
            .filter(|v| v.is_finite())
            .ok_or("bad player elo")?;
        let ranked_count = player
            .get("ranked")
            .and_then(Json::as_u32)
            .ok_or("bad player count")?;
        let callsign = match player.get("callsign") {
            None | Some(Json::Null) => None,
            Some(Json::Str(value)) => Some(value.clone()),
            _ => return Err("bad callsign"),
        };
        store
            .save_player(
                &pk,
                &PlayerRow {
                    elo,
                    ranked_count,
                    callsign,
                },
            )
            .map_err(|_| "bad player")?;
    }
    if let Some(attempts) = optional(snapshot, "attempts") {
        store.seed_attempt(
            &pk,
            contract,
            attempts.as_u32().ok_or("bad attempt counter")?,
        );
    }
    if let Some(value) = optional(snapshot, "mission_elo") {
        let elo = value
            .as_f64()
            .filter(|v| v.is_finite())
            .ok_or("bad mission elo")?;
        store
            .save_mission_elo(contract, elo)
            .map_err(|_| "bad mission elo")?;
    }
    if let Some(c) = optional(snapshot, "commitment") {
        let attempt = c
            .get("attempt")
            .and_then(Json::as_u32)
            .ok_or("bad commitment")?;
        let commitment = Commitment {
            design_hash: hex32(get_str(c, "design_hash").ok_or("bad commitment")?)
                .ok_or("bad commitment")?,
            prediction_hash: hex32(get_str(c, "prediction_hash").ok_or("bad commitment")?)
                .ok_or("bad commitment")?,
            mode: mode(
                c.get("mode")
                    .and_then(Json::as_u32)
                    .ok_or("bad commitment")?,
            )
            .ok_or("bad commitment")?,
            server_time_s: c
                .get("server_time")
                .and_then(Json::as_i64)
                .ok_or("bad commitment")?,
        };
        store
            .save_commitment(&pk, contract, attempt, &commitment)
            .map_err(|_| "bad commitment")?;
    }
    if let Some(r) = optional(snapshot, "receipt") {
        let attempt = r
            .get("attempt")
            .and_then(Json::as_u32)
            .ok_or("bad receipt")?;
        let raw = odd_report::hex::decode(get_str(r, "bytes").ok_or("bad receipt")?)
            .ok_or("bad receipt")?;
        let receipt = ServerReceipt::from_bytes(&raw).ok_or("bad receipt")?;
        if !receipt.verify(&key.public_key())
            || receipt.payload.pubkey != pk
            || receipt.payload.contract_id != contract
            || receipt.payload.attempt_num != attempt
        {
            return Err("receipt binding is invalid");
        }
        store
            .save_receipt(&pk, contract, attempt, &receipt)
            .map_err(|_| "bad receipt")?;
    }
    if let Some(row) = optional(snapshot, "fdr") {
        let hash = hex32(get_str(row, "hash").ok_or("bad flight")?).ok_or("bad flight")?;
        let payload = get_str(row, "payload").ok_or("bad flight")?;
        let fdr = Fdr::from_verify_payload(payload).map_err(|_| "bad flight")?;
        fdr.verify_signature().map_err(|_| "bad flight signature")?;
        let owner = hex32(get_str(row, "pubkey").ok_or("bad flight")?).ok_or("bad flight")?;
        let id = row
            .get("contract")
            .and_then(Json::as_u32)
            .ok_or("bad flight")?;
        let attempt = row
            .get("attempt")
            .and_then(Json::as_u32)
            .ok_or("bad flight")?;
        let flight_mode = mode(row.get("mode").and_then(Json::as_u32).ok_or("bad flight")?)
            .ok_or("bad flight")?;
        let score = row
            .get("score")
            .and_then(Json::as_f64)
            .filter(|v| v.is_finite())
            .ok_or("bad flight")?;
        let mask = u8::try_from(
            row.get("delegation_mask")
                .and_then(Json::as_u32)
                .ok_or("bad flight")?,
        )
        .map_err(|_| "bad flight")?;
        if fdr.digest() != hash
            || fdr.public_key != owner
            || fdr.contract_id != id
            || fdr.attempt_mode != flight_mode
            || fdr.metrics[4].to_bits() != score.to_bits()
            || fdr.delegation_mask != mask
        {
            return Err("flight binding is invalid");
        }
        let receipt = if let Some(r) = optional(row, "receipt") {
            let raw = odd_report::hex::decode(string(r).ok_or("bad flight receipt")?)
                .ok_or("bad flight receipt")?;
            let receipt = ServerReceipt::from_bytes(&raw).ok_or("bad flight receipt")?;
            let p = &receipt.payload;
            if !receipt.verify(&key.public_key())
                || p.fdr_hash != hash
                || p.pubkey != owner
                || p.contract_id != id
                || p.attempt_num != attempt
                || p.mode != flight_mode
                || p.score_s_bits != score.to_bits()
            {
                return Err("flight receipt binding is invalid");
            }
            Some(receipt)
        } else {
            None
        };
        store
            .save_fdr(
                &hash,
                &StoredFdr {
                    bytes: payload.as_bytes().to_vec(),
                    pubkey: owner,
                    contract_id: id,
                    attempt_num: attempt,
                    mode: flight_mode,
                    score_s: score,
                    delegation_mask: mask,
                    receipt,
                },
            )
            .map_err(|_| "bad flight")?;
    }
    Ok(store)
}

fn route(root: &Json) -> Result<Json, &'static str> {
    let seed = hex32(get_str(root, "seed").ok_or("missing seed")?).ok_or("invalid seed")?;
    let key = ServerKey::from_seed(seed);
    let store = load_snapshot(root, &key)?;
    let mut engine = AuthorityEngine::new(store, key);
    let method = get_str(root, "method").ok_or("missing method")?;
    let path = get_str(root, "path").ok_or("missing path")?;
    let body = get_str(root, "body").ok_or("missing body")?;
    let now_s = root
        .get("now_s")
        .and_then(Json::as_i64)
        .ok_or("bad clock")?;
    let subject = root.get("subject").ok_or("missing subject")?;
    let pk =
        hex32(get_str(subject, "pubkey").ok_or("missing public key")?).ok_or("bad public key")?;
    let contract = subject
        .get("contract")
        .and_then(Json::as_u32)
        .ok_or("bad contract")?;
    let response = http::route(&mut engine, method, path, body, now_s);
    let mut output = BTreeMap::new();
    output.insert("status".into(), num(response.status));
    output.insert("body".into(), Json::Str(response.body.clone()));
    if response.status == 200 && method == "POST" {
        if path == "/api/v1/submit" || path == "/api/v1/submit-segment" {
            let body = Json::parse(&response.body).ok_or("invalid engine result")?;
            let receipt = http::receipt_from_json(body.get("receipt").ok_or("missing receipt")?)
                .ok_or("missing receipt")?;
            let hash = receipt.payload.fdr_hash;
            let flight = engine
                .store()
                .load_fdr(&hash)
                .map_err(|_| "flight lookup failed")?
                .ok_or("flight missing after submission")?;
            // A later identical Practice flight may have replaced the public
            // hash index with its own receipt. A retry of an earlier ticket
            // still returns that ticket's original receipt; the canonical
            // record itself remains the same and was checked on snapshot load.
            let retried =
                optional(root.get("snapshot").ok_or("missing snapshot")?, "receipt").is_some();
            if !retried && flight.receipt.as_ref() != Some(&receipt) {
                return Err("flight receipt mismatch");
            }
            output.insert("receipt_bytes".into(), bytes(&receipt.to_bytes()));
            output.insert(
                "flight".into(),
                obj([
                    ("hash", bytes(&hash)),
                    (
                        "payload",
                        Json::Str(
                            String::from_utf8(flight.bytes)
                                .map_err(|_| "invalid flight payload")?,
                        ),
                    ),
                    ("pubkey", bytes(&flight.pubkey)),
                    ("contract", num(flight.contract_id)),
                    ("attempt", num(flight.attempt_num)),
                    ("mode", num(flight.mode as u8)),
                    ("score", Json::Num(flight.score_s)),
                    ("delegation_mask", num(flight.delegation_mask)),
                ]),
            );
            output.insert(
                "mission_elo".into(),
                Json::Num(
                    engine
                        .store()
                        .load_mission_elo(contract)
                        .map_err(|_| "mission lookup failed")?,
                ),
            );
        }
        if path == "/api/v1/submit"
            || path == "/api/v1/submit-segment"
            || path == "/api/v1/callsign"
        {
            output.insert(
                "player".into(),
                player_json(
                    &engine
                        .store()
                        .load_player(&pk)
                        .map_err(|_| "player lookup failed")?,
                ),
            );
        }
    }
    Ok(Json::Obj(output))
}

fn run(input: &str) -> String {
    let result =
        Json::parse(input)
            .ok_or("invalid JSON")
            .and_then(|root| match get_str(&root, "op") {
                Some("inspect") => inspect(&root),
                Some("route") => route(&root),
                _ => Err("unknown operation"),
            });
    match result {
        Ok(value) => value.to_string(),
        Err(error) => obj([
            ("status", num(500)),
            ("error", Json::Str(error.to_string())),
        ])
        .to_string(),
    }
}

/// Reserve an input buffer; the host writes exactly `len` bytes into exported
/// linear memory and calls `authority_run` synchronously before reusing it.
#[no_mangle]
pub extern "C" fn authority_input(len: u32) -> *mut u8 {
    let mut input = INPUT.lock().unwrap();
    if len as usize > MAX_INPUT {
        return std::ptr::null_mut();
    }
    input.resize(len as usize, 0);
    input.as_mut_ptr()
}

#[no_mangle]
pub extern "C" fn authority_run() -> i32 {
    let mut input = INPUT.lock().unwrap();
    let result = match std::str::from_utf8(&input) {
        Ok(text) => run(text),
        Err(_) => obj([
            ("status", num(500)),
            ("error", Json::Str("invalid UTF-8".into())),
        ])
        .to_string(),
    };
    input.fill(0); // Do not retain the configured signing seed in the buffer.
    *OUTPUT.lock().unwrap() = result.into_bytes();
    1
}

#[no_mangle]
pub extern "C" fn authority_output_len() -> u32 {
    OUTPUT.lock().unwrap().len() as u32
}
#[no_mangle]
pub extern "C" fn authority_output_ptr() -> *const u8 {
    OUTPUT.lock().unwrap().as_ptr()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_snapshots_fail_closed() {
        let seed = "5e".repeat(32);
        let pk = "00".repeat(32);
        let request = format!(
            r#"{{"op":"route","seed":"{seed}","method":"GET","path":"/api/v1/player/{pk}","body":"","now_s":1,"subject":{{"pubkey":"{pk}","contract":101}},"snapshot":{{"player":{{"elo":null,"ranked":0}}}}}}"#
        );
        let result = Json::parse(&run(&request)).unwrap();
        assert_eq!(result.get("status").and_then(Json::as_u32), Some(500));
        assert_eq!(result.get("error").and_then(string), Some("bad player elo"));
    }

    #[test]
    fn absent_player_defaults_to_base_rating() {
        let seed = "5e".repeat(32);
        let pk = "00".repeat(32);
        let request = format!(
            r#"{{"op":"route","seed":"{seed}","method":"GET","path":"/api/v1/player/{pk}","body":"","now_s":1,"subject":{{"pubkey":"{pk}","contract":101}},"snapshot":{{}}}}"#
        );
        let result = Json::parse(&run(&request)).unwrap();
        assert_eq!(result.get("status").and_then(Json::as_u32), Some(200));
        let body = Json::parse(result.get("body").and_then(string).unwrap()).unwrap();
        assert_eq!(body.get("elo").and_then(Json::as_f64), Some(1200.0));
    }
}
