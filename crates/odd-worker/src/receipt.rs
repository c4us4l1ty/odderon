//! The server receipt: the only thing that turns a local flight into a
//! certified, server-ranked result (blueprint §6.5, WP 1.9).
//!
//! A receipt is an Ed25519 signature by the *server* key over a fixed
//! payload. The browser embeds the server public key and re-checks the
//! signature; a DevTools-edited rating with no valid receipt can never read
//! as certified. The payload binds the exact FDR content hash, the pilot,
//! the contract, the attempt number and mode, the score and both Elo moves,
//! and the server's timestamp — so a receipt cannot be lifted onto another
//! flight or another rating.

use crate::protocol::{put_i64, put_u32, AttemptMode, Hash, PubKey, Sig};
use ed25519_compact::{KeyPair, PublicKey, Seed, Signature};

/// Domain separator for the receipt payload, frozen with schema v3.
pub const RECEIPT_DOMAIN: &[u8] = b"odderon-server-receipt-v3";
/// The schema version a receipt certifies.
pub const RECEIPT_SCHEMA_V3: u16 = 3;

/// The server's signing key. Held only by the authority; the public half is
/// what clients embed as `server_pk`.
#[derive(Clone)]
pub struct ServerKey {
    keys: KeyPair,
}

impl ServerKey {
    /// Derive the server keypair from a 32-byte seed. The seed is the only
    /// secret the operator configures; rotating it rotates `server_pk`.
    pub fn from_seed(seed: [u8; 32]) -> Self {
        Self {
            keys: KeyPair::from_seed(Seed::new(seed)),
        }
    }

    /// The public key clients embed and verify against.
    pub fn public_key(&self) -> PubKey {
        *self.keys.pk
    }

    /// Sign arbitrary canonical bytes (a ticket or a receipt payload).
    pub fn sign(&self, message: &[u8]) -> Sig {
        *self.keys.sk.sign(message, None)
    }
}

/// Verify a detached server signature against an embedded public key.
pub fn verify_server_sig(server_pk: &PubKey, message: &[u8], sig: &Sig) -> bool {
    PublicKey::new(*server_pk)
        .verify(message, &Signature::new(*sig))
        .is_ok()
}

/// The exact fields the server signs. Floats travel as their IEEE-754 bits
/// so the receipt is reproducible across platforms without decimal rounding.
#[derive(Clone, Debug, PartialEq)]
pub struct ReceiptPayload {
    pub schema_v3: u16,
    pub fdr_hash: Hash,
    pub pubkey: PubKey,
    pub contract_id: u32,
    pub attempt_num: u32,
    pub mode: AttemptMode,
    pub score_s_bits: u64,
    pub elo_before_bits: u64,
    pub elo_after_bits: u64,
    pub mission_elo_after_bits: u64,
    pub timestamp_s: i64,
}

impl ReceiptPayload {
    /// Build a payload from floating-point results, freezing their bits.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        fdr_hash: Hash,
        pubkey: PubKey,
        contract_id: u32,
        attempt_num: u32,
        mode: AttemptMode,
        score_s: f64,
        elo_before: f64,
        elo_after: f64,
        mission_elo_after: f64,
        timestamp_s: i64,
    ) -> Self {
        Self {
            schema_v3: RECEIPT_SCHEMA_V3,
            fdr_hash,
            pubkey,
            contract_id,
            attempt_num,
            mode,
            score_s_bits: score_s.to_bits(),
            elo_before_bits: elo_before.to_bits(),
            elo_after_bits: elo_after.to_bits(),
            mission_elo_after_bits: mission_elo_after.to_bits(),
            timestamp_s,
        }
    }

    /// The canonical signed layout:
    /// `domain ‖ schema_v3 ‖ fdr_hash ‖ pubkey ‖ contract_id ‖ attempt_num ‖
    /// mode ‖ score_s_bits ‖ elo_before_bits ‖ elo_after_bits ‖
    /// mission_elo_after_bits ‖ timestamp_s`, all big-endian.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(RECEIPT_DOMAIN.len() + 2 + 32 + 32 + 4 + 4 + 1 + 32 + 8);
        out.extend_from_slice(RECEIPT_DOMAIN);
        out.extend_from_slice(&self.schema_v3.to_be_bytes());
        out.extend_from_slice(&self.fdr_hash);
        out.extend_from_slice(&self.pubkey);
        put_u32(&mut out, self.contract_id);
        put_u32(&mut out, self.attempt_num);
        out.push(self.mode as u8);
        crate::protocol::put_u64(&mut out, self.score_s_bits);
        crate::protocol::put_u64(&mut out, self.elo_before_bits);
        crate::protocol::put_u64(&mut out, self.elo_after_bits);
        crate::protocol::put_u64(&mut out, self.mission_elo_after_bits);
        put_i64(&mut out, self.timestamp_s);
        out
    }

    pub fn score_s(&self) -> f64 {
        f64::from_bits(self.score_s_bits)
    }
    pub fn elo_before(&self) -> f64 {
        f64::from_bits(self.elo_before_bits)
    }
    pub fn elo_after(&self) -> f64 {
        f64::from_bits(self.elo_after_bits)
    }
    pub fn mission_elo_after(&self) -> f64 {
        f64::from_bits(self.mission_elo_after_bits)
    }
}

/// A payload plus the server's signature over its canonical bytes.
#[derive(Clone, Debug, PartialEq)]
pub struct ServerReceipt {
    pub payload: ReceiptPayload,
    pub server_sig: Sig,
}

impl ServerReceipt {
    /// Issue a receipt by signing the payload with the server key.
    pub fn issue(payload: ReceiptPayload, key: &ServerKey) -> Self {
        let server_sig = key.sign(&payload.canonical_bytes());
        Self {
            payload,
            server_sig,
        }
    }

    /// Re-check the server signature against an embedded public key. This is
    /// what the browser and the public verify page run; it needs no trust in
    /// the transport that delivered the receipt.
    pub fn verify(&self, server_pk: &PubKey) -> bool {
        verify_server_sig(server_pk, &self.payload.canonical_bytes(), &self.server_sig)
    }

    /// True only when the signature is valid *and* the attempt was ranked.
    /// A valid receipt over a practice attempt is server-verified but not
    /// ranked, so it is not a "server-ranked" certificate.
    pub fn is_certified_ranked(&self, server_pk: &PubKey) -> bool {
        self.payload.mode == AttemptMode::Ranked && self.verify(server_pk)
    }

    /// Serialize for storage: the canonical payload bytes followed by the
    /// 64-byte signature. This is the blob the SQLite and D1 adapters keep.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = self.payload.canonical_bytes();
        out.extend_from_slice(&self.server_sig);
        out
    }

    /// Parse a stored receipt blob. Returns `None` on any length or
    /// domain mismatch; the signature is *not* checked here (call `verify`).
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let dom = RECEIPT_DOMAIN.len();
        let expected = dom + 2 + 32 + 32 + 4 + 4 + 1 + 8 * 4 + 8 + 64;
        if bytes.len() != expected || bytes.get(..dom) != Some(RECEIPT_DOMAIN) {
            return None;
        }
        let mut p = dom;
        let take = |p: &mut usize, n: usize| -> &[u8] {
            let s = &bytes[*p..*p + n];
            *p += n;
            s
        };
        let schema_v3 = u16::from_be_bytes(take(&mut p, 2).try_into().ok()?);
        let fdr_hash: Hash = take(&mut p, 32).try_into().ok()?;
        let pubkey: PubKey = take(&mut p, 32).try_into().ok()?;
        let contract_id = u32::from_be_bytes(take(&mut p, 4).try_into().ok()?);
        let attempt_num = u32::from_be_bytes(take(&mut p, 4).try_into().ok()?);
        let mode = AttemptMode::from_u8(take(&mut p, 1)[0])?;
        let score_s_bits = u64::from_be_bytes(take(&mut p, 8).try_into().ok()?);
        let elo_before_bits = u64::from_be_bytes(take(&mut p, 8).try_into().ok()?);
        let elo_after_bits = u64::from_be_bytes(take(&mut p, 8).try_into().ok()?);
        let mission_elo_after_bits = u64::from_be_bytes(take(&mut p, 8).try_into().ok()?);
        let timestamp_s = i64::from_be_bytes(take(&mut p, 8).try_into().ok()?);
        let server_sig: Sig = take(&mut p, 64).try_into().ok()?;
        Some(Self {
            payload: ReceiptPayload {
                schema_v3,
                fdr_hash,
                pubkey,
                contract_id,
                attempt_num,
                mode,
                score_s_bits,
                elo_before_bits,
                elo_after_bits,
                mission_elo_after_bits,
                timestamp_s,
            },
            server_sig,
        })
    }
}

/// The certification banner the UI and dossier print. The stored local
/// rating is never an input: only a cryptographically valid receipt can
/// produce a certified label, which is what defeats a DevTools localStorage
/// edit.
pub fn certification_label(receipt: Option<&ServerReceipt>, server_pk: &PubKey) -> &'static str {
    match receipt {
        Some(r) if r.is_certified_ranked(server_pk) => "★ CERTIFIED SERVER-RANKED RECEIPT",
        Some(r) if r.verify(server_pk) => "SERVER-VERIFIED PRACTICE",
        _ => "UNVERIFIED LOCAL / PRACTICE",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn payload() -> ReceiptPayload {
        ReceiptPayload::new(
            [7u8; 32],
            [9u8; 32],
            103,
            1,
            AttemptMode::Ranked,
            0.8148623316575985,
            1200.0,
            1231.5,
            1647.2,
            1_700_000_000,
        )
    }

    #[test]
    fn a_receipt_round_trips_under_the_server_key() {
        let key = ServerKey::from_seed([42u8; 32]);
        let receipt = ServerReceipt::issue(payload(), &key);
        assert!(receipt.verify(&key.public_key()));
        assert!(receipt.is_certified_ranked(&key.public_key()));
        assert_eq!(
            certification_label(Some(&receipt), &key.public_key()),
            "★ CERTIFIED SERVER-RANKED RECEIPT"
        );
    }

    #[test]
    fn a_foreign_key_cannot_certify() {
        let key = ServerKey::from_seed([42u8; 32]);
        let other = ServerKey::from_seed([43u8; 32]);
        let receipt = ServerReceipt::issue(payload(), &key);
        assert!(!receipt.verify(&other.public_key()));
        assert_eq!(
            certification_label(Some(&receipt), &other.public_key()),
            "UNVERIFIED LOCAL / PRACTICE"
        );
    }

    #[test]
    fn tampering_the_rating_invalidates_the_receipt() {
        let key = ServerKey::from_seed([42u8; 32]);
        let mut receipt = ServerReceipt::issue(payload(), &key);
        // A DevTools edit that bumps the certified Elo to 2999.
        receipt.payload.elo_after_bits = 2999.0f64.to_bits();
        assert!(!receipt.verify(&key.public_key()));
        assert_eq!(
            certification_label(Some(&receipt), &key.public_key()),
            "UNVERIFIED LOCAL / PRACTICE"
        );
    }

    #[test]
    fn no_receipt_is_never_certified() {
        let key = ServerKey::from_seed([42u8; 32]);
        assert_eq!(
            certification_label(None, &key.public_key()),
            "UNVERIFIED LOCAL / PRACTICE"
        );
    }

    #[test]
    fn a_practice_receipt_is_verified_but_not_ranked() {
        let key = ServerKey::from_seed([42u8; 32]);
        let mut p = payload();
        p.mode = AttemptMode::Practice;
        p.attempt_num = 2;
        let receipt = ServerReceipt::issue(p, &key);
        assert!(receipt.verify(&key.public_key()));
        assert!(!receipt.is_certified_ranked(&key.public_key()));
        assert_eq!(
            certification_label(Some(&receipt), &key.public_key()),
            "SERVER-VERIFIED PRACTICE"
        );
    }
}
