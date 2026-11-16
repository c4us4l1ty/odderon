//! Transport-agnostic wire types for the authority API (blueprint §6.3).
//!
//! Every signed structure has an explicit big-endian canonical layout rather
//! than a serde-derived encoding, so a browser, a SQLite-backed local server
//! and a Cloudflare Worker all hash the identical bytes. Nothing here knows
//! about HTTP, D1 or R2.

pub use odd_report::fdr::AttemptMode;

/// An Ed25519 public key.
pub type PubKey = [u8; 32];
/// A SHA-256 digest (design, prediction or FDR content hash).
pub type Hash = [u8; 32];
/// An Ed25519 signature.
pub type Sig = [u8; 64];

/// Domain separator for a commit ticket the server signs.
pub const TICKET_DOMAIN: &[u8] = b"odderon-commit-ticket-v1";
/// Domain separator for the client's commitment signature.
pub const COMMIT_DOMAIN: &[u8] = b"odderon-client-commit-v1";
/// Domain separator for a callsign registration signature.
pub const CALLSIGN_DOMAIN: &[u8] = b"odderon-callsign-v1";

/// `POST /api/v1/commit` body. The client commits to a design and a
/// prediction *before* any physics runs; `client_sig` covers the canonical
/// commitment bytes under the pilot's key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommitRequest {
    pub contract_id: u32,
    pub pubkey: PubKey,
    pub design_hash: Hash,
    pub prediction_hash: Hash,
    pub client_sig: Sig,
}

impl CommitRequest {
    /// The bytes the pilot signs: domain, contract, key and both hashes.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(COMMIT_DOMAIN.len() + 4 + 32 + 32 + 32);
        out.extend_from_slice(COMMIT_DOMAIN);
        put_u32(&mut out, self.contract_id);
        out.extend_from_slice(&self.pubkey);
        out.extend_from_slice(&self.design_hash);
        out.extend_from_slice(&self.prediction_hash);
        out
    }
}

/// A callsign claim signed by the owner of the public key. The signature
/// covers the exact UTF-8 name as well as the key, with an explicit length.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallsignRequest {
    pub name: String,
    pub pubkey: PubKey,
    pub signature: Sig,
}

impl CallsignRequest {
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(CALLSIGN_DOMAIN);
        out.extend_from_slice(&self.pubkey);
        out.extend_from_slice(&(self.name.len() as u16).to_be_bytes());
        out.extend_from_slice(self.name.as_bytes());
        out
    }
}

/// The server's signed response to a commit. `attempt_num` is the atomic
/// per-`(pubkey, contract)` counter; only `attempt_num == 1` is Ranked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommitTicket {
    pub contract_id: u32,
    pub pubkey: PubKey,
    pub attempt_num: u32,
    pub mode: AttemptMode,
    pub design_hash: Hash,
    pub prediction_hash: Hash,
    pub server_time_s: i64,
    pub server_sig: Sig,
}

impl CommitTicket {
    /// The bytes the server signs (everything but the signature itself).
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(TICKET_DOMAIN.len() + 4 + 32 + 4 + 1 + 32 + 32 + 8);
        out.extend_from_slice(TICKET_DOMAIN);
        put_u32(&mut out, self.contract_id);
        out.extend_from_slice(&self.pubkey);
        put_u32(&mut out, self.attempt_num);
        out.push(self.mode as u8);
        out.extend_from_slice(&self.design_hash);
        out.extend_from_slice(&self.prediction_hash);
        put_i64(&mut out, self.server_time_s);
        out
    }

    /// The mode the server forces, regardless of what the client asked for.
    pub fn forced_mode(attempt_num: u32) -> AttemptMode {
        if attempt_num == 1 {
            AttemptMode::Ranked
        } else {
            AttemptMode::Practice
        }
    }
}

/// One checkpoint in a chunked hash-chain verification (blueprint §6.4).
/// The state vector is `[x, y, z, vx, vy, vz, mass]`; `cum_hash` chains every
/// prior checkpoint so a tampered intermediate breaks the chain at its own
/// boundary rather than only at the end.
#[derive(Clone, Debug, PartialEq)]
pub struct SegmentCheckpoint {
    pub t_us: i64,
    pub state_vector: [f64; 7],
    pub prop_kg: f64,
    pub tank_temps: [f64; 2],
    pub cum_hash: Hash,
}

/// `POST /api/v1/submit` body. `fdr_payload` is the signed FDR in the same
/// base64url wire form the QR code uses (`Fdr::verify_payload`). The ticket
/// is the one the server issued at commit; the server re-checks its own
/// signature on it. Optional segments switch on chunked verification.
#[derive(Clone, Debug, PartialEq)]
pub struct SubmitRequest {
    pub fdr_payload: String,
    pub ticket: CommitTicket,
    pub segments: Option<Vec<SegmentCheckpoint>>,
}

/// The authority's verdict on a submitted flight.
#[derive(Clone, Debug, PartialEq)]
pub struct SubmitResponse {
    pub accepted: bool,
    pub mode: AttemptMode,
    pub attempt_num: u32,
    pub score_s: f64,
    pub elo_before: f64,
    pub elo_after: f64,
    pub mission_elo_after: f64,
    pub fdr_hash: Hash,
    pub receipt: Option<crate::receipt::ServerReceipt>,
    pub error: Option<ServerError>,
}

/// Every way the authority can refuse a request. The variants are stable
/// identifiers the UI and the dossier match on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ServerError {
    /// The client signature over the commitment does not verify.
    InvalidClientSignature,
    /// The server signature on a presented ticket does not verify.
    InvalidTicketSignature,
    /// No such canonical contract.
    UnknownContract(u32),
    /// The submitted design does not hash to the committed design.
    DesignCommitmentMismatch,
    /// The submitted prediction does not hash to the committed prediction.
    /// This is the anti-hindsight gate: a re-signed prediction B fails here.
    PredictionCommitmentMismatch,
    /// The ticket is for a different key or contract than the FDR.
    TicketSubjectMismatch,
    /// The FDR did not decode or its own signature is invalid.
    MalformedFdr(String),
    /// The physics replay rejected the record (engine, metrics, state,
    /// vents or boil-off). Carries the verifier's diagnostic.
    ReplayRejected(String),
    /// The record's framing does not match the server-canonical contract.
    ContractMismatch(String),
    /// A chunked segment did not reproduce its checkpoint, or the cumulative
    /// hash chain broke at this boundary.
    SegmentMismatch(usize),
    /// The ticket does not correspond to a stored commitment.
    CommitmentMissing,
    /// The record or ticket mode does not match the forced attempt mode.
    AttemptModeMismatch,
    /// This attempt already certified a different record.
    AttemptAlreadySubmitted,
    /// A different pilot already stored this canonical content hash.
    FdrHashCollision,
    /// The callsign is already owned by another public key.
    CallsignTaken,
    /// The callsign failed validation (length or character set).
    InvalidCallsign,
    /// A storage backend failed.
    Storage(String),
}

impl core::fmt::Display for ServerError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidClientSignature => write!(f, "client commitment signature is invalid"),
            Self::InvalidTicketSignature => write!(f, "commit ticket signature is invalid"),
            Self::UnknownContract(id) => write!(f, "no canonical contract {id}"),
            Self::DesignCommitmentMismatch => write!(f, "design does not match the commitment"),
            Self::PredictionCommitmentMismatch => {
                write!(f, "prediction does not match the commitment")
            }
            Self::TicketSubjectMismatch => write!(f, "ticket subject does not match the record"),
            Self::MalformedFdr(e) => write!(f, "malformed flight record: {e}"),
            Self::ReplayRejected(e) => write!(f, "physics replay rejected the record: {e}"),
            Self::ContractMismatch(e) => write!(f, "contract framing mismatch: {e}"),
            Self::SegmentMismatch(i) => write!(f, "segment {i} broke the verification chain"),
            Self::CommitmentMissing => write!(f, "ticket has no matching stored commitment"),
            Self::AttemptModeMismatch => write!(f, "record mode does not match the ticket"),
            Self::AttemptAlreadySubmitted => {
                write!(f, "attempt already submitted a different record")
            }
            Self::FdrHashCollision => {
                write!(f, "record hash is already owned by a different record")
            }
            Self::CallsignTaken => write!(f, "callsign is already registered to another key"),
            Self::InvalidCallsign => write!(f, "callsign is invalid"),
            Self::Storage(e) => write!(f, "storage backend failed: {e}"),
        }
    }
}

impl std::error::Error for ServerError {}

// --- explicit big-endian encoding helpers, shared by every signed layout ---

pub(crate) fn put_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_be_bytes());
}

pub(crate) fn put_i64(out: &mut Vec<u8>, v: i64) {
    out.extend_from_slice(&v.to_be_bytes());
}

pub(crate) fn put_u64(out: &mut Vec<u8>, v: u64) {
    out.extend_from_slice(&v.to_be_bytes());
}

pub(crate) fn put_f64(out: &mut Vec<u8>, v: f64) {
    out.extend_from_slice(&v.to_bits().to_be_bytes());
}
