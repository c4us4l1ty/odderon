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
    AttemptMode, CommitRequest, CommitTicket, ServerError, SubmitRequest, SubmitResponse,
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

    /// `POST /api/v1/commit`: verify the pilot's signature, atomically take
    /// the next attempt number, store the commitment and return a signed
    /// ticket. Only attempt 1 is Ranked; every later commit — including a
    /// commit→refresh→commit — is forced to Practice.
    pub fn commit(&mut self, req: CommitRequest, now_s: i64) -> Result<CommitTicket, ServerError> {
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
        let fdr = Fdr::from_verify_payload(&req.fdr_payload)
            .map_err(|e| ServerError::MalformedFdr(format!("{e:?}")))?;
        if fdr.public_key != ticket.pubkey || fdr.contract_id != ticket.contract_id {
            return Err(ServerError::TicketSubjectMismatch);
        }
        let contract =
            Contract::get(fdr.contract_id).ok_or(ServerError::UnknownContract(fdr.contract_id))?;

        // Anti-hindsight: the presented design and prediction must hash to
        // exactly what was committed before the flight ran.
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

        // Optional chunked verification must chain to the same final state,
        // sampling each checkpoint against the authoritative trajectory.
        if let Some(segments) = &req.segments {
            verify_chain(segments, &outcome.final_state_hash, |t| {
                outcome.trajectory.sample(t).ok()
            })?;
        }

        // report[4] is the score the canonical replay computed from the
        // committed prediction — never a client claim.
        let score_s = outcome.report[4];
        let mode = ticket.mode;
        let attempt_num = ticket.attempt_num;
        let fdr_hash = fdr.digest();

        // Idempotent re-submit: the same attempt returns the same receipt
        // and never moves Elo twice.
        if let Some(receipt) =
            self.store
                .load_receipt(&ticket.pubkey, ticket.contract_id, attempt_num)?
        {
            let p = &receipt.payload;
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

        let mut player = self.store.load_player(&ticket.pubkey)?;
        let mission_before = self.store.load_mission_elo(ticket.contract_id)?;
        let elo_before = player.elo;
        let (elo_after, mission_after) = if mode == AttemptMode::Ranked {
            let (p, m) = odd_contracts::elo::apply_ranked(
                player.elo,
                mission_before,
                score_s,
                player.ranked_count,
            );
            player.elo = p;
            player.ranked_count += 1;
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
