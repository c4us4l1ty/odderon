//! The transport-agnostic authority. Everything the server decides —
//! commitment, replay, ranked rating, receipts — lives here and nowhere
//! else. The local HTTP/SQLite adapter and the Cloudflare D1/R2 adapter are
//! thin shells around this engine; the anti-cheat gates drive it directly.
//!
//! # Laws honoured
//!
//! * **No wall clock**: the engine never reads the system time. The adapter
//!   passes `now_s` in, so a test can pin it and a replay is reproducible.
//! * **No magic lookup**: the score and the verdict come from
//!   `odd_verify::verify_record`, the same physics the browser runs. The
//!   engine never trusts a client claim.
//! * **Server-canonical contract**: the contract is resolved from the id
//!   against `odd_contracts::Contract`, and the record's framing is checked
//!   against it. A client cannot submit an easier contract.

use crate::commitment::{design_hash, prediction_hash};
use crate::protocol::{
    AttemptMode, CallsignRequest, CommitRequest, CommitTicket, ServerError, SubmitRequest,
    SubmitResponse,
};
use crate::receipt::{ReceiptPayload, ServerKey, ServerReceipt};
use crate::segment::verify_chain;
use crate::store::{Commitment, Store, StoredFdr};
use odd_contracts::Contract;
use odd_report::fdr::Fdr;

/// The authority engine over a storage port.
pub struct AuthorityEngine<S: Store> {
    store: S,
    server_key: ServerKey,
}

impl<S: Store> AuthorityEngine<S> {
    /// Build an engine with a storage backend and the server signing key.
    pub fn new(store: S, server_key: ServerKey) -> Self {
        Self { store, server_key }
    }

    /// The public key clients embed as `server_pk`.
    pub fn server_pk(&self) -> [u8; 32] {
        self.server_key.public_key()
    }

    /// Borrow the storage backend (for an adapter that needs to flush).
    pub fn store(&self) -> &S {
        &self.store
    }

    /// Mutations must be all-or-nothing, including failure after a rating
    /// update. SQLite uses BEGIN IMMEDIATE so separate server processes also
    /// serialize the read/compute/write, not just threads in this process.
    fn atomic<T>(
        &mut self,
        operation: impl FnOnce(&mut Self) -> Result<T, ServerError>,
    ) -> Result<T, ServerError> {
        self.store.begin()?;
        match operation(self) {
            Ok(value) => match self.store.commit() {
                Ok(()) => Ok(value),
                Err(error) => {
                    let _ = self.store.rollback();
                    Err(error)
                }
            },
            Err(error) => {
                let _ = self.store.rollback();
                Err(error)
            }
        }
    }

    /// Claim a callsign with proof of possession of its public key. Names are
    /// case-insensitive and cannot be stolen by changing their letter case.
    pub fn register_callsign(&mut self, req: CallsignRequest) -> Result<String, ServerError> {
        if req.name.len() > 24
            || req.name.trim() != req.name
            || req.name.is_empty()
            || !req
                .name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b' ' | b'-' | b'_'))
        {
            return Err(ServerError::InvalidCallsign);
        }
        odd_identity::verify(&req.pubkey, &req.canonical_bytes(), &req.signature)
            .map_err(|_| ServerError::InvalidClientSignature)?;
        let name = req.name.to_ascii_uppercase();
        self.atomic(|engine| {
            engine.store.register_callsign(&name, &req.pubkey)?;
            Ok(name)
        })
    }

    /// `POST /api/v1/commit`: verify the pilot's signature, atomically take
    /// the next attempt number, store the commitment and return a signed
    /// ticket. Only attempt 1 is Ranked; every later commit — including a
    /// commit→refresh→commit — is forced to Practice.
    pub fn commit(&mut self, req: CommitRequest, now_s: i64) -> Result<CommitTicket, ServerError> {
        self.atomic(|engine| engine.commit_inner(req, now_s))
    }

    fn commit_inner(
        &mut self,
        req: CommitRequest,
        now_s: i64,
    ) -> Result<CommitTicket, ServerError> {
        odd_identity::verify(&req.pubkey, &req.canonical_bytes(), &req.client_sig)
            .map_err(|_| ServerError::InvalidClientSignature)?;
        if Contract::get(req.contract_id).is_none() {
            return Err(ServerError::UnknownContract(req.contract_id));
        }
        let attempt_num = self.store.increment_attempt(&req.pubkey, req.contract_id)?;
        let mode = CommitTicket::forced_mode(attempt_num);
        let commitment = Commitment {
            design_hash: req.design_hash,
            prediction_hash: req.prediction_hash,
            mode,
            server_time_s: now_s,
        };
        self.store
            .save_commitment(&req.pubkey, req.contract_id, attempt_num, &commitment)?;
        let mut ticket = CommitTicket {
            contract_id: req.contract_id,
            pubkey: req.pubkey,
            attempt_num,
            mode,
            design_hash: req.design_hash,
            prediction_hash: req.prediction_hash,
            server_time_s: now_s,
            server_sig: [0u8; 64],
        };
        ticket.server_sig = self.server_key.sign(&ticket.canonical_bytes());
        Ok(ticket)
    }

    /// `POST /api/v1/submit` (and `/submit-segment` with `segments` set):
    /// re-check the ticket, bind the record to the commitment, replay the
    /// server-canonical contract, update Elo only for a ranked first
    /// attempt, store the FDR and issue the receipt.
    pub fn submit(
        &mut self,
        req: SubmitRequest,
        now_s: i64,
    ) -> Result<SubmitResponse, ServerError> {
        self.atomic(|engine| engine.submit_inner(req, now_s))
    }

    fn submit_inner(
        &mut self,
        req: SubmitRequest,
        now_s: i64,
    ) -> Result<SubmitResponse, ServerError> {
        let ticket = &req.ticket;
        // The ticket must carry our own signature; a forged or edited ticket
        // (e.g. attempt_num changed to 1) fails here.
        if !crate::receipt::verify_server_sig(
            &self.server_key.public_key(),
            &ticket.canonical_bytes(),
            &ticket.server_sig,
        ) {
            return Err(ServerError::InvalidTicketSignature);
        }
        if ticket.attempt_num == 0 || ticket.mode != CommitTicket::forced_mode(ticket.attempt_num) {
            return Err(ServerError::AttemptModeMismatch);
        }
        let expected = Commitment {
            design_hash: ticket.design_hash,
            prediction_hash: ticket.prediction_hash,
            mode: ticket.mode,
            server_time_s: ticket.server_time_s,
        };
        if self
            .store
            .load_commitment(&ticket.pubkey, ticket.contract_id, ticket.attempt_num)?
            != Some(expected)
        {
            return Err(ServerError::CommitmentMissing);
        }
        let fdr = Fdr::from_verify_payload(&req.fdr_payload)
            .map_err(|e| ServerError::MalformedFdr(format!("{e:?}")))?;
        // The authority issues v4 receipts and still replays v3 certificates,
        // which keep verifying under the frozen model they were flown with.
        // v1/v2 remain readable offline but omit the cryogenic and delegation
        // proof, so they cannot earn a receipt.
        if fdr.schema_version < odd_report::FROZEN_V3_SCHEMA_VERSION {
            return Err(ServerError::MalformedFdr(
                "server requires schema 3 or 4".into(),
            ));
        }
        if fdr.schema_version > odd_report::SCHEMA_VERSION {
            return Err(ServerError::MalformedFdr("unknown recorder schema".into()));
        }
        // A submission must be flown under the current physics. Replaying a v3
        // record here is a verification service, not a new rating: it has no
        // v4 coupled state to certify.
        if fdr.schema_version != odd_report::SCHEMA_VERSION {
            return Err(ServerError::MalformedFdr(
                "new submissions require the current recorder schema".into(),
            ));
        }
        if fdr.public_key != ticket.pubkey || fdr.contract_id != ticket.contract_id {
            return Err(ServerError::TicketSubjectMismatch);
        }
        if fdr.attempt_mode != ticket.mode {
            return Err(ServerError::AttemptModeMismatch);
        }
        let contract =
            Contract::get(fdr.contract_id).ok_or(ServerError::UnknownContract(fdr.contract_id))?;

        // The presented vehicle, maneuver plan and prediction must hash to
        // exactly what the pilot committed in the signed launch ticket.
        if design_hash(&fdr) != ticket.design_hash {
            return Err(ServerError::DesignCommitmentMismatch);
        }
        if prediction_hash(&fdr) != ticket.prediction_hash {
            return Err(ServerError::PredictionCommitmentMismatch);
        }

        // The client's contract parameters are ignored; the canonical
        // contract fixes the framing and the replay decides the outcome.
        odd_verify::certify_contract(&fdr, &contract)
            .map_err(|e| ServerError::ContractMismatch(format!("{e:?}")))?;
        let outcome = odd_verify::verify_record(&fdr)
            .map_err(|e| ServerError::ReplayRejected(format!("{e:?}")))?;

        // Optional checkpoints must chain to the same final state, sampling
        // each against the complete authoritative replay above.
        if let Some(segments) = &req.segments {
            // The replay currently retains orbital states but not a sampled
            // cryogenic temperature history. Never authenticate invented
            // tank temperatures: full submission still verifies all signed
            // thermal loss and vent events, but segmented cryo is not ready.
            if !fdr.thermal_specs.is_empty() {
                return Err(ServerError::ThermalSegmentsUnsupported);
            }
            verify_chain(segments, &outcome.final_state_hash, fdr.dry_mass_kg, |t| {
                outcome.trajectory.sample(t).ok()
            })?;
        }

        // report[4] is the score the canonical replay computed from the
        // committed prediction — never a client claim.
        let score_s = outcome.report[4];
        if !score_s.is_finite() || !(0.0..=1.0).contains(&score_s) {
            return Err(ServerError::ReplayRejected(
                "non-finite or out-of-range score".into(),
            ));
        }
        let mode = ticket.mode;
        let attempt_num = ticket.attempt_num;
        let fdr_hash = fdr.digest();

        // The digest covers the canonical body, not the public key. Validate
        // the stored record and receipt, then compare decoded bodies rather
        // than transport spellings: a URL, fragment and bare base64url can
        // all encode the same signed flight.
        let existing = self.store.load_fdr(&fdr_hash)?;
        if let Some(stored) = &existing {
            let decoded = stored.verified_record(&fdr_hash, &self.server_key.public_key())?;
            if decoded.public_key != fdr.public_key
                || decoded.canonical_bytes() != fdr.canonical_bytes()
            {
                return Err(ServerError::FdrHashCollision);
            }
        }

        // Idempotent re-submit: the same attempt returns the same receipt
        // and never moves Elo twice.
        if let Some(receipt) =
            self.store
                .load_receipt(&ticket.pubkey, ticket.contract_id, attempt_num)?
        {
            let p = &receipt.payload;
            // A persisted receipt is an idempotency barrier, not blindly
            // trusted data. Corrupt or swapped receipts must never be returned
            // as success or treated as permission to score this attempt again.
            if !receipt.verify(&self.server_key.public_key())
                || p.pubkey != ticket.pubkey
                || p.contract_id != ticket.contract_id
                || p.attempt_num != attempt_num
                || p.mode != mode
            {
                return Err(ServerError::Storage(
                    "stored receipt binding is invalid".into(),
                ));
            }
            if p.fdr_hash != fdr_hash {
                return Err(ServerError::AttemptAlreadySubmitted);
            }
            if p.score_s_bits != score_s.to_bits() {
                return Err(ServerError::Storage(
                    "stored receipt score is invalid".into(),
                ));
            }
            let stored = existing
                .as_ref()
                .ok_or_else(|| ServerError::Storage("stored flight is missing".into()))?;
            // SQLite keeps a receipt copy on the public flight as well as the
            // idempotency row. If this flight still indexes this attempt,
            // independently valid but divergent copies must not disagree.
            // A later Practice attempt may legitimately reuse the same FDR
            // hash and replace the flight index with its own receipt.
            if stored.pubkey == ticket.pubkey
                && stored.contract_id == ticket.contract_id
                && stored.attempt_num == attempt_num
                && stored.receipt.as_ref() != Some(&receipt)
            {
                return Err(ServerError::Storage(
                    "stored receipt copies disagree".into(),
                ));
            }
            return Ok(SubmitResponse {
                accepted: true,
                mode: p.mode,
                attempt_num: p.attempt_num,
                score_s: p.score_s(),
                elo_before: p.elo_before(),
                elo_after: p.elo_after(),
                mission_elo_after: p.mission_elo_after(),
                fdr_hash: p.fdr_hash,
                receipt: Some(receipt),
                error: None,
            });
        }

        // If the flight index says this exact attempt was already submitted,
        // a missing idempotency row is corruption, not a fresh submission.
        if existing.as_ref().is_some_and(|stored| {
            stored.pubkey == ticket.pubkey
                && stored.contract_id == ticket.contract_id
                && stored.attempt_num == attempt_num
        }) {
            return Err(ServerError::Storage("stored receipt is missing".into()));
        }

        let mut player = self.store.load_player(&ticket.pubkey)?;
        let mission_before = self.store.load_mission_elo(ticket.contract_id)?;
        if !player.elo.is_finite() || !mission_before.is_finite() {
            return Err(ServerError::Storage("invalid stored rating".into()));
        }
        let elo_before = player.elo;
        let (elo_after, mission_after) = if mode == AttemptMode::Ranked {
            let next_count = player
                .ranked_count
                .checked_add(1)
                .ok_or_else(|| ServerError::Storage("ranked counter overflow".into()))?;
            let (p, m) = odd_contracts::elo::apply_ranked(
                player.elo,
                mission_before,
                score_s,
                player.ranked_count,
            );
            if !p.is_finite() || !m.is_finite() {
                return Err(ServerError::Storage("invalid resulting rating".into()));
            }
            player.elo = p;
            player.ranked_count = next_count;
            self.store.save_player(&ticket.pubkey, &player)?;
            self.store.save_mission_elo(ticket.contract_id, m)?;
            (p, m)
        } else {
            // Practice is stored and replayable but never rated.
            (player.elo, mission_before)
        };

        let payload = ReceiptPayload::new(
            fdr_hash,
            ticket.pubkey,
            ticket.contract_id,
            attempt_num,
            mode,
            score_s,
            elo_before,
            elo_after,
            mission_after,
            now_s,
        );
        let receipt = ServerReceipt::issue(payload, &self.server_key);
        let stored = StoredFdr {
            bytes: req.fdr_payload.clone().into_bytes(),
            pubkey: ticket.pubkey,
            contract_id: ticket.contract_id,
            attempt_num,
            mode,
            score_s,
            delegation_mask: fdr.delegation_mask,
            receipt: Some(receipt.clone()),
        };
        self.store.save_fdr(&fdr_hash, &stored)?;
        self.store
            .save_receipt(&ticket.pubkey, ticket.contract_id, attempt_num, &receipt)?;

        Ok(SubmitResponse {
            accepted: true,
            mode,
            attempt_num,
            score_s,
            elo_before,
            elo_after,
            mission_elo_after: mission_after,
            fdr_hash,
            receipt: Some(receipt),
            error: None,
        })
    }
}
