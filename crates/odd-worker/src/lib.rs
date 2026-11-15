//! # odd-worker — server-authoritative Astro-Elo (WP 1.9)
//!
//! Hexagonal by construction: [`AuthorityEngine`] is pure and transport-
//! agnostic and talks to a [`Store`] port. Two adapters implement the ports:
//!
//! * a **local HTTP/SQLite** adapter (`local` feature) that `./scripts/gate.sh`
//!   exercises with no cloud credentials, and
//! * a **Cloudflare Workers D1/R2** adapter (`cloudflare` feature) for the
//!   deployed $0/month service.
//!
//! The same `odd_verify::verify_record` physics runs in the browser and here,
//! so a receipt certifies a replay anyone can re-run. A result reads as
//! "★ CERTIFIED SERVER-RANKED RECEIPT" only with a valid [`ServerReceipt`]
//! over the canonical payload; a DevTools-edited local rating with no
//! receipt reads as "UNVERIFIED LOCAL / PRACTICE".
//!
//! [`ServerReceipt`]: receipt::ServerReceipt

pub mod commitment;
pub mod engine;
pub mod protocol;
pub mod receipt;
pub mod segment;
pub mod store;

pub use engine::AuthorityEngine;
pub use protocol::{
    AttemptMode, CommitRequest, CommitTicket, SegmentCheckpoint, ServerError, SubmitRequest,
    SubmitResponse,
};
pub use receipt::{
    certification_label, verify_server_sig, ReceiptPayload, ServerKey, ServerReceipt,
};
pub use store::{Commitment, LeaderRow, MemoryStore, PlayerRow, Store, StoredFdr};

#[cfg(feature = "local")]
pub mod http;
#[cfg(feature = "local")]
pub mod sqlite;

#[cfg(feature = "cloudflare")]
pub mod worker;
