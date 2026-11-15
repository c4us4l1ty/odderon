//! Local durable SQLite adapter for the [`Store`] port.
//!
//! Implemented in the next increment of WP 1.9: a thin FFI binding to the
//! system `libsqlite3` (no vendored database crate), so the local server
//! persists commitments, ratings and FDRs exactly as Cloudflare D1 does.
//! The anti-cheat oracles already run against the storage-agnostic engine
//! via `MemoryStore`; this adapter will be exercised by the same gates over
//! HTTP in `./scripts/gate.sh`.
//!
//! [`Store`]: crate::store::Store
