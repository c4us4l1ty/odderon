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

/// Schema 3 is the only schema this build issues. It adds the physical
/// cryogenic fit-out, the Chief-AI delegation bitmask and the exact vent
/// history to the signed bytes.
///
/// **This schema is frozen.** `odd-worker` and every later work package build
/// against it; no field may be added, removed or reordered without a new
/// schema and a migration note in `docs/fdr-schema-changelog.md`.
///
/// Older records still parse and verify: schema 2 carries a certified engine
/// but no tanks, schema 1 carries neither and is labelled legacy.
pub const SCHEMA_VERSION: u16 = 3;
/// First schema whose engine block is a physical design rather than a claim.
pub const ENGINE_SCHEMA_VERSION: u16 = 2;
/// Last schema whose engine was an unverified claim.
pub const LEGACY_SCHEMA_VERSION: u16 = 1;
pub const METRIC_COUNT: usize = 58;
/// Domain separator for the signed bytes. Distinct from the key-derivation
/// domain so a license signature can never be replayed as a flight signature.
pub const SIGN_DOMAIN: &[u8] = b"odderon-flight-data-recorder-v3\0";
/// Domain of schema-2 records (engine, no tanks). Kept distinct so a v2
/// signature can never be replayed as a v3 signature.
pub const ENGINE_SIGN_DOMAIN: &[u8] = b"odderon-flight-data-recorder-v2\0";
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
/// Vent openings are rare — a relief valve opens once and stays open while
/// the net heat is positive — but the signed record must stay bounded so a
/// dossier cannot be inflated past the QR transport's capacity.
pub const MAX_VENT_EVENTS: usize = 256;
/// A stage has an oxidizer tank and a fuel tank.
pub const TANKS_PER_STAGE: usize = 2;

/// The physical cryogenic fit-out of one stage. Every field is a design
/// choice or a material property; **none is a performance claim**. The
/// verifier recomputes boil-off from these, exactly as it recomputes thrust
/// and Isp from the engine design.
///
/// The work-package card listed `cryocooler_cop` here. A raw COP is a
/// derived performance number, and trusting one from the client is precisely
/// what the no-magic-lookup law forbids — a claimed COP of 0.1 at 20 K/300 K
/// would exceed Carnot by 40%. The signed field is therefore the machine's
/// **Carnot fraction**, and the COP is recomputed from it and the radiator
/// geometry. The dossier prints the resulting COP for readability.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StageThermalSpec {
    pub stage: u8,
    /// Oxidizer then fuel tank volume, m³.
    pub tank_volumes_m3: [f64; TANKS_PER_STAGE],
    /// Propellant pair wire id, matching `EngineSpec::propellant`.
    pub propellant_id: u8,
    pub mli_layers: u32,
    pub epsilon_eff: f64,
    pub tank_shell_material: u8,
    pub absorptivity: f64,
    pub outer_epsilon: f64,
    pub strut_conductance_w_k: f64,
    pub relief_pressure_pa: f64,
    pub radiator_area_m2: f64,
    pub radiator_epsilon: f64,
    /// Commanded heat lift at the cold tip, W.
    pub cryocooler_w: f64,
    /// Fraction of Carnot the machine achieves, in (0, 1].
    pub cryocooler_carnot_fraction: f64,
    pub array_w_at_1au: f64,
    pub avionics_w: f64,
    pub electric_propulsion_w: f64,
    pub structure_temperature_k: f64,
    pub sink_temperature_k: f64,
}

/// One relief-valve opening, at an exact microsecond. The verifier replays
/// the tank ODE and requires this list to match event for event.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VentEvent {
    pub time_us: i64,
    /// 0 = oxidizer, 1 = fuel.
    pub tank: u8,
    pub pressure_pa: f64,
    pub temperature_k: f64,
}

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
    /// Per-stage cryogenic fit-out. Empty for a storable stage, and absent
    /// (schema < 3) for records issued before WP 1.6.
    pub thermal_specs: Vec<StageThermalSpec>,
    /// Which disciplines the Chief-AI sized. Bit 0 Propulsion, 1 Thermal,
    /// 2 Power, 3 GNC. Only the low four bits may be set.
    pub delegation_mask: u8,
    /// Every relief-valve opening, recomputed and compared on replay.
    pub vent_events: Vec<VentEvent>,
    /// Total mass vented per tank, kg (oxidizer then fuel).
    ///
    /// The event *list* alone is not enough: a tank that starts at its relief
    /// setting opens at t = 0 and stays open, so a worse blanket produces the
    /// same two openings while losing far more propellant. The integrated
    /// loss is therefore signed and compared too.
    pub boil_off_kg: [f64; TANKS_PER_STAGE],
    /// Electricity the cryocoolers drew over the flight, J.
    pub cryocooler_energy_j: f64,
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
    /// A delegated discipline's signed hardware is not the chief's
    /// textbook choice, so the record claims delegation it did not use.
    DelegationSpecMismatch(&'static str),
    /// The engine design itself is outside the model's validated envelope.
    EngineUnusable,
    /// The record signs a framing condition the contract fixes. A contract
    /// defines where the flight starts, when, and for how long; a player who
    /// could choose those could start on the target orbit and score a perfect
    /// rescue without doing anything.
    ContractMismatch(&'static str),
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
            Self::DelegationSpecMismatch(department) => write!(
                f,
                "DelegationSpecMismatch: {department} is marked AUTO-DELEGATED but its signed hardware is not the chief's textbook choice"
            ),
            Self::EngineUnusable => {
                write!(f, "EngineUnusable: the signed engine design does not solve")
            }
            Self::ContractMismatch(field) => write!(
                f,
                "ContractMismatch: recorded {field} is not what the contract specifies"
            ),
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
        if self.schema_version >= ENGINE_SCHEMA_VERSION {
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
        if self.schema_version >= SCHEMA_VERSION {
            // --- FROZEN v3 BLOCK. Order and width are part of the schema. ---
            out.push(self.delegation_mask);
            put_u16(&mut out, self.thermal_specs.len() as u16);
            for spec in &self.thermal_specs {
                out.push(spec.stage);
                for volume in &spec.tank_volumes_m3 {
                    put_f64(&mut out, *volume);
                }
                out.push(spec.propellant_id);
                put_u32(&mut out, spec.mli_layers);
                put_f64(&mut out, spec.epsilon_eff);
                out.push(spec.tank_shell_material);
                put_f64(&mut out, spec.absorptivity);
                put_f64(&mut out, spec.outer_epsilon);
                put_f64(&mut out, spec.strut_conductance_w_k);
                put_f64(&mut out, spec.relief_pressure_pa);
                put_f64(&mut out, spec.radiator_area_m2);
                put_f64(&mut out, spec.radiator_epsilon);
                put_f64(&mut out, spec.cryocooler_w);
                put_f64(&mut out, spec.cryocooler_carnot_fraction);
                put_f64(&mut out, spec.array_w_at_1au);
                put_f64(&mut out, spec.avionics_w);
                put_f64(&mut out, spec.electric_propulsion_w);
                put_f64(&mut out, spec.structure_temperature_k);
                put_f64(&mut out, spec.sink_temperature_k);
            }
            for value in &self.boil_off_kg {
                put_f64(&mut out, *value);
            }
            put_f64(&mut out, self.cryocooler_energy_j);
            put_u16(&mut out, self.vent_events.len() as u16);
            for event in &self.vent_events {
                put_i64(&mut out, event.time_us);
                out.push(event.tank);
                put_f64(&mut out, event.pressure_pa);
                put_f64(&mut out, event.temperature_k);
            }
        }
        out
    }

    /// Domain separator for this record's schema.
    pub fn sign_domain(&self) -> &'static [u8] {
        match self.schema_version {
            0..=LEGACY_SCHEMA_VERSION => LEGACY_SIGN_DOMAIN,
            ENGINE_SCHEMA_VERSION => ENGINE_SIGN_DOMAIN,
            _ => SIGN_DOMAIN,
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
        if !matches!(
            self.schema_version,
            LEGACY_SCHEMA_VERSION | ENGINE_SCHEMA_VERSION | SCHEMA_VERSION
        ) {
            return Err(FdrError::UnsupportedSchema(self.schema_version));
        }
        if self.schema_version >= ENGINE_SCHEMA_VERSION {
            let Some(engine) = &self.engine else {
                return Err(FdrError::Malformed("engine design missing"));
            };
            if engine.validate().is_err() {
                return Err(FdrError::EngineUnusable);
            }
        } else if self.engine.is_some() {
            return Err(FdrError::Malformed("legacy record carries an engine"));
        }
        if self.schema_version >= SCHEMA_VERSION {
            if self.delegation_mask & !0b1111 != 0 {
                return Err(FdrError::Malformed("delegation mask bits"));
            }
            if self.vent_events.len() > MAX_VENT_EVENTS {
                return Err(FdrError::Malformed("too many vent events"));
            }
            if !self.cryocooler_energy_j.is_finite() || self.cryocooler_energy_j < 0.0 {
                return Err(FdrError::Malformed("cryocooler energy"));
            }
            for value in &self.boil_off_kg {
                if !value.is_finite() || *value < 0.0 {
                    return Err(FdrError::Malformed("boil-off mass"));
                }
            }
            for spec in &self.thermal_specs {
                if spec.tank_shell_material > 2 {
                    return Err(FdrError::Malformed("tank shell material"));
                }
                if spec.propellant_id > 3 {
                    return Err(FdrError::Malformed("tank propellant id"));
                }
                if !(0.0..1.0).contains(&spec.epsilon_eff)
                    || !(0.0..=1.0).contains(&spec.absorptivity)
                    || !(0.0..=1.0).contains(&spec.outer_epsilon)
                    || !(0.0..=1.0).contains(&spec.radiator_epsilon)
                    || spec.cryocooler_carnot_fraction <= 0.0
                    || spec.cryocooler_carnot_fraction > 1.0
                {
                    return Err(FdrError::Malformed("thermal spec range"));
                }
                for value in [
                    spec.tank_volumes_m3[0],
                    spec.tank_volumes_m3[1],
                    spec.strut_conductance_w_k,
                    spec.relief_pressure_pa,
                    spec.radiator_area_m2,
                    spec.cryocooler_w,
                    spec.array_w_at_1au,
                    spec.avionics_w,
                    spec.electric_propulsion_w,
                    spec.structure_temperature_k,
                    spec.sink_temperature_k,
                ] {
                    if !value.is_finite() || value < 0.0 {
                        return Err(FdrError::Malformed("thermal spec value"));
                    }
                }
                if spec.relief_pressure_pa <= 0.0
                    || spec.structure_temperature_k <= 0.0
                    || spec.sink_temperature_k <= 0.0
                {
                    return Err(FdrError::Malformed("thermal spec non-positive"));
                }
            }
            let mut previous = 0i64;
            for event in &self.vent_events {
                if event.time_us < previous || event.time_us > self.duration_us {
                    return Err(FdrError::Malformed("vent event order or epoch"));
                }
                if event.tank as usize >= TANKS_PER_STAGE {
                    return Err(FdrError::Malformed("vent event tank id"));
                }
                if !event.pressure_pa.is_finite()
                    || event.pressure_pa <= 0.0
                    || !event.temperature_k.is_finite()
                    || event.temperature_k <= 0.0
                {
                    return Err(FdrError::Malformed("vent event value"));
                }
                previous = event.time_us;
            }
        } else if !self.thermal_specs.is_empty() || !self.vent_events.is_empty() {
            return Err(FdrError::Malformed("pre-v3 record carries tanks"));
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

    /// Compare a replayed vent history against the recorded one.
    ///
    /// The thermal spec is signed, but so is the * consequence * of that
    /// spec: a client cannot claim a smaller loss than its own hardware
    /// produces. Timestamps must match to the microsecond and the pressures
    /// and temperatures to the bit, because both sides ran the same
    /// deterministic ODE.
    /// Compare the integrated boil-off and cooler draw against a replay.
    /// Bit-exact: both sides ran the same deterministic ODE.
    pub fn compare_boil_off(
        &self,
        replayed_kg: [f64; TANKS_PER_STAGE],
        replayed_energy_j: f64,
    ) -> Result<(), FdrError> {
        if self.schema_version < SCHEMA_VERSION {
            return Ok(());
        }
        for (index, (recorded, actual)) in self.boil_off_kg.iter().zip(replayed_kg).enumerate() {
            if recorded.to_bits() != actual.to_bits() {
                return Err(FdrError::PhysicsReplayMismatch {
                    field: "boil_off_kg",
                    index,
                });
            }
        }
        if self.cryocooler_energy_j.to_bits() != replayed_energy_j.to_bits() {
            return Err(FdrError::PhysicsReplayMismatch {
                field: "cryocooler_energy_j",
                index: 0,
            });
        }
        Ok(())
    }

    pub fn compare_vent_events(&self, replayed: &[VentEvent]) -> Result<(), FdrError> {
        if self.schema_version < SCHEMA_VERSION {
            return Ok(());
        }
        if self.vent_events.len() != replayed.len() {
            return Err(FdrError::PhysicsReplayMismatch {
                field: "vent_events.len",
                index: 0,
            });
        }
        for (index, (recorded, actual)) in self.vent_events.iter().zip(replayed).enumerate() {
            if recorded.time_us != actual.time_us || recorded.tank != actual.tank {
                return Err(FdrError::PhysicsReplayMismatch {
                    field: "vent_events.time_us",
                    index,
                });
            }
            if recorded.pressure_pa.to_bits() != actual.pressure_pa.to_bits()
                || recorded.temperature_k.to_bits() != actual.temperature_k.to_bits()
            {
                return Err(FdrError::PhysicsReplayMismatch {
                    field: "vent_events.state",
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
        let domain: &[u8] = if bytes.starts_with(LEGACY_SIGN_DOMAIN) {
            LEGACY_SIGN_DOMAIN
        } else if bytes.starts_with(ENGINE_SIGN_DOMAIN) {
            ENGINE_SIGN_DOMAIN
        } else {
            SIGN_DOMAIN
        };
        reader.expect(domain)?;
        let schema_version = reader.u16()?;
        if !matches!(
            schema_version,
            LEGACY_SCHEMA_VERSION | ENGINE_SCHEMA_VERSION | SCHEMA_VERSION
        ) {
            return Err(FdrError::UnsupportedSchema(schema_version));
        }
        let legacy = schema_version == LEGACY_SCHEMA_VERSION;
        let expected_domain = match schema_version {
            LEGACY_SCHEMA_VERSION => LEGACY_SIGN_DOMAIN,
            ENGINE_SCHEMA_VERSION => ENGINE_SIGN_DOMAIN,
            _ => SIGN_DOMAIN,
        };
        if domain != expected_domain {
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
        // --- FROZEN v3 BLOCK ---
        let (thermal_specs, delegation_mask, vent_events, boil_off_kg, cryocooler_energy_j) =
            if schema_version >= SCHEMA_VERSION {
                let mask = reader.u8()?;
                let spec_count = reader.u16()? as usize;
                if spec_count > 16 {
                    return Err(FdrError::Malformed("too many thermal specs"));
                }
                let mut specs = Vec::with_capacity(spec_count);
                for _ in 0..spec_count {
                    let stage = reader.u8()?;
                    let tank_volumes_m3 = reader.array::<TANKS_PER_STAGE>()?;
                    let propellant_id = reader.u8()?;
                    let mli_layers = reader.u32()?;
                    let epsilon_eff = reader.f64()?;
                    let tank_shell_material = reader.u8()?;
                    let absorptivity = reader.f64()?;
                    let outer_epsilon = reader.f64()?;
                    let strut_conductance_w_k = reader.f64()?;
                    let relief_pressure_pa = reader.f64()?;
                    let radiator_area_m2 = reader.f64()?;
                    let radiator_epsilon = reader.f64()?;
                    let cryocooler_w = reader.f64()?;
                    let cryocooler_carnot_fraction = reader.f64()?;
                    let array_w_at_1au = reader.f64()?;
                    let avionics_w = reader.f64()?;
                    let electric_propulsion_w = reader.f64()?;
                    let structure_temperature_k = reader.f64()?;
                    let sink_temperature_k = reader.f64()?;
                    specs.push(StageThermalSpec {
                        stage,
                        tank_volumes_m3,
                        propellant_id,
                        mli_layers,
                        epsilon_eff,
                        tank_shell_material,
                        absorptivity,
                        outer_epsilon,
                        strut_conductance_w_k,
                        relief_pressure_pa,
                        radiator_area_m2,
                        radiator_epsilon,
                        cryocooler_w,
                        cryocooler_carnot_fraction,
                        array_w_at_1au,
                        avionics_w,
                        electric_propulsion_w,
                        structure_temperature_k,
                        sink_temperature_k,
                    });
                }
                let boil_off_kg = reader.array::<TANKS_PER_STAGE>()?;
                let cryocooler_energy_j = reader.f64()?;
                let event_count = reader.u16()? as usize;
                if event_count > MAX_VENT_EVENTS {
                    return Err(FdrError::Malformed("too many vent events"));
                }
                let mut events = Vec::with_capacity(event_count);
                for _ in 0..event_count {
                    let time_us = reader.i64()?;
                    let tank = reader.u8()?;
                    let pressure_pa = reader.f64()?;
                    let temperature_k = reader.f64()?;
                    events.push(VentEvent {
                        time_us,
                        tank,
                        pressure_pa,
                        temperature_k,
                    });
                }
                (specs, mask, events, boil_off_kg, cryocooler_energy_j)
            } else {
                (Vec::new(), 0, Vec::new(), [0.0; TANKS_PER_STAGE], 0.0)
            };
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
            thermal_specs,
            delegation_mask,
            vent_events,
            boil_off_kg,
            cryocooler_energy_j,
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
            o.number("delegation_mask", self.delegation_mask as f64);
            o.thermal_specs("thermal_specs", &self.thermal_specs);
            o.vent_events("vent_events", &self.vent_events);
            o.array("boil_off_kg", &self.boil_off_kg);
            o.number("cryocooler_energy_j", self.cryocooler_energy_j);
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
