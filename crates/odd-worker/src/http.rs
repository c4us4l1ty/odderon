//! Local HTTP adapter for the [`AuthorityEngine`] (WP 1.9).
//!
//! A dependency-free `std::net` HTTP/1.1 server. The router is a pure
//! function of (method, path, body, clock) so the gate can drive it without
//! a socket; [`serve`] wraps it in a blocking accept loop. JSON is the wire
//! format the browser uses in Slice 3; byte arrays travel as lowercase hex.
//!
//! The adapter — not the engine — reads the wall clock and supplies `now_s`,
//! keeping the authority itself reproducible.
//!
//! [`AuthorityEngine`]: crate::engine::AuthorityEngine

use crate::engine::AuthorityEngine;
use crate::protocol::{
    CommitRequest, CommitTicket, SegmentCheckpoint, ServerError, SubmitRequest, SubmitResponse,
};
use crate::receipt::{ReceiptPayload, ServerReceipt};
use crate::store::Store;
use std::collections::BTreeMap;
use std::io::{Read, Write};
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
        obj.insert("error".into(), Json::Str(format!("{err}")));
        obj.insert("code".into(), Json::Str(error_code(err).into()));
        Self::json(400, &Json::Obj(obj))
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
    let parsed = Json::parse(body);
    match (method, path) {
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
            match engine.submit(req, now_s) {
                Ok(resp) => Response::json(200, &response_to_json(&resp)),
                Err(e) => Response::error(&e),
            }
        }
        _ => Response::json(404, &Json::Str("not found".into())),
    }
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
        mode: crate::protocol::AttemptMode::from_u8(v.get("mode")?.as_u32()? as u8)?,
        design_hash: array32(v.get("design_hash"))?,
        prediction_hash: array32(v.get("prediction_hash"))?,
        server_time_s: v.get("server_time_s")?.as_i64()?,
        server_sig: array64(v.get("server_sig"))?,
    })
}

fn checkpoint_to_json(c: &SegmentCheckpoint) -> Json {
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
        _ => None,
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
        schema_v3: v.get("schema_v3")?.as_u32()? as u16,
        fdr_hash: array32(v.get("fdr_hash"))?,
        pubkey: array32(v.get("pubkey"))?,
        contract_id: v.get("contract_id")?.as_u32()?,
        attempt_num: v.get("attempt_num")?.as_u32()?,
        mode: crate::protocol::AttemptMode::from_u8(v.get("mode")?.as_u32()? as u8)?,
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

fn read_request(stream: &mut TcpStream) -> Option<(String, String, String)> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    // Read until the header terminator, then read the declared body length.
    let header_end = loop {
        let n = stream.read(&mut chunk).ok()?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break pos;
        }
        if buf.len() > 1_048_576 {
            return None;
        }
    };
    let header = String::from_utf8_lossy(&buf[..header_end]).into_owned();
    let mut lines = header.split("\r\n");
    let request_line = lines.next()?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next()?.to_string();
    let path = parts.next()?.to_string();
    let content_length = lines
        .find_map(|l| {
            let (name, value) = l.split_once(':')?;
            if name.eq_ignore_ascii_case("content-length") {
                value.trim().parse::<usize>().ok()
            } else {
                None
            }
        })
        .unwrap_or(0);
    let mut body = buf[header_end + 4..].to_vec();
    while body.len() < content_length {
        let n = stream.read(&mut chunk).ok()?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..n]);
    }
    body.truncate(content_length);
    Some((method, path, String::from_utf8_lossy(&body).into_owned()))
}

fn write_response(stream: &mut TcpStream, resp: &Response) -> std::io::Result<()> {
    let reason = match resp.status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
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
pub fn serve<S: Store>(addr: &str, engine: AuthorityEngine<S>) -> std::io::Result<()> {
    serve_listener(TcpListener::bind(addr)?, engine)
}

/// Serve on an already-bound listener (a test binds port 0 to discover it).
pub fn serve_listener<S: Store>(
    listener: TcpListener,
    mut engine: AuthorityEngine<S>,
) -> std::io::Result<()> {
    for stream in listener.incoming() {
        let Ok(mut stream) = stream else { continue };
        let Some((method, path, body)) = read_request(&mut stream) else {
            continue;
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
