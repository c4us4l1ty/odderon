//! Cloudflare Workers D1/R2 deployment adapter for the odd-worker authority.
//!
//! The pure [`AuthorityEngine`] is unchanged: this adapter loads a snapshot
//! of the relevant rows from D1 into a [`MemoryStore`], runs the engine
//! synchronously, then flushes the changed rows back to D1 and the FDR blob
//! to R2 in a batch. D1 is SQLite, so the schema mirrors the local adapter
//! (hashes and keys are stored as hex TEXT here because D1 binds strings
//! most reliably; the storage port hides the representation).
//!
//! Built in CI with network (`workers-rs` is not vendored for the offline
//! gate). The anti-cheat oracles run against the identical engine via the
//! local adapter, so this shell adds no logic that could cheat.

use odd_worker::{
    AuthorityEngine, CommitRequest, CommitTicket, MemoryStore, PlayerRow, SegmentCheckpoint,
    ServerError, ServerKey, Store, SubmitRequest,
};
use serde::{Deserialize, Serialize};
use worker::*;

/// The operator configures only the 32-byte server seed (hex) as a secret.
fn server_key(env: &Env) -> Result<ServerKey> {
    let secret = env.secret("SERVER_SEED").map_err(|e| e)?;
    let bytes = hex::decode(secret.to_string()).map_err(|_| Error::RustError("bad SERVER_SEED".into()))?;
    let seed: [u8; 32] = bytes
        .try_into()
        .map_err(|_| Error::RustError("SERVER_SEED must be 32 bytes".into()))?;
    Ok(ServerKey::from_seed(seed))
}

fn now_s() -> i64 {
    (js_sys::Date::now() / 1000.0) as i64
}

fn to_worker(e: ServerError) -> Error {
    Error::RustError(format!("{e}"))
}

// --- JSON wire shapes (hex for byte arrays) ---------------------------------

#[derive(Deserialize)]
struct CommitBody {
    contract_id: u32,
    pubkey: String,
    design_hash: String,
    prediction_hash: String,
    client_sig: String,
}

#[derive(Serialize, Deserialize)]
struct TicketBody {
    contract_id: u32,
    pubkey: String,
    attempt_num: u32,
    mode: u8,
    design_hash: String,
    prediction_hash: String,
    server_time_s: i64,
    server_sig: String,
}

#[derive(Deserialize)]
struct CheckpointBody {
    t_us: i64,
    state_vector: [f64; 7],
    prop_kg: f64,
    tank_temps: [f64; 2],
    cum_hash: String,
}

#[derive(Deserialize)]
struct SubmitBody {
    fdr_payload: String,
    ticket: TicketBody,
    #[serde(default)]
    segments: Option<Vec<CheckpointBody>>,
}

fn h32(s: &str) -> Result<[u8; 32]> {
    hex::decode(s)
        .ok()
        .and_then(|b| <[u8; 32]>::try_into(b).ok())
        .ok_or_else(|| Error::RustError("expected 32-byte hex".into()))
}
fn h64(s: &str) -> Result<[u8; 64]> {
    hex::decode(s)
        .ok()
        .and_then(|b| <[u8; 64]>::try_into(b).ok())
        .ok_or_else(|| Error::RustError("expected 64-byte hex".into()))
}

impl TryFrom<CommitBody> for CommitRequest {
    type Error = Error;
    fn try_from(b: CommitBody) -> Result<Self> {
        Ok(CommitRequest {
            contract_id: b.contract_id,
            pubkey: h32(&b.pubkey)?,
            design_hash: h32(&b.design_hash)?,
            prediction_hash: h32(&b.prediction_hash)?,
            client_sig: h64(&b.client_sig)?,
        })
    }
}

impl From<&CommitTicket> for TicketBody {
    fn from(t: &CommitTicket) -> Self {
        TicketBody {
            contract_id: t.contract_id,
            pubkey: hex::encode(t.pubkey),
            attempt_num: t.attempt_num,
            mode: t.mode as u8,
            design_hash: hex::encode(t.design_hash),
            prediction_hash: hex::encode(t.prediction_hash),
            server_time_s: t.server_time_s,
            server_sig: hex::encode(t.server_sig),
        }
    }
}

impl TryFrom<TicketBody> for CommitTicket {
    type Error = Error;
    fn try_from(b: TicketBody) -> Result<Self> {
        Ok(CommitTicket {
            contract_id: b.contract_id,
            pubkey: h32(&b.pubkey)?,
            attempt_num: b.attempt_num,
            mode: odd_worker::AttemptMode::from_u8(b.mode)
                .ok_or_else(|| Error::RustError("bad mode".into()))?,
            design_hash: h32(&b.design_hash)?,
            prediction_hash: h32(&b.prediction_hash)?,
            server_time_s: b.server_time_s,
            server_sig: h64(&b.server_sig)?,
        })
    }
}

impl TryFrom<SubmitBody> for SubmitRequest {
    type Error = Error;
    fn try_from(b: SubmitBody) -> Result<Self> {
        let segments = match b.segments {
            None => None,
            Some(items) => {
                let mut out = Vec::with_capacity(items.len());
                for c in items {
                    out.push(SegmentCheckpoint {
                        t_us: c.t_us,
                        state_vector: c.state_vector,
                        prop_kg: c.prop_kg,
                        tank_temps: c.tank_temps,
                        cum_hash: h32(&c.cum_hash)?,
                    });
                }
                Some(out)
            }
        };
        Ok(SubmitRequest {
            fdr_payload: b.fdr_payload,
            ticket: b.ticket.try_into()?,
            segments,
        })
    }
}

// --- D1 snapshot load / flush -----------------------------------------------

/// Load just the rows a commit or submit for `(pubkey, contract)` can touch.
async fn load_snapshot(db: &D1Database, pubkey: &[u8; 32], contract: u32) -> Result<MemoryStore> {
    let mut store = MemoryStore::new();
    let pk = hex::encode(pubkey);

    // Attempt counter.
    let row: Option<serde_json::Value> = db
        .prepare("SELECT n FROM attempts WHERE pubkey=?1 AND contract=?2")
        .bind(&[pk.clone().into(), (contract as i64).into()])?
        .first(None)
        .await?;
    if let Some(v) = row {
        let n = v.get("n").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
        store.seed_attempt(pubkey, contract, n);
    }

    // Player.
    let row: Option<serde_json::Value> = db
        .prepare("SELECT elo, ranked, callsign FROM players WHERE pubkey=?1")
        .bind(&[pk.clone().into()])?
        .first(None)
        .await?;
    if let Some(v) = row {
        store.save_player(
            pubkey,
            &PlayerRow {
                elo: v.get("elo").and_then(|x| x.as_f64()).unwrap_or(1200.0),
                ranked_count: v.get("ranked").and_then(|x| x.as_u64()).unwrap_or(0) as u32,
                callsign: v.get("callsign").and_then(|x| x.as_str()).map(String::from),
            },
        ).map_err(to_worker)?;
    }

    // Mission rating.
    let row: Option<serde_json::Value> = db
        .prepare("SELECT elo FROM missions WHERE contract=?1")
        .bind(&[(contract as i64).into()])?
        .first(None)
        .await?;
    if let Some(v) = row {
        if let Some(elo) = v.get("elo").and_then(|x| x.as_f64()) {
            store.save_mission_elo(contract, elo).map_err(to_worker)?;
        }
    }

    // Prior receipts for this subject (idempotent re-submit).
    let receipts = db
        .prepare("SELECT attempt, receipt FROM receipts WHERE pubkey=?1 AND contract=?2")
        .bind(&[pk.clone().into(), (contract as i64).into()])?
        .all()
        .await?
        .results::<serde_json::Value>()?;
    for v in receipts {
        let attempt = v.get("attempt").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
        if let Some(blob) = v.get("receipt").and_then(|x| x.as_str()) {
            if let Ok(bytes) = hex::decode(blob) {
                if let Some(r) = odd_worker::ServerReceipt::from_bytes(&bytes) {
                    store.save_receipt(pubkey, contract, attempt, &r).map_err(to_worker)?;
                }
            }
        }
    }
    Ok(store)
}

/// Persist the rows a commit changed: the atomic counter and the commitment.
async fn flush_commit(db: &D1Database, store: &MemoryStore, ticket: &CommitTicket) -> Result<()> {
    let pk = hex::encode(ticket.pubkey);
    // The counter is written atomically; D1 serializes the upsert.
    db.prepare(
        "INSERT INTO attempts (pubkey, contract, n) VALUES (?1, ?2, ?3)
         ON CONFLICT(pubkey, contract) DO UPDATE SET n=?3",
    )
    .bind(&[pk.clone().into(), (ticket.contract_id as i64).into(), (ticket.attempt_num as i64).into()])?
    .run()
    .await?;
    let commitment = store
        .load_commitment(&ticket.pubkey, ticket.contract_id, ticket.attempt_num)
        .map_err(to_worker)?
        .ok_or_else(|| Error::RustError("commitment missing after commit".into()))?;
    db.prepare(
        "INSERT OR REPLACE INTO commitments
         (pubkey, contract, attempt, design_hash, prediction_hash, mode, server_time)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )
    .bind(&[
        pk.into(),
        (ticket.contract_id as i64).into(),
        (ticket.attempt_num as i64).into(),
        hex::encode(commitment.design_hash).into(),
        hex::encode(commitment.prediction_hash).into(),
        (commitment.mode as u8 as i64).into(),
        commitment.server_time_s.into(),
    ])?
    .run()
    .await?;
    Ok(())
}

/// Persist a submit: player, mission, FDR (R2 + index), receipt.
async fn flush_submit(
    db: &D1Database,
    bucket: &Bucket,
    store: &MemoryStore,
    resp: &odd_worker::SubmitResponse,
    pubkey: &[u8; 32],
    contract: u32,
    attempt: u32,
) -> Result<()> {
    let pk = hex::encode(pubkey);
    let player = store.load_player(pubkey).map_err(to_worker)?;
    let mission = store.load_mission_elo(contract).map_err(to_worker)?;
    let receipt = store
        .load_receipt(pubkey, contract, attempt)
        .map_err(to_worker)?
        .ok_or_else(|| Error::RustError("receipt missing after submit".into()))?;
    let fdr = store
        .load_fdr(&resp.fdr_hash)
        .map_err(to_worker)?
        .ok_or_else(|| Error::RustError("fdr missing after submit".into()))?;

    // R2 holds the signed FDR payload by content hash (the QR target).
    bucket
        .put(hex::encode(resp.fdr_hash), Data::Bytes(fdr.bytes.clone()))
        .execute()
        .await?;

    // D1 batch: rating, mission, receipt and the FDR index row.
    let receipt_hex = hex::encode(receipt.to_bytes());
    let player_stmt = db
        .prepare("INSERT OR REPLACE INTO players (pubkey, elo, ranked, callsign) VALUES (?1, ?2, ?3, ?4)")
        .bind(&[
            pk.clone().into(),
            player.elo.into(),
            (player.ranked_count as i64).into(),
            player.callsign.clone().map(|s| s.into()).unwrap_or(wasm_bindgen::JsValue::NULL),
        ])?;
    let mission_stmt = db
        .prepare("INSERT OR REPLACE INTO missions (contract, elo) VALUES (?1, ?2)")
        .bind(&[(contract as i64).into(), mission.into()])?;
    let receipt_stmt = db
        .prepare("INSERT OR REPLACE INTO receipts (pubkey, contract, attempt, receipt) VALUES (?1, ?2, ?3, ?4)")
        .bind(&[pk.clone().into(), (contract as i64).into(), (attempt as i64).into(), receipt_hex.into()])?;
    let fdr_stmt = db
        .prepare(
            "INSERT OR REPLACE INTO fdrs (hash, pubkey, contract, attempt, mode, score, delegation_mask)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )
        .bind(&[
            hex::encode(resp.fdr_hash).into(),
            pk.into(),
            (contract as i64).into(),
            (attempt as i64).into(),
            (resp.mode as u8 as i64).into(),
            resp.score_s.into(),
            (fdr.delegation_mask as i64).into(),
        ])?;
    db.batch(vec![player_stmt, mission_stmt, receipt_stmt, fdr_stmt]).await?;
    Ok(())
}

// --- fetch handler ----------------------------------------------------------

#[event(fetch)]
async fn fetch(mut req: Request, env: Env, _ctx: Context) -> Result<Response> {
    let key = server_key(&env)?;
    let db = env.d1("DB")?;
    let bucket = env.bucket("FDRS")?;
    let method = req.method();
    let path = req.path();
    let now = now_s();

    match (method, path.as_str()) {
        (Method::Post, "/api/v1/commit") => {
            let body: CommitBody = req.json().await?;
            let creq: CommitRequest = body.try_into()?;
            let (pubkey, contract) = (creq.pubkey, creq.contract_id);
            let snapshot = load_snapshot(&db, &pubkey, contract).await?;
            let mut engine = AuthorityEngine::new(snapshot, key);
            let ticket = engine.commit(creq, now).map_err(to_worker)?;
            flush_commit(&db, engine.store(), &ticket).await?;
            Response::from_json(&TicketBody::from(&ticket))
        }
        (Method::Post, "/api/v1/submit") | (Method::Post, "/api/v1/submit-segment") => {
            let body: SubmitBody = req.json().await?;
            let sreq: SubmitRequest = body.try_into()?;
            let (pubkey, contract, attempt) =
                (sreq.ticket.pubkey, sreq.ticket.contract_id, sreq.ticket.attempt_num);
            let snapshot = load_snapshot(&db, &pubkey, contract).await?;
            let mut engine = AuthorityEngine::new(snapshot, key);
            let resp = engine.submit(sreq, now).map_err(to_worker)?;
            flush_submit(&db, &bucket, engine.store(), &resp, &pubkey, contract, attempt).await?;
            Response::from_json(&resp_json(&resp))
        }
        (Method::Get, p) if p.starts_with("/api/v1/verify/") => {
            let hash = p.trim_start_matches("/api/v1/verify/");
            let obj = bucket.get(hash).execute().await?;
            match obj {
                Some(o) => {
                    let body = o.text().await.unwrap_or_default();
                    Response::ok(body)
                }
                None => Response::error("not found", 404),
            }
        }
        _ => Response::error("not found", 404),
    }
}

/// The submit response JSON, matching the local adapter's shape.
fn resp_json(r: &odd_worker::SubmitResponse) -> serde_json::Value {
    serde_json::json!({
        "accepted": r.accepted,
        "mode": r.mode as u8,
        "attempt_num": r.attempt_num,
        "score_s": r.score_s,
        "elo_before": r.elo_before,
        "elo_after": r.elo_after,
        "mission_elo_after": r.mission_elo_after,
        "fdr_hash": hex::encode(r.fdr_hash),
        "receipt": r.receipt.as_ref().map(|receipt| {
            let p = &receipt.payload;
            serde_json::json!({
                "schema_v3": p.schema_v3,
                "fdr_hash": hex::encode(p.fdr_hash),
                "pubkey": hex::encode(p.pubkey),
                "contract_id": p.contract_id,
                "attempt_num": p.attempt_num,
                "mode": p.mode as u8,
                "score_s": p.score_s(),
                "elo_before": p.elo_before(),
                "elo_after": p.elo_after(),
                "mission_elo_after": p.mission_elo_after(),
                "timestamp_s": p.timestamp_s,
                "server_sig": hex::encode(receipt.server_sig),
            })
        }),
    })
}
