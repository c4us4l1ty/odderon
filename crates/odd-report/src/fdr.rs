//! Canonical, signed Flight Data Recorder.
//!
//! The signed object is a fixed-layout byte string, not JSON. JSON is only a
//! transport: two documents that differ in whitespace, key order or number
//! formatting describe the same flight and must produce the same signature.
//! Every field that a reader could act on is inside the signed bytes,
//! including the callsign and the rating change, so a dossier cannot be
//! re-attributed or its Elo delta inflated without breaking the signature.
//!
//! Floats are encoded as raw IEEE-754 big-endian bits so a metric cannot be
//! rounded by a decimal round trip. The single exception is NaN: undefined
//! metrics are common (an impacted flight has no attribution shares) and NaN
//! payloads are not stable across platforms, so every NaN collapses to one
//! canonical quiet NaN. Signed zero is preserved, because +0 and -0 are
//! different bit patterns and the replay check compares bits.
use crate::json::Json;
use odd_identity::{License, LicenseError};
use odd_propulsion::{EngineSpec, Propellant};
use sha2::{Digest, Sha256};

/// Schema 2 carries the physical engine design and is the only schema this
/// build issues. Schema 1 records (thrust and Isp as bare claims) still parse
/// and verify so certificates issued before WP 1.5 do not become unreadable;
/// they are labelled legacy because their engine was never certified.
pub const SCHEMA_VERSION: u16 = 2;
/// Last schema whose engine was an unverified claim.
pub const LEGACY_SCHEMA_VERSION: u16 = 1;
pub const METRIC_COUNT: usize = 58;
/// Domain separator for the signed bytes. Distinct from the key-derivation
/// domain so a license signature can never be replayed as a flight signature.
pub const SIGN_DOMAIN: &[u8] = b"odderon-flight-data-recorder-v2\0";
/// Domain of pre-engine records. Kept distinct so a legacy signature can
/// never be replayed as a schema-2 signature.
pub const LEGACY_SIGN_DOMAIN: &[u8] = b"odderon-flight-data-recorder-v1\0";
const CANONICAL_NAN: u64 = 0x7ff8_0000_0000_0000;
/// A schema-2 record without an engine is malformed and can never be signed;
/// this sentinel only keeps `canonical_bytes` total for such a record.
const UNSIGNABLE_ENGINE: EngineSpec = EngineSpec {
    propellant: Propellant::LoxLh2,
    of_ratio: f64::NAN,
    p_c_pa: f64::NAN,
    r_throat_m: f64::NAN,
    r_exit_m: f64::NAN,
    length_fraction: f64::NAN,
};
pub const MAX_CALLSIGN: usize = 24;
pub const MAX_COMMANDS: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttemptMode {
    Ranked = 0,
    Practice = 1,
}

impl AttemptMode {
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Ranked),
            1 => Some(Self::Practice),
            _ => None,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Ranked => "Ranked",
            Self::Practice => "Practice",
        }
    }
}

/// One commanded maneuver. Time is integer microseconds so a command cannot
/// drift by a fraction during transport; the delta-v components stay f64
/// because they are physical quantities, not schedule ticks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Command {
    pub time_us: i64,
    pub delta_v_tnw: [f64; 3],
    /// Optional guided target state (position then velocity).
    pub guidance: Option<[f64; 6]>,
}

#[derive(Clone, Debug)]
pub struct Fdr {
    pub schema_version: u16,
    pub contract_id: u32,
    pub attempt_mode: AttemptMode,
    pub callsign: String,
    /// Seconds since the Unix epoch, as the planner uses it.
    pub epoch_s: i64,
    pub duration_us: i64,
    pub initial_mass_kg: f64,
    pub dry_mass_kg: f64,
    /// Derived performance, recorded for readability. A schema-2 verifier
    /// recomputes both from `engine` and refuses a mismatch; these floats are
    /// never trusted as inputs.
    pub thrust_n: f64,
    pub specific_impulse_s: f64,
    /// The physical engine design. `None` only for a legacy schema-1 record.
    pub engine: Option<EngineSpec>,
    /// Initial position then velocity, J2000, at mission time zero.
    pub initial_state: [f64; 6],
    /// True when attribution measures node rotation instead of apoapsis.
    pub node_metric: bool,
    /// Locked prediction: delta-v, apoapsis altitude, periapsis altitude,
    /// remaining propellant above dry mass, flight time, target miss
    /// distance. NaN where not predicted.
    pub prediction_baseline: [f64; 6],
    pub commands: Vec<Command>,
    pub metrics: [f64; METRIC_COUNT],
    /// SHA-256 of the canonical final state (see `final_state_hash`).
    pub final_state_hash: [u8; 32],
    pub elo_before: f64,
    pub elo_after: f64,
    pub public_key: [u8; 32],
    pub signature: [u8; 64],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FdrError {
    /// The document declares a schema this build cannot interpret.
    UnsupportedSchema(u16),
    /// Structurally impossible: bad length, command count or callsign.
    Malformed(&'static str),
    /// The Ed25519 signature does not cover these canonical bytes.
    SignatureMismatch,
    /// Signature is valid but the physics does not reproduce a recorded value.
    /// `field` names the first mismatch so a rejection is diagnosable.
    PhysicsReplayMismatch {
        field: &'static str,
        index: usize,
    },
    /// The recorded final-state hash does not match the replayed final state.
    FinalStateMismatch,
    /// A recorded derived quantity disagrees with the engine the record
    /// signs: the client claimed performance the design cannot produce.
    EngineMismatch(&'static str),
    /// The engine design itself is outside the model's validated envelope.
    EngineUnusable,
    /// A ranked document was submitted for a contract already certified.
    AlreadyCertified(u32),
    License(LicenseError),
}

impl core::fmt::Display for FdrError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnsupportedSchema(v) => write!(f, "unsupported FDR schema version {v}"),
            Self::Malformed(what) => write!(f, "malformed FDR: {what}"),
            Self::SignatureMismatch => {
                write!(f, "SignatureMismatch: the signed bytes do not match the signature")
            }
            Self::PhysicsReplayMismatch { field, index } => write!(
                f,
                "PhysicsReplayMismatch: {field}[{index}] does not reproduce from the command timeline"
            ),
            Self::FinalStateMismatch => {
                write!(f, "PhysicsReplayMismatch: final_state_hash does not reproduce")
            }
            Self::EngineMismatch(field) => write!(
                f,
                "EngineMismatch: recorded {field} is not what the signed engine design produces"
            ),
            Self::EngineUnusable => {
                write!(f, "EngineUnusable: the signed engine design does not solve")
            }
            Self::AlreadyCertified(id) => {
                write!(f, "contract {id} already has a certified ranked certificate")
            }
            Self::License(e) => write!(f, "license error: {e:?}"),
        }
    }
}

/// Equality means "the same signed flight", not field-by-field float
/// equality: undefined metrics are NaN, and NaN is not equal to itself, so a
/// derived PartialEq would say a recorder differs from its own round trip.
impl PartialEq for Fdr {
    fn eq(&self, other: &Self) -> bool {
        self.public_key == other.public_key
            && self.signature == other.signature
            && self.canonical_bytes() == other.canonical_bytes()
    }
}

impl Eq for Fdr {}

fn put_f64(out: &mut Vec<u8>, value: f64) {
    let bits = if value.is_nan() {
        CANONICAL_NAN
    } else {
        value.to_bits()
    };
    out.extend_from_slice(&bits.to_be_bytes());
}

fn put_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_be_bytes());
}
fn put_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_be_bytes());
}
fn put_i64(out: &mut Vec<u8>, value: i64) {
    out.extend_from_slice(&value.to_be_bytes());
}

/// Hash of the terminal state. Includes the evaluation epoch and mass so a
/// replay that stops at a different time or with a different propellant
/// remaining cannot pass even if position and velocity happen to agree.
pub fn final_state_hash(time_s: f64, state: &[f64; 6], mass_kg: f64) -> [u8; 32] {
    let mut bytes = Vec::with_capacity(64);
    put_f64(&mut bytes, time_s);
    for value in state {
        put_f64(&mut bytes, *value);
    }
    put_f64(&mut bytes, mass_kg);
    Sha256::digest(&bytes).into()
}

/// Compare two recorded floats bit for bit, treating any NaN as equal to any
/// other NaN because the canonical form deliberately erases NaN payloads.
pub fn same_metric(a: f64, b: f64) -> bool {
    if a.is_nan() && b.is_nan() {
        return true;
    }
    a.to_bits() == b.to_bits()
}

impl Fdr {
    /// The exact bytes covered by the signature. Everything a verifier or a
    /// reader acts on is here; the public key and signature are not.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(1024);
        out.extend_from_slice(self.sign_domain());
        put_u16(&mut out, self.schema_version);
        put_u32(&mut out, self.contract_id);
        out.push(self.attempt_mode as u8);
        put_u16(&mut out, self.callsign.len() as u16);
        out.extend_from_slice(self.callsign.as_bytes());
        put_i64(&mut out, self.epoch_s);
        put_i64(&mut out, self.duration_us);
        put_f64(&mut out, self.initial_mass_kg);
        put_f64(&mut out, self.dry_mass_kg);
        put_f64(&mut out, self.thrust_n);
        put_f64(&mut out, self.specific_impulse_s);
        if self.schema_version >= SCHEMA_VERSION {
            // The design, not the claim. Six fields, fixed order.
            let engine = self.engine.unwrap_or(UNSIGNABLE_ENGINE);
            out.push(engine.propellant as u8);
            put_f64(&mut out, engine.of_ratio);
            put_f64(&mut out, engine.p_c_pa);
            put_f64(&mut out, engine.r_throat_m);
            put_f64(&mut out, engine.r_exit_m);
            put_f64(&mut out, engine.length_fraction);
        }
        for value in &self.initial_state {
            put_f64(&mut out, *value);
        }
        out.push(u8::from(self.node_metric));
        for value in &self.prediction_baseline {
            put_f64(&mut out, *value);
        }
        put_u16(&mut out, self.commands.len() as u16);
        for command in &self.commands {
            put_i64(&mut out, command.time_us);
            for value in &command.delta_v_tnw {
                put_f64(&mut out, *value);
            }
            match command.guidance {
                Some(target) => {
                    out.push(1);
                    for value in &target {
                        put_f64(&mut out, *value);
                    }
                }
                None => out.push(0),
            }
        }
        for value in &self.metrics {
            put_f64(&mut out, *value);
        }
        out.extend_from_slice(&self.final_state_hash);
        put_f64(&mut out, self.elo_before);
        put_f64(&mut out, self.elo_after);
        out
    }

    /// Domain separator for this record's schema.
    pub fn sign_domain(&self) -> &'static [u8] {
        if self.schema_version <= LEGACY_SCHEMA_VERSION {
            LEGACY_SIGN_DOMAIN
        } else {
            SIGN_DOMAIN
        }
    }

    /// True when the engine was never certified by this build's physics.
    pub fn is_legacy(&self) -> bool {
        self.schema_version <= LEGACY_SCHEMA_VERSION
    }

    /// SHA-256 of the canonical bytes: the digest printed in the dossier.
    pub fn digest(&self) -> [u8; 32] {
        Sha256::digest(self.canonical_bytes()).into()
    }

    pub fn validate_shape(&self) -> Result<(), FdrError> {
        if self.schema_version != SCHEMA_VERSION && self.schema_version != LEGACY_SCHEMA_VERSION {
            return Err(FdrError::UnsupportedSchema(self.schema_version));
        }
        if self.schema_version >= SCHEMA_VERSION {
            let Some(engine) = &self.engine else {
                return Err(FdrError::Malformed("engine design missing"));
            };
            if engine.validate().is_err() {
                return Err(FdrError::EngineUnusable);
            }
        } else if self.engine.is_some() {
            return Err(FdrError::Malformed("legacy record carries an engine"));
        }
        if self.commands.len() > MAX_COMMANDS {
            return Err(FdrError::Malformed("too many commands"));
        }
        if self.callsign.is_empty() || self.callsign.len() > MAX_CALLSIGN {
            return Err(FdrError::Malformed("callsign length"));
        }
        if !self
            .callsign
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b' ' || b == b'-' || b == b'_')
        {
            return Err(FdrError::Malformed("callsign charset"));
        }
        if self.duration_us <= 0 {
            return Err(FdrError::Malformed("duration"));
        }
        // The JSON transport prints every non-finite value as null and the
        // reader maps null back to NaN. An infinity therefore cannot survive
        // its own transport: the document would verify locally and then fail
        // with SignatureMismatch after a round trip. Reject it here instead,
        // by name. NaN stays legal only where the format documents a field as
        // possibly undefined (the report and the prediction baseline).
        for (label, value) in [
            ("initial mass", self.initial_mass_kg),
            ("dry mass", self.dry_mass_kg),
            ("thrust", self.thrust_n),
            ("specific impulse", self.specific_impulse_s),
            ("elo before", self.elo_before),
            ("elo after", self.elo_after),
        ] {
            if !value.is_finite() {
                return Err(FdrError::Malformed(label));
            }
        }
        if !self.initial_state.iter().all(|value| value.is_finite()) {
            return Err(FdrError::Malformed("initial state"));
        }
        for value in self.metrics.iter().chain(self.prediction_baseline.iter()) {
            if value.is_infinite() {
                return Err(FdrError::Malformed("infinite metric or prediction"));
            }
        }
        for command in &self.commands {
            if command.time_us <= 0 || command.time_us > self.duration_us {
                return Err(FdrError::Malformed("command epoch outside the flight"));
            }
            if !command.delta_v_tnw.iter().all(|value| value.is_finite()) {
                return Err(FdrError::Malformed("command delta-v"));
            }
            if let Some(target) = &command.guidance {
                if !target.iter().all(|value| value.is_finite()) {
                    return Err(FdrError::Malformed("command guidance"));
                }
            }
        }
        Ok(())
    }

    /// Sign with a flight license. The public key is taken from the license,
    /// so a document cannot claim a different signer than the key it carries.
    pub fn sign(&mut self, license: &License) -> Result<(), FdrError> {
        self.validate_shape()?;
        self.public_key = license.public_key();
        self.signature = license.sign(&self.canonical_bytes());
        Ok(())
    }

    pub fn verify_signature(&self) -> Result<(), FdrError> {
        self.validate_shape()?;
        odd_identity::verify(&self.public_key, &self.canonical_bytes(), &self.signature)
            .map_err(|_| FdrError::SignatureMismatch)
    }

    /// Compare a replayed report against the recorded one. The first
    /// differing field is named so a rejection says what failed.
    pub fn compare_metrics(&self, replayed: &[f64; METRIC_COUNT]) -> Result<(), FdrError> {
        for (index, (recorded, actual)) in self.metrics.iter().zip(replayed.iter()).enumerate() {
            if !same_metric(*recorded, *actual) {
                return Err(FdrError::PhysicsReplayMismatch {
                    field: "metrics",
                    index,
                });
            }
        }
        Ok(())
    }

    pub fn compare_final_state(&self, hash: &[u8; 32]) -> Result<(), FdrError> {
        if &self.final_state_hash == hash {
            Ok(())
        } else {
            Err(FdrError::FinalStateMismatch)
        }
    }

    /// Compact payload for the `#verify=` URL fragment and the QR code.
    /// Base64url without padding, so it survives a fragment and a QR byte
    /// segment without escaping.
    pub fn verify_payload(&self) -> String {
        let mut bytes = self.canonical_bytes();
        bytes.extend_from_slice(&self.public_key);
        bytes.extend_from_slice(&self.signature);
        format!("#verify={}", crate::base64::encode_url(&bytes))
    }

    pub fn from_verify_payload(payload: &str) -> Result<Self, FdrError> {
        let encoded = payload
            .trim()
            .trim_start_matches("#verify=")
            .split('#')
            .next_back()
            .unwrap_or("");
        let encoded = encoded.strip_prefix("verify=").unwrap_or(encoded);
        let bytes = crate::base64::decode_url(encoded)
            .ok_or(FdrError::Malformed("verify payload is not base64url"))?;
        Self::from_canonical_with_signature(&bytes)
    }

    /// Rebuild from canonical bytes followed by the 32-byte public key and
    /// the 64-byte signature. This is the only parser, and it reads the same
    /// layout `canonical_bytes` writes, so the two cannot drift.
    pub fn from_canonical_with_signature(bytes: &[u8]) -> Result<Self, FdrError> {
        let mut reader = Reader::new(bytes);
        // The domain is read first so a legacy document is parsed with the
        // legacy layout instead of misreading its engine block.
        let legacy = if bytes.starts_with(LEGACY_SIGN_DOMAIN) {
            reader.expect(LEGACY_SIGN_DOMAIN)?;
            true
        } else {
            reader.expect(SIGN_DOMAIN)?;
            false
        };
        let schema_version = reader.u16()?;
        if schema_version != SCHEMA_VERSION && schema_version != LEGACY_SCHEMA_VERSION {
            return Err(FdrError::UnsupportedSchema(schema_version));
        }
        if legacy != (schema_version == LEGACY_SCHEMA_VERSION) {
            return Err(FdrError::Malformed("schema and signing domain disagree"));
        }
        let contract_id = reader.u32()?;
        let attempt_mode =
            AttemptMode::from_u8(reader.u8()?).ok_or(FdrError::Malformed("attempt mode"))?;
        let callsign_len = reader.u16()? as usize;
        if callsign_len == 0 || callsign_len > MAX_CALLSIGN {
            return Err(FdrError::Malformed("callsign length"));
        }
        let callsign = String::from_utf8(reader.take(callsign_len)?.to_vec())
            .map_err(|_| FdrError::Malformed("callsign encoding"))?;
        let epoch_s = reader.i64()?;
        let duration_us = reader.i64()?;
        let initial_mass_kg = reader.f64()?;
        let dry_mass_kg = reader.f64()?;
        let thrust_n = reader.f64()?;
        let specific_impulse_s = reader.f64()?;
        let engine = if legacy {
            None
        } else {
            let propellant =
                Propellant::from_u8(reader.u8()?).ok_or(FdrError::Malformed("propellant pair"))?;
            Some(EngineSpec {
                propellant,
                of_ratio: reader.f64()?,
                p_c_pa: reader.f64()?,
                r_throat_m: reader.f64()?,
                r_exit_m: reader.f64()?,
                length_fraction: reader.f64()?,
            })
        };
        let initial_state = reader.array::<6>()?;
        let node_metric = match reader.u8()? {
            0 => false,
            1 => true,
            _ => return Err(FdrError::Malformed("node metric flag")),
        };
        let prediction_baseline = reader.array::<6>()?;
        let command_count = reader.u16()? as usize;
        if command_count > MAX_COMMANDS {
            return Err(FdrError::Malformed("too many commands"));
        }
        let mut commands = Vec::with_capacity(command_count);
        for _ in 0..command_count {
            let time_us = reader.i64()?;
            let delta_v_tnw = reader.array::<3>()?;
            let guidance = match reader.u8()? {
                0 => None,
                1 => Some(reader.array::<6>()?),
                _ => return Err(FdrError::Malformed("guidance flag")),
            };
            commands.push(Command {
                time_us,
                delta_v_tnw,
                guidance,
            });
        }
        let metrics = reader.array::<METRIC_COUNT>()?;
        let final_state_hash = reader.bytes::<32>()?;
        let elo_before = reader.f64()?;
        let elo_after = reader.f64()?;
        let public_key = reader.bytes::<32>()?;
        let signature = reader.bytes::<64>()?;
        if !reader.at_end() {
            return Err(FdrError::Malformed("trailing bytes"));
        }
        let fdr = Self {
            schema_version,
            contract_id,
            attempt_mode,
            callsign,
            epoch_s,
            duration_us,
            initial_mass_kg,
            dry_mass_kg,
            thrust_n,
            specific_impulse_s,
            engine,
            initial_state,
            node_metric,
            prediction_baseline,
            commands,
            metrics,
            final_state_hash,
            elo_before,
            elo_after,
            public_key,
            signature,
        };
        fdr.validate_shape()?;
        Ok(fdr)
    }

    /// Deterministic JSON transport. Fixed key order, no insignificant
    /// whitespace decisions left to a serializer, and floats printed
    /// round-trip exactly so a parse reproduces the same bits.
    pub fn to_json(&self) -> String {
        let mut json = Json::new();
        json.object(|o| {
            o.number("schema_version", self.schema_version as f64);
            o.number("contract_id", self.contract_id as f64);
            o.string("attempt_mode", self.attempt_mode.label());
            o.string("callsign", &self.callsign);
            o.string("public_id", &odd_identity::public_id(&self.public_key));
            o.number("epoch_s", self.epoch_s as f64);
            o.number("duration_us", self.duration_us as f64);
            o.number("initial_mass_kg", self.initial_mass_kg);
            o.number("dry_mass_kg", self.dry_mass_kg);
            o.number("thrust_n", self.thrust_n);
            o.number("specific_impulse_s", self.specific_impulse_s);
            match &self.engine {
                Some(engine) => {
                    o.string("engine_propellant", engine.propellant.label());
                    o.number("engine_propellant_id", engine.propellant as u8 as f64);
                    o.number("engine_of_ratio", engine.of_ratio);
                    o.number("engine_p_c_pa", engine.p_c_pa);
                    o.number("engine_r_throat_m", engine.r_throat_m);
                    o.number("engine_r_exit_m", engine.r_exit_m);
                    o.number("engine_length_fraction", engine.length_fraction);
                }
                None => o.string("engine_propellant", "legacy (uncertified)"),
            }
            o.array("initial_state", &self.initial_state);
            o.boolean("node_metric", self.node_metric);
            o.array("prediction_baseline", &self.prediction_baseline);
            o.commands("command_timeline", &self.commands);
            o.array("metrics", &self.metrics);
            o.string(
                "final_state_hash",
                &crate::hex::encode(&self.final_state_hash),
            );
            o.number("elo_before", self.elo_before);
            o.number("elo_after", self.elo_after);
            o.string("public_key", &crate::hex::encode(&self.public_key));
            o.string("ed25519_signature", &crate::hex::encode(&self.signature));
            o.string("canonical_digest", &crate::hex::encode(&self.digest()));
            o.string("verify_payload", &self.verify_payload());
        });
        json.finish()
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, at: 0 }
    }
    fn take(&mut self, count: usize) -> Result<&'a [u8], FdrError> {
        let end = self
            .at
            .checked_add(count)
            .ok_or(FdrError::Malformed("length overflow"))?;
        let slice = self
            .bytes
            .get(self.at..end)
            .ok_or(FdrError::Malformed("truncated"))?;
        self.at = end;
        Ok(slice)
    }
    fn expect(&mut self, expected: &[u8]) -> Result<(), FdrError> {
        // A short input is a wrong document, not a truncated one: report the
        // domain mismatch so a paste of arbitrary text reads sensibly.
        let end = self.at.saturating_add(expected.len());
        match self.bytes.get(self.at..end) {
            Some(slice) if slice == expected => {
                self.at = end;
                Ok(())
            }
            _ => Err(FdrError::Malformed("not an Odderon flight data recorder")),
        }
    }
    fn u8(&mut self) -> Result<u8, FdrError> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, FdrError> {
        Ok(u16::from_be_bytes(self.bytes::<2>()?))
    }
    fn u32(&mut self) -> Result<u32, FdrError> {
        Ok(u32::from_be_bytes(self.bytes::<4>()?))
    }
    fn i64(&mut self) -> Result<i64, FdrError> {
        Ok(i64::from_be_bytes(self.bytes::<8>()?))
    }
    fn f64(&mut self) -> Result<f64, FdrError> {
        Ok(f64::from_bits(u64::from_be_bytes(self.bytes::<8>()?)))
    }
    fn bytes<const N: usize>(&mut self) -> Result<[u8; N], FdrError> {
        let mut out = [0u8; N];
        out.copy_from_slice(self.take(N)?);
        Ok(out)
    }
    fn array<const N: usize>(&mut self) -> Result<[f64; N], FdrError> {
        let mut out = [0.0; N];
        for value in &mut out {
            *value = self.f64()?;
        }
        Ok(out)
    }
    fn at_end(&self) -> bool {
        self.at == self.bytes.len()
    }
}
