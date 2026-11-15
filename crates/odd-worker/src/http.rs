//! Local HTTP adapter for the [`AuthorityEngine`].
//!
//! Implemented in the next increment of WP 1.9: a dependency-free
//! `std::net` HTTP/1.1 server that routes `/api/v1/commit`, `/submit` and
//! `/submit-segment` to the engine over the SQLite store, so every
//! anti-cheat gate runs locally with no cloud credentials.
//!
//! [`AuthorityEngine`]: crate::engine::AuthorityEngine
