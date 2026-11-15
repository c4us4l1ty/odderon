//! Cloudflare Workers D1/R2 adapter for the [`AuthorityEngine`].
//!
//! Implemented in the final increment of WP 1.9 behind the `cloudflare`
//! feature: D1 implements the [`Store`] port (SQLite under the hood, the
//! same schema as the local adapter) and R2 holds FDR blobs by hash. Needs
//! `workers-rs` and a wasm target, so it compiles in CI with network and is
//! never built by the offline gate.
//!
//! [`AuthorityEngine`]: crate::engine::AuthorityEngine
//! [`Store`]: crate::store::Store
