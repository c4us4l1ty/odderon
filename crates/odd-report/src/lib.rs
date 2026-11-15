//! Signed flight data recorder and deterministic verification dossier.
//!
//! This crate deliberately contains no physics. It defines the canonical
//! bytes that a flight is signed over, the errors a verifier can report, and
//! the printable memorandum. Re-running the flight belongs to the simulator
//! that produced it, so replay verification lives next to the integrator and
//! calls `Fdr::compare_metrics` / `Fdr::compare_final_state` here.
//!
//! ```text
//! Fdr::canonical_bytes  --sign-->  ed25519  --transport-->  JSON / #verify= / QR
//!                       --verify-> signature check
//!                       --replay-> bit-for-bit metric and terminal-state check
//! ```
pub mod base64;
pub mod dossier;
pub mod fdr;
pub mod hex;
pub mod json;
pub mod metrics;
pub mod qr;

pub use fdr::{AttemptMode, Command, Fdr, FdrError, METRIC_COUNT, SCHEMA_VERSION};

/// A successful verification, after both the signature and the replay agree.
pub const VERIFIED_BANNER: &str = "\u{2713} CRYPTOGRAPHICALLY & PHYSICALLY VERIFIED";
