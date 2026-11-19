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

/// A finite IEEE-754 value with negative zero normalized to positive zero.
/// This type is constructed explicitly so deserialization cannot bypass its
/// validation. It does not quantize inputs or eliminate rounding differences.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(transparent)]
pub struct FiniteF64(f64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NonFiniteValue;

impl FiniteF64 {
    pub fn new(value: f64) -> Result<Self, NonFiniteValue> {
        if !value.is_finite() {
            return Err(NonFiniteValue);
        }
        Ok(Self(if value == 0.0 { 0.0 } else { value }))
    }

    pub fn get(self) -> f64 {
        self.0
    }
}

/// A deterministic building block for explicitly versioned schemas. Maps are
/// ordered lexicographically by their string keys; floats must be finite.
/// Changing variant order, field semantics, or postcard encoding requires a
/// schema-version change. String normalization is the caller's responsibility.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub enum CanonicalValue {
    Null,
    Bool(bool),
    Integer(i64),
    Float(FiniteF64),
    Text(String),
    Bytes(Vec<u8>),
    List(Vec<CanonicalValue>),
    Map(std::collections::BTreeMap<String, CanonicalValue>),
}
