//! Local HTTP adapter for the [`AuthorityEngine`] (WP 1.9).
//!
//! A dependency-free `std::net` HTTP/1.1 server. The router is a pure
//! function of (method, path, body, clock) so the gate can drive it without
//! a socket; the `local` feature adds a blocking accept loop. JSON is the wire
//! format the browser uses in Slice 3; byte arrays travel as lowercase hex.
//!
//! The adapter — not the engine — reads the wall clock and supplies `now_s`,
//! keeping the authority itself reproducible.
//!
//! [`AuthorityEngine`]: crate::engine::AuthorityEngine

use crate::engine::AuthorityEngine;
use crate::protocol::{
    CallsignRequest, CommitRequest, CommitTicket, SegmentCheckpoint, ServerError, SubmitRequest,
    SubmitResponse,
};
use crate::receipt::{ReceiptPayload, ServerReceipt};
use crate::store::Store;
use std::collections::BTreeMap;
#[cfg(feature = "local")]
use std::io::{Read, Write};
#[cfg(feature = "local")]
use std::net::{TcpListener, TcpStream};

pub mod json;
use json::Json;

/// An HTTP response the router produces.
pub struct Response {
    pub status: u16,
    pub body: String,
}

impl Response {
    fn json(status: u16, value: &Json) -> Self {
        Self {
            status,
            body: value.to_string(),
        }
    }
    fn error(err: &ServerError) -> Self {
        let mut obj = BTreeMap::new();
        let storage_failure = matches!(err, ServerError::Storage(_));
        let message = if storage_failure {
            "authority storage failure".to_string()
        } else {
            format!("{err}")
        };
        obj.insert("error".into(), Json::Str(message));
        obj.insert("code".into(), Json::Str(error_code(err).into()));
        Self::json(if storage_failure { 503 } else { 400 }, &Json::Obj(obj))
    }
}

/// The stable machine code for an error, matched by the UI.
pub fn error_code(err: &ServerError) -> &'static str {
    match err {
        ServerError::InvalidClientSignature => "InvalidClientSignature",
        ServerError::InvalidTicketSignature => "InvalidTicketSignature",
        ServerError::UnknownContract(_) => "UnknownContract",
        ServerError::DesignCommitmentMismatch => "DesignCommitmentMismatch",
        ServerError::PredictionCommitmentMismatch => "PredictionCommitmentMismatch",
        ServerError::TicketSubjectMismatch => "TicketSubjectMismatch",
        ServerError::MalformedFdr(_) => "MalformedFdr",
        ServerError::ReplayRejected(_) => "ReplayRejected",
        ServerError::ContractMismatch(_) => "ContractMismatch",
        ServerError::SegmentMismatch(_) => "SegmentMismatch",
        ServerError::ThermalSegmentsUnsupported => "ThermalSegmentsUnsupported",
        ServerError::SegmentNotResumable => "SegmentNotResumable",
        ServerError::SegmentTooLarge => "SegmentTooLarge",
        ServerError::RateLimited => "RateLimited",
        ServerError::CommitmentMissing => "CommitmentMissing",
        ServerError::AttemptModeMismatch => "AttemptModeMismatch",
        ServerError::AttemptAlreadySubmitted => "AttemptAlreadySubmitted",
        ServerError::FdrHashCollision => "FdrHashCollision",
        ServerError::CallsignTaken => "CallsignTaken",
        ServerError::InvalidCallsign => "InvalidCallsign",
        ServerError::Storage(_) => "Storage",
    }
}

/// Route one request. Pure given the clock, so it is socket-free testable.
pub fn route<S: Store>(
    engine: &mut AuthorityEngine<S>,
    method: &str,
    path: &str,
    body: &str,
    now_s: i64,
) -> Response {
    if body.len() > MAX_BODY_BYTES {
        return Response::json(413, &Json::Str("request body too large".into()));
    }
    let (pathname, query) = path.split_once('?').unwrap_or((path, ""));
    let parsed = Json::parse(body);
    match (method, pathname) {
        ("POST", "/api/v1/commit") => {
            let Some(req) = parsed.as_ref().and_then(commit_from_json) else {
                return Response::json(400, &Json::Str("bad commit body".into()));
            };
            match engine.commit(req, now_s) {
                Ok(ticket) => Response::json(200, &ticket_to_json(&ticket)),
                Err(e) => Response::error(&e),
            }
        }
        ("POST", "/api/v1/submit") | ("POST", "/api/v1/submit-segment") => {
            let Some(req) = parsed.as_ref().and_then(submit_from_json) else {
                return Response::json(400, &Json::Str("bad submit body".into()));
            };
            if pathname == "/api/v1/submit-segment" && req.segments.is_none() {
                return Response::error(&ServerError::SegmentMismatch(0));
            }
            match engine.submit(req, now_s) {
                Ok(resp) => Response::json(200, &response_to_json(&resp)),
                Err(e) => Response::error(&e),
            }
        }
        ("POST", "/api/v1/callsign") => {
            let Some(req) = parsed.as_ref().and_then(callsign_from_json) else {
                return Response::json(400, &Json::Str("bad callsign body".into()));
            };
            match engine.register_callsign(req) {
                Ok(name) => {
                    let mut o = BTreeMap::new();
                    o.insert("ok".into(), Json::Bool(true));
                    o.insert("name".into(), Json::Str(name));
                    Response::json(200, &Json::Obj(o))
                }
                Err(e) => Response::error(&e),
            }
        }
        ("GET", "/api/v1/leaderboard") => leaderboard(engine, query),
        ("GET", p) if p.starts_with("/api/v1/verify/") => {
            let hash = &p["/api/v1/verify/".len()..];
            let Some(hash) = parse_hash(hash) else {
                return Response::json(400, &Json::Str("bad hash".into()));
            };
            match engine.store().load_fdr(&hash) {
                Ok(Some(fdr)) => {
                    // Do not return a corrupt payload alongside a valid
                    // receipt. Authenticate both envelopes and all index
                    // bindings before making a public replay available.
                    if let Err(error) = fdr.verified_record(&hash, &engine.server_pk()) {
                        return Response::error(&error);
                    }
                    let Some(receipt) = fdr.receipt.as_ref() else {
                        return Response::error(&ServerError::Storage(
                            "stored receipt is missing".into(),
                        ));
                    };
                    // Native SQLite keeps the public receipt and idempotency
                    // receipt separately. Never serve conflicting copies (or
                    // one whose attempt row disappeared) as a healthy flight.
                    match engine
                        .store()
                        .load_receipt(&fdr.pubkey, fdr.contract_id, fdr.attempt_num)
                    {
                        Ok(Some(indexed)) if indexed == *receipt => {}
                        Ok(_) => {
                            return Response::error(&ServerError::Storage(
                                "stored receipt copies disagree".into(),
                            ));
                        }
                        Err(error) => return Response::error(&error),
                    }
                    let Ok(payload) = String::from_utf8(fdr.bytes) else {
                        return Response::error(&ServerError::Storage(
                            "stored record is invalid".into(),
                        ));
                    };
                    let mut o = BTreeMap::new();
                    o.insert("fdr_payload".into(), Json::Str(payload));
                    o.insert("receipt".into(), receipt_to_json(receipt));
                    Response::json(200, &Json::Obj(o))
                }
                Ok(None) => Response::json(404, &Json::Str("not found".into())),
                Err(e) => Response::error(&e),
            }
        }
        ("GET", p) if p.starts_with("/api/v1/player/") => {
            let pk = &p["/api/v1/player/".len()..];
            let Some(pk) = parse_hash(pk) else {
                return Response::json(400, &Json::Str("bad public key".into()));
            };
            match engine.store().load_player(&pk) {
                Ok(player) => {
                    let mut o = BTreeMap::new();
                    o.insert("pubkey".into(), hex(&pk));
                    o.insert("elo".into(), Json::Num(player.elo));
                    o.insert("ranked_count".into(), Json::Num(player.ranked_count as f64));
                    o.insert(
                        "callsign".into(),
                        player.callsign.map(Json::Str).unwrap_or(Json::Null),
                    );
                    Response::json(200, &Json::Obj(o))
                }
                Err(e) => Response::error(&e),
            }
        }
        _ => Response::json(404, &Json::Str("not found".into())),
    }
}

/// The public board never contains practice flights. Pagination is bounded so
/// neither a query nor a large database requires an unbounded response.
fn leaderboard<S: Store>(engine: &AuthorityEngine<S>, query: &str) -> Response {
    let (mut contract, mut page) = (None, 1u32);
    let (mut seen_contract, mut seen_page) = (false, false);
    for part in query.split('&').filter(|s| !s.is_empty()) {
        let Some((key, value)) = part.split_once('=') else {
            return Response::json(400, &Json::Str("bad query".into()));
        };
        match key {
            "contract" if !seen_contract => {
                seen_contract = true;
                let Ok(id) = value.parse::<u32>() else {
                    return Response::json(400, &Json::Str("bad contract".into()));
                };
                if odd_contracts::Contract::get(id).is_none() {
                    return Response::json(400, &Json::Str("unknown contract".into()));
                }
                contract = Some(id);
            }
            "page" if !seen_page => {
                seen_page = true;
                let Ok(n) = value.parse::<u32>() else {
                    return Response::json(400, &Json::Str("bad page".into()));
                };
                if !(1..=10_000).contains(&n) {
                    return Response::json(400, &Json::Str("bad page".into()));
                }
                page = n;
            }
            _ => return Response::json(400, &Json::Str("bad query".into())),
        }
    }
    match engine.store().leaderboard(contract, (page - 1) * 25, 25) {
        Ok(rows) => Response::json(
            200,
            &Json::Arr(
                rows.into_iter()
                    .map(|row| {
                        let mut o = BTreeMap::new();
                        o.insert(
                            "callsign".into(),
                            row.callsign.map(Json::Str).unwrap_or(Json::Null),
                        );
                        o.insert("pubkey".into(), hex(&row.pubkey));
                        o.insert("elo".into(), Json::Num(row.elo));
                        o.insert("score_s".into(), Json::Num(row.score_s));
                        o.insert(
                            "delegation_mask".into(),
                            Json::Num(row.delegation_mask as f64),
                        );
                        o.insert("fdr_hash".into(), hex(&row.fdr_hash));
                        Json::Obj(o)
                    })
                    .collect(),
            ),
        ),
        Err(e) => Response::error(&e),
    }
}

fn parse_hash(value: &str) -> Option<[u8; 32]> {
    odd_report::hex::decode(value)?.try_into().ok()
}

fn callsign_from_json(v: &Json) -> Option<CallsignRequest> {
    let Json::Str(name) = v.get("name")? else {
        return None;
    };
    Some(CallsignRequest {
        name: name.clone(),
        pubkey: array32(v.get("pubkey"))?,
        signature: array64(v.get("sig"))?,
    })
}

// --- protocol ↔ JSON -------------------------------------------------------

fn hex(bytes: &[u8]) -> Json {
    Json::Str(odd_report::hex::encode(bytes))
}

fn unhex(v: Option<&Json>) -> Option<Vec<u8>> {
    match v {
        Some(Json::Str(s)) => odd_report::hex::decode(s),
        _ => None,
    }
}

fn array32(v: Option<&Json>) -> Option<[u8; 32]> {
    let bytes = unhex(v)?;
    bytes.try_into().ok()
}

fn array64(v: Option<&Json>) -> Option<[u8; 64]> {
    let bytes = unhex(v)?;
    bytes.try_into().ok()
}

fn commit_from_json(v: &Json) -> Option<CommitRequest> {
    Some(CommitRequest {
        contract_id: v.get("contract_id")?.as_u32()?,
        pubkey: array32(v.get("pubkey"))?,
        design_hash: array32(v.get("design_hash"))?,
        prediction_hash: array32(v.get("prediction_hash"))?,
        client_sig: array64(v.get("client_sig"))?,
    })
}

/// Serialize a commit request the way the browser posts it.
pub fn commit_to_json(req: &CommitRequest) -> Json {
    let mut o = BTreeMap::new();
    o.insert("contract_id".into(), Json::Num(req.contract_id as f64));
    o.insert("pubkey".into(), hex(&req.pubkey));
    o.insert("design_hash".into(), hex(&req.design_hash));
    o.insert("prediction_hash".into(), hex(&req.prediction_hash));
    o.insert("client_sig".into(), hex(&req.client_sig));
    Json::Obj(o)
}

/// Serialize a submit request, with or without a segment chain.
pub fn submit_to_json(req: &SubmitRequest) -> Json {
    let mut o = BTreeMap::new();
    o.insert("fdr_payload".into(), Json::Str(req.fdr_payload.clone()));
    o.insert("ticket".into(), ticket_to_json(&req.ticket));
    o.insert(
        "segments".into(),
        match &req.segments {
            Some(segs) => Json::Arr(segs.iter().map(checkpoint_to_json).collect()),
            None => Json::Null,
        },
    );
    Json::Obj(o)
}

fn ticket_to_json(t: &CommitTicket) -> Json {
    let mut o = BTreeMap::new();
    o.insert("contract_id".into(), Json::Num(t.contract_id as f64));
    o.insert("pubkey".into(), hex(&t.pubkey));
    o.insert("attempt_num".into(), Json::Num(t.attempt_num as f64));
    o.insert("mode".into(), Json::Num(t.mode as u8 as f64));
    o.insert("design_hash".into(), hex(&t.design_hash));
    o.insert("prediction_hash".into(), hex(&t.prediction_hash));
    o.insert("server_time_s".into(), Json::Num(t.server_time_s as f64));
    o.insert("server_sig".into(), hex(&t.server_sig));
    Json::Obj(o)
}

pub fn ticket_from_json(v: &Json) -> Option<CommitTicket> {
    Some(CommitTicket {
        contract_id: v.get("contract_id")?.as_u32()?,
        pubkey: array32(v.get("pubkey"))?,
        attempt_num: v.get("attempt_num")?.as_u32()?,
        mode: crate::protocol::AttemptMode::from_u8(u8::try_from(v.get("mode")?.as_u32()?).ok()?)?,
        design_hash: array32(v.get("design_hash"))?,
        prediction_hash: array32(v.get("prediction_hash"))?,
        server_time_s: v.get("server_time_s")?.as_i64()?,
        server_sig: array64(v.get("server_sig"))?,
    })
}

/// Serialize one checkpoint in the public wire format.
pub fn checkpoint_to_json(c: &SegmentCheckpoint) -> Json {
    let mut o = BTreeMap::new();
    o.insert("t_us".into(), Json::Num(c.t_us as f64));
    o.insert(
        "state_vector".into(),
        Json::Arr(c.state_vector.iter().map(|v| Json::Num(*v)).collect()),
    );
    o.insert("prop_kg".into(), Json::Num(c.prop_kg));
    o.insert(
        "tank_temps".into(),
        Json::Arr(c.tank_temps.iter().map(|v| Json::Num(*v)).collect()),
    );
    o.insert("cum_hash".into(), hex(&c.cum_hash));
    // The chain link. Always emitted, `null` at the head of a chain: a segment
    // that names its predecessor cannot be re-used under a different chain, so
    // omitting it here would silently disable that binding for every HTTP
    // submission.
    o.insert(
        "prev_seg_hash".into(),
        match &c.prev_seg_hash {
            Some(hash) => hex(hash),
            None => Json::Null,
        },
    );
    // The v4 resumable state. Emitted only when the checkpoint actually carries
    // it, so v1–v3 chains keep the exact wire form they always had: a legacy
    // checkpoint parses back as `tank_states: None`, `cooler_joules: 0.0`,
    // `vent_count: 0`, which is what it was.
    if let Some(tanks) = c.tank_states {
        o.insert(
            "tank_states".into(),
            Json::Arr(
                tanks
                    .iter()
                    .map(|row| Json::Arr(row.iter().map(|v| Json::Num(*v)).collect()))
                    .collect(),
            ),
        );
        o.insert("cooler_joules".into(), Json::Num(c.cooler_joules));
        o.insert("vent_count".into(), Json::Num(c.vent_count as f64));
    }
    Json::Obj(o)
}

fn checkpoint_from_json(v: &Json) -> Option<SegmentCheckpoint> {
    let sv = v.get("state_vector")?.as_array()?;
    let tt = v.get("tank_temps")?.as_array()?;
    if sv.len() != 7 || tt.len() != 2 {
        return None;
    }
    let mut state_vector = [0.0; 7];
    for (i, item) in sv.iter().enumerate() {
        state_vector[i] = item.as_f64()?;
    }
    Some(SegmentCheckpoint {
        t_us: v.get("t_us")?.as_i64()?,
        state_vector,
        prop_kg: v.get("prop_kg")?.as_f64()?,
        tank_temps: [tt[0].as_f64()?, tt[1].as_f64()?],
        cum_hash: array32(v.get("cum_hash"))?,
        prev_seg_hash: match v.get("prev_seg_hash") {
            Some(Json::Null) | None => None,
            Some(value) => Some(array32(Some(value))?),
        },
        tank_states: match v.get("tank_states") {
            Some(Json::Arr(rows)) => {
                if rows.len() != 2 {
                    return None;
                }
                let mut out = [[0.0; 3]; 2];
                for (row, slot) in rows.iter().zip(out.iter_mut()) {
                    let cells = row.as_array()?;
                    if cells.len() != 3 {
                        return None;
                    }
                    for (cell, value) in cells.iter().zip(slot.iter_mut()) {
                        *value = cell.as_f64()?;
                    }
                }
                Some(out)
            }
            _ => None,
        },
        cooler_joules: v.get("cooler_joules").map_or(0.0, |j| {
            j.as_f64()
                .filter(|v| v.is_finite() && *v >= 0.0)
                .unwrap_or(0.0)
        }),
        vent_count: v.get("vent_count").and_then(|j| j.as_u32()).unwrap_or(0),
    })
}

fn submit_from_json(v: &Json) -> Option<SubmitRequest> {
    let fdr_payload = match v.get("fdr_payload")? {
        Json::Str(s) => s.clone(),
        _ => return None,
    };
    let ticket = ticket_from_json(v.get("ticket")?)?;
    let segments = match v.get("segments") {
        Some(Json::Arr(items)) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(checkpoint_from_json(item)?);
            }
            Some(out)
        }
        None | Some(Json::Null) => None,
        _ => return None,
    };
    Some(SubmitRequest {
        fdr_payload,
        ticket,
        segments,
    })
}

fn receipt_to_json(r: &ServerReceipt) -> Json {
    let p = &r.payload;
    let mut o = BTreeMap::new();
    o.insert("schema_v3".into(), Json::Num(p.schema_v3 as f64));
    o.insert("fdr_hash".into(), hex(&p.fdr_hash));
    o.insert("pubkey".into(), hex(&p.pubkey));
    o.insert("contract_id".into(), Json::Num(p.contract_id as f64));
    o.insert("attempt_num".into(), Json::Num(p.attempt_num as f64));
    o.insert("mode".into(), Json::Num(p.mode as u8 as f64));
    o.insert("score_s".into(), Json::Num(p.score_s()));
    o.insert("elo_before".into(), Json::Num(p.elo_before()));
    o.insert("elo_after".into(), Json::Num(p.elo_after()));
    o.insert("mission_elo_after".into(), Json::Num(p.mission_elo_after()));
    o.insert("timestamp_s".into(), Json::Num(p.timestamp_s as f64));
    o.insert("server_sig".into(), hex(&r.server_sig));
    Json::Obj(o)
}

fn response_to_json(r: &SubmitResponse) -> Json {
    let mut o = BTreeMap::new();
    o.insert("accepted".into(), Json::Bool(r.accepted));
    o.insert("mode".into(), Json::Num(r.mode as u8 as f64));
    o.insert("attempt_num".into(), Json::Num(r.attempt_num as f64));
    o.insert("score_s".into(), Json::Num(r.score_s));
    o.insert("elo_before".into(), Json::Num(r.elo_before));
    o.insert("elo_after".into(), Json::Num(r.elo_after));
    o.insert("mission_elo_after".into(), Json::Num(r.mission_elo_after));
    o.insert("fdr_hash".into(), hex(&r.fdr_hash));
    o.insert(
        "receipt".into(),
        r.receipt
            .as_ref()
            .map(receipt_to_json)
            .unwrap_or(Json::Null),
    );
    Json::Obj(o)
}

/// Parse a receipt back from the JSON the server emits (the browser does the
/// equivalent). Exposed so the round-trip is testable.
pub fn receipt_from_json(v: &Json) -> Option<ServerReceipt> {
    let payload = ReceiptPayload {
        schema_v3: u16::try_from(v.get("schema_v3")?.as_u32()?)
            .ok()
            .filter(|v| *v == crate::receipt::RECEIPT_SCHEMA_V3)?,
        fdr_hash: array32(v.get("fdr_hash"))?,
        pubkey: array32(v.get("pubkey"))?,
        contract_id: v.get("contract_id")?.as_u32()?,
        attempt_num: v.get("attempt_num")?.as_u32()?,
        mode: crate::protocol::AttemptMode::from_u8(u8::try_from(v.get("mode")?.as_u32()?).ok()?)?,
        score_s_bits: v.get("score_s")?.as_f64()?.to_bits(),
        elo_before_bits: v.get("elo_before")?.as_f64()?.to_bits(),
        elo_after_bits: v.get("elo_after")?.as_f64()?.to_bits(),
        mission_elo_after_bits: v.get("mission_elo_after")?.as_f64()?.to_bits(),
        timestamp_s: v.get("timestamp_s")?.as_i64()?,
    };
    Some(ServerReceipt {
        payload,
        server_sig: array64(v.get("server_sig"))?,
    })
}

// --- blocking TCP server ---------------------------------------------------

/// A bounded request is enough for a flight and its checkpoint chain.
pub const MAX_BODY_BYTES: usize = 256 * 1024;
#[cfg(feature = "local")]
const MAX_HEADER_BYTES: usize = 16 * 1024;

#[cfg(feature = "local")]
fn read_request(stream: &mut TcpStream) -> Result<(String, String, String), u16> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let header_end = loop {
        let n = stream.read(&mut chunk).map_err(|_| 400u16)?;
        if n == 0 {
            return Err(400);
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            if pos > MAX_HEADER_BYTES {
                return Err(413);
            }
            break pos;
        }
        if buf.len() > MAX_HEADER_BYTES {
            return Err(413);
        }
    };
    let header = std::str::from_utf8(&buf[..header_end]).map_err(|_| 400u16)?;
    let mut lines = header.split("\r\n");
    let mut parts = lines.next().ok_or(400u16)?.split_whitespace();
    let method = parts.next().ok_or(400u16)?.to_string();
    let path = parts.next().ok_or(400u16)?.to_string();
    if parts.next().is_none() || parts.next().is_some() {
        return Err(400);
    }
    let mut content_length = None;
    for line in lines {
        let (name, value) = line.split_once(':').ok_or(400u16)?;
        if name.eq_ignore_ascii_case("transfer-encoding") {
            return Err(400);
        }
        if name.eq_ignore_ascii_case("content-length") {
            if content_length.is_some() {
                return Err(400);
            }
            content_length = Some(value.trim().parse::<usize>().map_err(|_| 400u16)?);
        }
    }
    if method == "POST" && content_length.is_none() {
        return Err(400);
    }
    let length = content_length.unwrap_or(0);
    if length > MAX_BODY_BYTES {
        return Err(413);
    }
    let mut body = buf[header_end + 4..].to_vec();
    while body.len() < length {
        let n = stream.read(&mut chunk).map_err(|_| 400u16)?;
        if n == 0 {
            return Err(400);
        }
        body.extend_from_slice(&chunk[..n]);
    }
    body.truncate(length);
    Ok((method, path, String::from_utf8(body).map_err(|_| 400u16)?))
}

#[cfg(feature = "local")]
fn write_response(stream: &mut TcpStream, resp: &Response) -> std::io::Result<()> {
    let reason = match resp.status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        413 => "Payload Too Large",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "Error",
    };
    let head = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n",
        resp.status,
        reason,
        resp.body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(resp.body.as_bytes())?;
    stream.flush()
}

/// Serve the authority on `addr` until the process is stopped. Single
/// threaded: one request holds the engine exclusively, which is exactly the
/// atomicity the storage port requires. The adapter reads the wall clock.
#[cfg(feature = "local")]
pub fn serve<S: Store>(addr: &str, engine: AuthorityEngine<S>) -> std::io::Result<()> {
    serve_listener(TcpListener::bind(addr)?, engine)
}

/// Serve on an already-bound listener (a test binds port 0 to discover it).
#[cfg(feature = "local")]
pub fn serve_listener<S: Store>(
    listener: TcpListener,
    mut engine: AuthorityEngine<S>,
) -> std::io::Result<()> {
    for stream in listener.incoming() {
        let Ok(mut stream) = stream else { continue };
        let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(5)));
        let (method, path, body) = match read_request(&mut stream) {
            Ok(request) => request,
            Err(status) => {
                let _ = write_response(
                    &mut stream,
                    &Response::json(status, &Json::Str("invalid request".into())),
                );
                continue;
            }
        };
        let now_s = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let resp = route(&mut engine, &method, &path, &body, now_s);
        let _ = write_response(&mut stream, &resp);
    }
    Ok(())
}

#[cfg(test)]
mod v4_wire_tests {
    use super::*;

    /// `checkpoint_to_json` must emit every field `checkpoint_from_json`
    /// reads back. A round trip is the only thing that proves it.
    #[test]
    fn v4_checkpoint_survives_the_wire() {
        let mut cp = SegmentCheckpoint::legacy(
            86_400 * 1_000_000,
            [7.0e6, 1.0, 2.0, 0.0, 7_600.0, 3.0, 5_182.1 + 6_400.0],
            6_400.0,
            [0.0, 0.0],
            [7u8; 32],
        );
        cp.prev_seg_hash = Some([9u8; 32]);
        cp.tank_states = Some([[20.4, 5_376.0, 12.5], [21.0, 1_024.0, 3.25]]);
        cp.cooler_joules = 1.5e9;
        cp.vent_count = 4;

        let wire = checkpoint_to_json(&cp);
        let back = checkpoint_from_json(&wire)
            .expect("a checkpoint the authority itself wrote must parse");
        assert_eq!(
            back, cp,
            "the wire format must preserve the whole checkpoint"
        );
        assert!(back.is_resumable());
    }

    /// The fix above must not change the bytes a v1–v3 chain already uses.
    /// A legacy checkpoint has no v4 block, so it must not gain one.
    #[test]
    fn legacy_checkpoints_keep_their_wire_form() {
        let cp = SegmentCheckpoint::legacy(1_000_000, [1.0; 7], 2.0, [0.0, 0.0], [3u8; 32]);
        let wire = checkpoint_to_json(&cp);
        let Json::Obj(map) = &wire else {
            panic!("a checkpoint is an object");
        };
        for absent in ["tank_states", "cooler_joules", "vent_count"] {
            assert!(
                !map.contains_key(absent),
                "{absent} must be omitted for a chain-only checkpoint"
            );
        }
        assert_eq!(checkpoint_from_json(&wire), Some(cp));
    }
}
