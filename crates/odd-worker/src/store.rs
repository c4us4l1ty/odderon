//! The storage port. The engine talks to this trait only; the local SQLite
//! adapter and the Cloudflare D1/R2 adapter implement it. An adapter must
//! give the engine exclusive access for the duration of one operation (a
//! mutex guard locally, a D1 batch in the cloud) so the read–compute–write
//! of a ranked result is atomic.

use crate::protocol::{AttemptMode, Hash, PubKey, ServerError};
use crate::receipt::ServerReceipt;
use std::collections::BTreeMap;

/// The hashes a pilot committed to before flying.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Commitment {
    pub design_hash: Hash,
    pub prediction_hash: Hash,
    pub mode: AttemptMode,
    pub server_time_s: i64,
}

/// A pilot's authoritative rating state.
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerRow {
    pub elo: f64,
    pub ranked_count: u32,
    pub callsign: Option<String>,
}

impl Default for PlayerRow {
    fn default() -> Self {
        Self {
            elo: odd_contracts::elo::START_ELO,
            ranked_count: 0,
            callsign: None,
        }
    }
}

/// A stored flight, kept by content hash so the public verify page can fetch
/// it and so a re-submit is idempotent.
#[derive(Clone, Debug, PartialEq)]
pub struct StoredFdr {
    pub bytes: Vec<u8>,
    pub pubkey: PubKey,
    pub contract_id: u32,
    pub attempt_num: u32,
    pub mode: AttemptMode,
    pub score_s: f64,
    pub receipt: Option<ServerReceipt>,
}

/// One leaderboard row (Slice 3 reads these).
#[derive(Clone, Debug, PartialEq)]
pub struct LeaderRow {
    pub callsign: Option<String>,
    pub pubkey: PubKey,
    pub elo: f64,
    pub score_s: f64,
    pub delegation_mask: u8,
    pub fdr_hash: Hash,
}

/// The dumb CRUD port. Atomicity is the adapter's job, not the trait's.
pub trait Store {
    /// Atomically bump and return the per-`(pubkey, contract)` attempt count.
    /// The first call returns 1; a commit→refresh→commit returns 2, which the
    /// engine forces to Practice.
    fn increment_attempt(&mut self, pubkey: &PubKey, contract_id: u32) -> Result<u32, ServerError>;

    fn save_commitment(
        &mut self,
        pubkey: &PubKey,
        contract_id: u32,
        attempt_num: u32,
        commitment: &Commitment,
    ) -> Result<(), ServerError>;

    fn load_commitment(
        &self,
        pubkey: &PubKey,
        contract_id: u32,
        attempt_num: u32,
    ) -> Result<Option<Commitment>, ServerError>;

    fn load_player(&self, pubkey: &PubKey) -> Result<PlayerRow, ServerError>;

    fn save_player(&mut self, pubkey: &PubKey, row: &PlayerRow) -> Result<(), ServerError>;

    /// The mission's current Elo, seeded from the contract's difficulty when
    /// no pilot has moved it yet.
    fn load_mission_elo(&self, contract_id: u32) -> Result<f64, ServerError>;

    fn save_mission_elo(&mut self, contract_id: u32, elo: f64) -> Result<(), ServerError>;

    fn save_fdr(&mut self, hash: &Hash, fdr: &StoredFdr) -> Result<(), ServerError>;

    fn load_fdr(&self, hash: &Hash) -> Result<Option<StoredFdr>, ServerError>;

    fn save_receipt(
        &mut self,
        pubkey: &PubKey,
        contract_id: u32,
        attempt_num: u32,
        receipt: &ServerReceipt,
    ) -> Result<(), ServerError>;

    fn load_receipt(
        &self,
        pubkey: &PubKey,
        contract_id: u32,
        attempt_num: u32,
    ) -> Result<Option<ServerReceipt>, ServerError>;

    /// Register a callsign first-come-first-served. Returns `CallsignTaken`
    /// if another key already owns it; idempotent for the same key.
    fn register_callsign(&mut self, callsign: &str, pubkey: &PubKey) -> Result<(), ServerError>;

    /// The stored flights for a contract (or all contracts when `None`),
    /// newest first. The engine orders and truncates for the leaderboard.
    fn leaderboard(&self, contract_id: Option<u32>) -> Result<Vec<LeaderRow>, ServerError>;
}

type AttemptKey = ([u8; 32], u32);
type CommitKey = ([u8; 32], u32, u32);

/// An in-memory store for tests and for the default local server before a
/// database path is configured. Deterministic and dependency-free.
#[derive(Default)]
pub struct MemoryStore {
    attempts: BTreeMap<AttemptKey, u32>,
    commitments: BTreeMap<CommitKey, Commitment>,
    players: BTreeMap<[u8; 32], PlayerRow>,
    missions: BTreeMap<u32, f64>,
    fdrs: BTreeMap<[u8; 32], StoredFdr>,
    receipts: BTreeMap<CommitKey, ServerReceipt>,
    callsigns: BTreeMap<String, [u8; 32]>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self::default()
    }
}

/// The seeded difficulty of each canonical contract, used until a pilot
/// moves the mission's rating. Mirrors `web/mission.js` CONTRACT_RATINGS.
pub fn seeded_mission_elo(contract_id: u32) -> f64 {
    match contract_id {
        101 => 800.0,
        102 => 1250.0,
        103 => 1650.0,
        _ => odd_contracts::elo::START_ELO,
    }
}

impl Store for MemoryStore {
    fn increment_attempt(&mut self, pubkey: &PubKey, contract_id: u32) -> Result<u32, ServerError> {
        let entry = self.attempts.entry((*pubkey, contract_id)).or_insert(0);
        *entry += 1;
        Ok(*entry)
    }

    fn save_commitment(
        &mut self,
        pubkey: &PubKey,
        contract_id: u32,
        attempt_num: u32,
        commitment: &Commitment,
    ) -> Result<(), ServerError> {
        self.commitments
            .insert((*pubkey, contract_id, attempt_num), commitment.clone());
        Ok(())
    }

    fn load_commitment(
        &self,
        pubkey: &PubKey,
        contract_id: u32,
        attempt_num: u32,
    ) -> Result<Option<Commitment>, ServerError> {
        Ok(self
            .commitments
            .get(&(*pubkey, contract_id, attempt_num))
            .cloned())
    }

    fn load_player(&self, pubkey: &PubKey) -> Result<PlayerRow, ServerError> {
        Ok(self.players.get(pubkey).cloned().unwrap_or_default())
    }

    fn save_player(&mut self, pubkey: &PubKey, row: &PlayerRow) -> Result<(), ServerError> {
        self.players.insert(*pubkey, row.clone());
        Ok(())
    }

    fn load_mission_elo(&self, contract_id: u32) -> Result<f64, ServerError> {
        Ok(self
            .missions
            .get(&contract_id)
            .copied()
            .unwrap_or_else(|| seeded_mission_elo(contract_id)))
    }

    fn save_mission_elo(&mut self, contract_id: u32, elo: f64) -> Result<(), ServerError> {
        self.missions.insert(contract_id, elo);
        Ok(())
    }

    fn save_fdr(&mut self, hash: &Hash, fdr: &StoredFdr) -> Result<(), ServerError> {
        self.fdrs.insert(*hash, fdr.clone());
        Ok(())
    }

    fn load_fdr(&self, hash: &Hash) -> Result<Option<StoredFdr>, ServerError> {
        Ok(self.fdrs.get(hash).cloned())
    }

    fn save_receipt(
        &mut self,
        pubkey: &PubKey,
        contract_id: u32,
        attempt_num: u32,
        receipt: &ServerReceipt,
    ) -> Result<(), ServerError> {
        self.receipts
            .insert((*pubkey, contract_id, attempt_num), receipt.clone());
        Ok(())
    }

    fn load_receipt(
        &self,
        pubkey: &PubKey,
        contract_id: u32,
        attempt_num: u32,
    ) -> Result<Option<ServerReceipt>, ServerError> {
        Ok(self
            .receipts
            .get(&(*pubkey, contract_id, attempt_num))
            .cloned())
    }

    fn register_callsign(&mut self, callsign: &str, pubkey: &PubKey) -> Result<(), ServerError> {
        match self.callsigns.get(callsign) {
            Some(owner) if owner != pubkey => Err(ServerError::CallsignTaken),
            _ => {
                self.callsigns.insert(callsign.to_string(), *pubkey);
                let mut row = self.load_player(pubkey)?;
                row.callsign = Some(callsign.to_string());
                self.save_player(pubkey, &row)
            }
        }
    }

    fn leaderboard(&self, contract_id: Option<u32>) -> Result<Vec<LeaderRow>, ServerError> {
        let mut rows = Vec::new();
        for (hash, fdr) in &self.fdrs {
            if let Some(id) = contract_id {
                if fdr.contract_id != id {
                    continue;
                }
            }
            let player = self.load_player(&fdr.pubkey)?;
            // The delegation mask is recovered from the stored FDR bytes by
            // the engine when it builds the public leaderboard; the store
            // keeps the row minimal and the engine fills the mask.
            rows.push(LeaderRow {
                callsign: player.callsign,
                pubkey: fdr.pubkey,
                elo: player.elo,
                score_s: fdr.score_s,
                delegation_mask: 0,
                fdr_hash: *hash,
            });
        }
        Ok(rows)
    }
}
