//! Versioned postcard encoding and SHA-256 fingerprints for ordered data.
//!
//! Serialization does not make arbitrary types canonical: callers must use
//! versioned, ordered schemas (no unordered maps/sets), define float policies,
//! and reject non-finite values at their boundary. Postcard preserves floating
//! point bit patterns, including signed zero and NaN payloads; it does not
//! normalize them. Hashes identify data, not authorship or physical validity.

use serde::Serialize;
use sha2::{Digest, Sha256};

/// Encode an explicitly ordered schema. The schema version is not included.
pub fn canonical_bytes<T: Serialize + ?Sized>(value: &T) -> Result<Vec<u8>, postcard::Error> {
    postcard::to_allocvec(value)
}

/// SHA-256 of the little-endian schema version followed by postcard bytes.
pub fn canonical_hash<T: Serialize + ?Sized>(
    version: u16,
    value: &T,
) -> Result<[u8; 32], postcard::Error> {
    let bytes = canonical_bytes(value)?;
    let mut hash = Sha256::new();
    hash.update(version.to_le_bytes());
    hash.update(bytes);
    Ok(hash.finalize().into())
}
