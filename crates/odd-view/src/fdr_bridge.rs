//! Signed flight recorder boundary: capture, sign, export, verify, replay.
//!
//! Replay is not a second implementation. `verify_and_replay` rebuilds the
//! settings from the recorder's typed fields and calls the same `compute`
//! the live flight used, then compares all 58 fields and the terminal-state
//! hash bit for bit. A document therefore cannot pass by describing a
//! different flight in prose.
//!
//! Transport uses the same scalar conventions as the rest of the bridge:
//! numbers arrive through a fixed-layout f64 array, text through the shared
//! byte channel, and strings leave as length + byte.
use super::identity_bridge;
use super::mission_bridge::{self as bridge};
use odd_report::{AttemptMode, Command, Fdr, FdrError, FROZEN_V3_SCHEMA_VERSION, SCHEMA_VERSION};

/// First schema whose record carries the coupled-mass block.
const V4_SCHEMA_VERSION: u16 = SCHEMA_VERSION;
use odd_verify::REPORT_LEN;
use std::sync::Mutex;

/// Fixed header layout of the transport array. Commands follow at
/// `COMMAND_OFFSET` with a stride of `COMMAND_STRIDE`, then the metrics,
/// the terminal hash, the public key and the signature.
///
/// 0 schema, 1 contract, 2 mode, 3 epoch_s, 4 duration_us, 5 initial mass,
/// 6 dry mass, 7 thrust, 8 isp, 9..15 initial state, 15 node metric,
/// 16..22 prediction, 22 command count, 23 elo before, 24 elo after,
/// 25 propellant id, 26 O/F, 27 chamber pressure, 28 throat radius,
/// 29 exit radius, 30 bell length fraction.
///
/// The engine block is present for schema 2 and all-NaN for a legacy
/// schema-1 document, so one layout serves both.
const HEADER: usize = 31;
const COMMAND_OFFSET: usize = HEADER;
/// time_us, dvT, dvN, dvW, guided flag, six guidance components.
const COMMAND_STRIDE: usize = 11;
const MAX_COMMANDS: usize = 12;
/// v3 trailer, appended after the signature so every historical index stays
/// put: mask, spec count, the specs, vent count, the vents.
const THERMAL_SPEC_STRIDE: usize = 20;
const VENT_STRIDE: usize = 4;
const MAX_THERMAL_SPECS: usize = 1;
const MAX_VENT_EVENTS: usize = odd_report::MAX_VENT_EVENTS;
const V3_TRAILER_MAX: usize =
    5 + MAX_THERMAL_SPECS * THERMAL_SPEC_STRIDE + MAX_VENT_EVENTS * VENT_STRIDE;
/// v4 trailer, one field per entry so neither count can change the stride:
///
/// ```text
/// 0  burn window count
/// 1  flameout present (0 or 1)     2  flameout seconds
/// 3  remaining propellant present  4  remaining propellant kg
/// 5..5+2·MAX_COMMANDS  start_s, end_s pairs
/// ```
const V4_HEADER: usize = 5;
const V4_TRAILER_MAX: usize = V4_HEADER + MAX_COMMANDS * 2;
const MAX_FIELDS: usize = HEADER
    + MAX_COMMANDS * COMMAND_STRIDE
    + REPORT_LEN
    + 32
    + 32
    + 64
    + V3_TRAILER_MAX
    + V4_TRAILER_MAX;

static FIELDS: Mutex<Vec<f64>> = Mutex::new(Vec::new());
static RECORDER: Mutex<Option<Fdr>> = Mutex::new(None);
static JSON: Mutex<String> = Mutex::new(String::new());
static DOSSIER: Mutex<String> = Mutex::new(String::new());
static SERVER_RECEIPT: Mutex<Option<odd_report::dossier::ReceiptAnnex>> = Mutex::new(None);
/// The deployment page URL, supplied by the browser. A QR code has to carry
/// an absolute URL to be scannable by a phone; the module cannot know the
/// origin it is served from, so the page states it.
static ORIGIN: Mutex<String> = Mutex::new(String::new());
/// Bound the origin so a hostile page cannot inflate the QR payload.
const MAX_ORIGIN: usize = 512;
static VERDICT: Mutex<Option<Result<(), FdrError>>> = Mutex::new(None);
static REPLAYED: Mutex<Option<[f64; REPORT_LEN]>> = Mutex::new(None);
static GEOMETRY: Mutex<Option<odd_report::dossier::Geometry>> = Mutex::new(None);

#[no_mangle]
pub extern "C" fn mission_fdr_reset() {
    FIELDS.lock().unwrap().clear();
}

/// Append one transport number. NaN is allowed: undefined metrics are NaN.
#[no_mangle]
pub extern "C" fn mission_fdr_field(value: f64) -> i32 {
    let mut fields = FIELDS.lock().unwrap();
    if fields.len() >= MAX_FIELDS {
        return 0;
    }
    fields.push(value);
    1
}

#[no_mangle]
pub extern "C" fn mission_fdr_field_count() -> u32 {
    FIELDS.lock().unwrap().len() as u32
}

/// Index just past the v3 trailer, which is where the v4 block starts.
/// A pre-v3 record has no trailer at all, so the answer is the end of the
/// signature.
fn v3_end_from(fields: &[f64]) -> Option<usize> {
    let schema = integer(*fields.first()?)?;
    let metrics_at = HEADER + integer(*fields.get(22)?)? as usize * COMMAND_STRIDE;
    let hash_at = metrics_at + REPORT_LEN;
    let mut at = hash_at + 128;
    if schema < i64::from(FROZEN_V3_SCHEMA_VERSION) {
        return Some(at);
    }
    let spec_count = integer(*fields.get(at + 1)?)?;
    if !(0..=MAX_THERMAL_SPECS as i64).contains(&spec_count) {
        return None;
    }
    at += 2 + spec_count as usize * THERMAL_SPEC_STRIDE;
    at += 3;
    let vent_count = integer(*fields.get(at)?)?;
    if !(0..=MAX_VENT_EVENTS as i64).contains(&vent_count) {
        return None;
    }
    Some(at + 1 + vent_count as usize * VENT_STRIDE)
}

fn integer(value: f64) -> Option<i64> {
    if value.is_finite() && value.fract() == 0.0 && value.abs() < 9.007_199_254_740_992e15 {
        Some(value as i64)
    } else {
        None
    }
}

/// Rebuild a recorder from the transport array plus the callsign in the byte
/// channel. Returns 1 on success, 0 when the layout is impossible.
#[no_mangle]
pub extern "C" fn mission_fdr_parse() -> i32 {
    *SERVER_RECEIPT.lock().unwrap() = None;
    *VERDICT.lock().unwrap() = None;
    *GEOMETRY.lock().unwrap() = None;
    *RECORDER.lock().unwrap() = None;
    let fields = FIELDS.lock().unwrap().clone();
    let Some(fdr) = parse_fields(&fields, &identity_bridge::input_text()) else {
        return 0;
    };
    *JSON.lock().unwrap() = fdr.to_json();
    *RECORDER.lock().unwrap() = Some(fdr);
    1
}

/// Check the pilot's signature without running the expensive replay.
#[no_mangle]
pub extern "C" fn mission_fdr_signature_valid() -> i32 {
    i32::from(
        RECORDER
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|fdr| fdr.verify_signature().is_ok()),
    )
}

/// The digest is recomputed from parsed fields, never taken from a mutable
/// JSON `canonical_digest` property supplied by the browser.
#[no_mangle]
pub extern "C" fn mission_fdr_digest_byte(index: u32) -> f64 {
    RECORDER
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|fdr| fdr.digest().get(index as usize).copied())
        .map_or(f64::NAN, f64::from)
}

/// Read the signed engine design out of the transport header. A legacy
/// document carries NaN there and parses to `None`.
fn engine_from_fields(fields: &[f64]) -> Option<Option<odd_propulsion::EngineSpec>> {
    let schema = integer(fields[0])?;
    if schema < i64::from(SCHEMA_VERSION) {
        return Some(None);
    }
    let propellant = integer(fields[25])?;
    Some(Some(odd_propulsion::EngineSpec {
        propellant: odd_propulsion::Propellant::from_u8(u8::try_from(propellant).ok()?)?,
        of_ratio: fields[26],
        p_c_pa: fields[27],
        r_throat_m: fields[28],
        r_exit_m: fields[29],
        length_fraction: fields[30],
    }))
}

fn parse_fields(fields: &[f64], callsign: &str) -> Option<Fdr> {
    if fields.len() < HEADER {
        return None;
    }
    let schema = integer(fields[0])?;
    let contract = integer(fields[1])?;
    let mode = integer(fields[2])?;
    let command_count = integer(fields[22])?;
    if !(0..=MAX_COMMANDS as i64).contains(&command_count) {
        return None;
    }
    let command_count = command_count as usize;
    let metrics_at = COMMAND_OFFSET + command_count * COMMAND_STRIDE;
    let base_len = metrics_at + REPORT_LEN + 32 + 32 + 64;
    // Pre-v3 records end at the signature; v3 appends a variable-length
    // trailer whose own counts are checked below.
    if schema >= i64::from(SCHEMA_VERSION) {
        if fields.len() < base_len + 3 || fields.len() > base_len + V3_TRAILER_MAX {
            return None;
        }
    } else if fields.len() != base_len {
        return None;
    }
    let mut commands = Vec::with_capacity(command_count);
    for index in 0..command_count {
        let at = COMMAND_OFFSET + index * COMMAND_STRIDE;
        let time_us = integer(fields[at])?;
        let guided = integer(fields[at + 4])?;
        let guidance = match guided {
            0 => None,
            1 => Some(core::array::from_fn(|component| fields[at + 5 + component])),
            _ => return None,
        };
        commands.push(Command {
            time_us,
            delta_v_tnw: [fields[at + 1], fields[at + 2], fields[at + 3]],
            guidance,
        });
    }
    let bytes = |from: usize, count: usize| -> Option<Vec<u8>> {
        fields[from..from + count]
            .iter()
            .map(|value| {
                let byte = integer(*value)?;
                if (0..=255).contains(&byte) {
                    Some(byte as u8)
                } else {
                    None
                }
            })
            .collect()
    };
    let hash_at = metrics_at + REPORT_LEN;
    let final_state_hash: [u8; 32] = bytes(hash_at, 32)?.try_into().ok()?;
    let public_key: [u8; 32] = bytes(hash_at + 32, 32)?.try_into().ok()?;
    let signature: [u8; 64] = bytes(hash_at + 64, 64)?.try_into().ok()?;
    // --- v3 trailer ---
    let mut thermal_specs = Vec::new();
    let mut vent_events = Vec::new();
    let mut delegation_mask = 0u8;
    let mut parsed_loss = ([0.0; 2], 0.0);
    if schema >= i64::from(FROZEN_V3_SCHEMA_VERSION) {
        let mut at = hash_at + 128;
        delegation_mask = u8::try_from(integer(*fields.get(at)?)?).ok()?;
        let spec_count = integer(*fields.get(at + 1)?)?;
        if !(0..=MAX_THERMAL_SPECS as i64).contains(&spec_count) {
            return None;
        }
        at += 2;
        for _ in 0..spec_count {
            let f = fields.get(at..at + THERMAL_SPEC_STRIDE)?;
            thermal_specs.push(odd_report::StageThermalSpec {
                stage: u8::try_from(integer(f[0])?).ok()?,
                tank_volumes_m3: [f[1], f[2]],
                propellant_id: u8::try_from(integer(f[3])?).ok()?,
                mli_layers: u32::try_from(integer(f[4])?).ok()?,
                epsilon_eff: f[5],
                tank_shell_material: u8::try_from(integer(f[6])?).ok()?,
                absorptivity: f[7],
                outer_epsilon: f[8],
                strut_conductance_w_k: f[9],
                relief_pressure_pa: f[10],
                radiator_area_m2: f[11],
                radiator_epsilon: f[12],
                cryocooler_w: f[13],
                cryocooler_carnot_fraction: f[14],
                array_w_at_1au: f[15],
                avionics_w: f[16],
                electric_propulsion_w: f[17],
                structure_temperature_k: f[18],
                sink_temperature_k: f[19],
            });
            at += THERMAL_SPEC_STRIDE;
        }
        parsed_loss = (
            [*fields.get(at)?, *fields.get(at + 1)?],
            *fields.get(at + 2)?,
        );
        at += 3;
        let vent_count = integer(*fields.get(at)?)?;
        if !(0..=MAX_VENT_EVENTS as i64).contains(&vent_count) {
            return None;
        }
        at += 1;
        for _ in 0..vent_count {
            let f = fields.get(at..at + VENT_STRIDE)?;
            vent_events.push(odd_report::VentEvent {
                time_us: integer(f[0])?,
                tank: u8::try_from(integer(f[1])?).ok()?,
                pressure_pa: f[2],
                temperature_k: f[3],
            });
            at += VENT_STRIDE;
        }
        // A v4 record continues with the coupled-state block; a v3 record has
        // nothing after the vent list.
        if schema == i64::from(FROZEN_V3_SCHEMA_VERSION) && at != fields.len() {
            return None;
        }
    }
    // --- v4 trailer ---
    let mut flown_burn_windows = Vec::new();
    let mut flameout_time_s = None;
    let mut propellant_remaining_kg = None;
    if schema >= i64::from(V4_SCHEMA_VERSION) {
        // The v4 block begins immediately after the v3 trailer's vent list.
        let mut at = v3_end_from(fields)?;
        let count = integer(*fields.get(at)?)?;
        if !(0..=MAX_COMMANDS as i64).contains(&count) {
            return None;
        }
        at += 1;
        for _ in 0..count {
            let start = *fields.get(at)?;
            let end = *fields.get(at + 1)?;
            flown_burn_windows.push((start, end));
            at += 2;
        }
        match integer(*fields.get(at)?)? {
            0 => {}
            1 => flameout_time_s = Some(*fields.get(at + 1)?),
            _ => return None,
        }
        at += 2;
        match integer(*fields.get(at)?)? {
            0 => {}
            1 => propellant_remaining_kg = Some(*fields.get(at + 1)?),
            _ => return None,
        }
        at += 2;
        if at != fields.len() {
            return None;
        }
    }
    Some(Fdr {
        schema_version: u16::try_from(schema).ok()?,
        contract_id: u32::try_from(contract).ok()?,
        attempt_mode: AttemptMode::from_u8(u8::try_from(mode).ok()?)?,
        callsign: callsign.to_string(),
        epoch_s: integer(fields[3])?,
        duration_us: integer(fields[4])?,
        initial_mass_kg: fields[5],
        dry_mass_kg: fields[6],
        thrust_n: fields[7],
        specific_impulse_s: fields[8],
        engine: engine_from_fields(fields)?,
        initial_state: core::array::from_fn(|index| fields[9 + index]),
        node_metric: integer(fields[15])? == 1,
        prediction_baseline: core::array::from_fn(|index| fields[16 + index]),
        commands,
        metrics: core::array::from_fn(|index| fields[metrics_at + index]),
        final_state_hash,
        elo_before: fields[23],
        elo_after: fields[24],
        thermal_specs,
        delegation_mask,
        vent_events,
        boil_off_kg: parsed_loss.0,
        cryocooler_energy_j: parsed_loss.1,
        flown_burn_windows,
        flameout_time_s,
        propellant_remaining_kg,
        public_key,
        signature,
    })
}

/// Capture the executed flight as an unsigned recorder. The callsign comes
/// from the byte channel; mode 0 ranked, 1 practice.
#[no_mangle]
pub extern "C" fn mission_fdr_capture(mode: u32, elo_before: f64, elo_after: f64) -> i32 {
    *RECORDER.lock().unwrap() = None;
    JSON.lock().unwrap().clear();
    DOSSIER.lock().unwrap().clear();
    let Some(mode) = u8::try_from(mode).ok().and_then(AttemptMode::from_u8) else {
        return 0;
    };
    if !elo_before.is_finite() || !elo_after.is_finite() {
        return 0;
    }
    let Some(captured) = bridge::capture_flight() else {
        return 0;
    };
    // Only a catalog contract has a stage whose engine, dry mass and load can
    // be recomputed. A free-sandbox flight has nothing to certify against.
    if odd_contracts::Contract::get(captured.contract_id).is_none() {
        return 0;
    }
    let fdr = record_from_capture(
        captured,
        mode,
        elo_before,
        elo_after,
        identity_bridge::input_text(),
    );
    if fdr.validate_shape().is_err() {
        return 0;
    }
    *RECORDER.lock().unwrap() = Some(fdr);
    1
}

/// The common constructor is used both before and after flight, preventing
/// the commitment from drifting away from the design the recorder captures.
fn record_from_capture(
    captured: bridge::CapturedFlight,
    mode: AttemptMode,
    elo_before: f64,
    elo_after: f64,
    callsign: String,
) -> Fdr {
    Fdr {
        schema_version: SCHEMA_VERSION,
        contract_id: captured.contract_id,
        attempt_mode: mode,
        callsign,
        epoch_s: captured.epoch_s,
        duration_us: captured.duration_us,
        initial_mass_kg: captured.initial.mass,
        dry_mass_kg: captured.dry_mass,
        thrust_n: captured.thrust_n,
        specific_impulse_s: captured.specific_impulse_s,
        engine: Some(captured.engine_spec),
        initial_state: [
            captured.initial.position[0],
            captured.initial.position[1],
            captured.initial.position[2],
            captured.initial.velocity[0],
            captured.initial.velocity[1],
            captured.initial.velocity[2],
        ],
        node_metric: captured.node_metric,
        prediction_baseline: captured.prediction,
        commands: captured
            .commands
            .iter()
            .map(|command| Command {
                time_us: command.epoch_us,
                delta_v_tnw: command.tnw_m_s,
                guidance: command.guidance,
            })
            .collect(),
        metrics: captured.report,
        final_state_hash: odd_report::fdr::final_state_hash(
            captured.report[29],
            &[
                captured.final_state.position[0],
                captured.final_state.position[1],
                captured.final_state.position[2],
                captured.final_state.velocity[0],
                captured.final_state.velocity[1],
                captured.final_state.velocity[2],
            ],
            captured.final_state.mass,
        ),
        elo_before,
        elo_after,
        thermal_specs: if captured.thermal.inert {
            Vec::new()
        } else {
            vec![odd_report::StageThermalSpec {
                stage: 0,
                tank_volumes_m3: captured.tank_volumes_m3,
                propellant_id: captured.engine_spec.propellant as u8,
                mli_layers: captured.thermal_design.mli_layers,
                epsilon_eff: captured.thermal_design.epsilon_eff,
                tank_shell_material: captured.thermal_design.shell,
                absorptivity: captured.thermal_design.absorptivity,
                outer_epsilon: captured.thermal_design.outer_epsilon,
                strut_conductance_w_k: captured.thermal_design.strut_conductance_w_k,
                relief_pressure_pa: captured.thermal_design.relief_pressure_pa,
                radiator_area_m2: captured.thermal_design.radiator_area_m2,
                radiator_epsilon: captured.thermal_design.radiator_epsilon,
                cryocooler_w: captured.thermal_design.cryocooler_lift_w,
                cryocooler_carnot_fraction: captured.thermal_design.cryocooler_carnot_fraction,
                array_w_at_1au: captured.thermal_design.bus.array_w_at_1au,
                avionics_w: captured.thermal_design.bus.avionics_w,
                electric_propulsion_w: captured.thermal_design.bus.electric_propulsion_w,
                structure_temperature_k: captured.thermal_design.structure_temperature_k,
                sink_temperature_k: captured.thermal_design.sink_temperature_k,
            }]
        },
        delegation_mask: captured.delegation.0,
        boil_off_kg: captured.thermal.tank_lost_kg,
        cryocooler_energy_j: captured.thermal.cooler_energy_j,
        // v4: the flown windows and live pool come from the same flight the
        // recorder captured, so the signed record and the trajectory agree.
        flown_burn_windows: captured.flown_burn_windows.clone(),
        flameout_time_s: captured.flameout_time_s,
        propellant_remaining_kg: Some(captured.final_state.mass - captured.dry_mass),
        vent_events: captured
            .thermal
            .vents
            .iter()
            .map(|event| odd_report::VentEvent {
                time_us: event.time_us,
                tank: event.tank,
                pressure_pa: event.pressure_pa,
                temperature_k: event.temperature_k,
            })
            .collect(),
        public_key: [0; 32],
        signature: [0; 64],
    }
}

static COMMIT_HASHES: Mutex<[u8; 64]> = Mutex::new([0; 64]);

/// Hash the locked prediction and effective design before any execution. A
/// failure must stop the ranked launch rather than issue a mismatched ticket.
#[no_mangle]
pub extern "C" fn mission_commit_prepare() -> i32 {
    let Some(captured) = bridge::capture_preflight() else {
        return 0;
    };
    let fdr = record_from_capture(captured, AttemptMode::Practice, 0.0, 0.0, String::new());
    let mut hashes = COMMIT_HASHES.lock().unwrap();
    hashes[..32].copy_from_slice(&odd_worker::commitment::design_hash(&fdr));
    hashes[32..].copy_from_slice(&odd_worker::commitment::prediction_hash(&fdr));
    1
}

#[no_mangle]
pub extern "C" fn mission_commit_hash_byte(index: u32) -> f64 {
    COMMIT_HASHES
        .lock()
        .unwrap()
        .get(index as usize)
        .map_or(f64::NAN, |v| f64::from(*v))
}

/// Sign the captured recorder with the active flight license.
#[no_mangle]
pub extern "C" fn mission_fdr_sign() -> i32 {
    let Some(license) = identity_bridge::active_license() else {
        return 0;
    };
    let mut recorder = RECORDER.lock().unwrap();
    let Some(fdr) = recorder.as_mut() else {
        return 0;
    };
    if fdr.sign(&license).is_err() {
        return 0;
    }
    *JSON.lock().unwrap() = fdr.to_json();
    1
}

/// Verify the parsed recorder's signature and replay its physics.
/// Returns 1 when both agree, 0 when the document is unusable,
/// -1 SignatureMismatch, -2 metric replay mismatch, -3 terminal-state mismatch.
#[no_mangle]
pub extern "C" fn mission_fdr_verify() -> i32 {
    *SERVER_RECEIPT.lock().unwrap() = None;
    *REPLAYED.lock().unwrap() = None;
    *GEOMETRY.lock().unwrap() = None;
    let Some(fdr) = RECORDER.lock().unwrap().clone() else {
        *VERDICT.lock().unwrap() = Some(Err(FdrError::Malformed("no recorder parsed")));
        return 0;
    };
    let verdict = verify_and_replay(&fdr);
    let code = match &verdict {
        Ok(()) => 1,
        Err(FdrError::SignatureMismatch) => -1,
        Err(FdrError::PhysicsReplayMismatch { .. }) => -2,
        Err(FdrError::FinalStateMismatch) => -3,
        Err(_) => 0,
    };
    *VERDICT.lock().unwrap() = Some(verdict);
    code
}

/// The full check. Kept separate from the export so native tests can call it
/// with a constructed recorder.
pub(crate) fn verify_and_replay(fdr: &Fdr) -> Result<(), FdrError> {
    // The check itself is portable: odd_verify::verify_record is what a CLI
    // or an edge worker calls. This wrapper only caches what the UI shows.
    match odd_verify::verify_record(fdr) {
        Ok(outcome) => {
            *REPLAYED.lock().unwrap() = Some(outcome.report);
            *GEOMETRY.lock().unwrap() = Some(outcome.geometry);
            Ok(())
        }
        Err(error) => Err(error),
    }
}

/// First differing metric index, or -1. Names the field a rejection is about.
#[no_mangle]
pub extern "C" fn mission_fdr_mismatch_index() -> i32 {
    match VERDICT.lock().unwrap().as_ref() {
        Some(Err(FdrError::PhysicsReplayMismatch { index, .. })) => *index as i32,
        _ => -1,
    }
}

#[no_mangle]
pub extern "C" fn mission_fdr_replayed(index: u32) -> f64 {
    REPLAYED
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|report| report.get(index as usize).copied())
        .unwrap_or(f64::NAN)
}

fn verdict_text() -> String {
    match VERDICT.lock().unwrap().as_ref() {
        Some(Ok(())) => odd_report::VERIFIED_BANNER.to_string(),
        Some(Err(error)) => error.to_string(),
        None => String::new(),
    }
}

/// Load a server key followed by the canonical receipt and signature from the
/// byte channel. Only a valid receipt for the already-replayed record reaches
/// the report. A failed load clears a previously accepted receipt.
#[no_mangle]
pub extern "C" fn mission_fdr_receipt_load() -> i32 {
    *SERVER_RECEIPT.lock().unwrap() = None;
    if !matches!(*VERDICT.lock().unwrap(), Some(Ok(()))) {
        return 0;
    }
    let bytes = identity_bridge::input_bytes();
    let Some(key_bytes) = bytes.get(..32) else {
        return 0;
    };
    let Ok(key) = <[u8; 32]>::try_from(key_bytes) else {
        return 0;
    };
    let Some(receipt) = odd_worker::ServerReceipt::from_bytes(&bytes[32..]) else {
        return 0;
    };
    let Some(fdr) = RECORDER.lock().unwrap().clone() else {
        return 0;
    };
    let p = &receipt.payload;
    if !receipt.verify(&key)
        || p.fdr_hash != fdr.digest()
        || p.pubkey != fdr.public_key
        || p.contract_id != fdr.contract_id
        || p.mode != fdr.attempt_mode
        || p.score_s_bits != fdr.metrics[4].to_bits()
    {
        return 0;
    }
    *SERVER_RECEIPT.lock().unwrap() = Some(odd_report::dossier::ReceiptAnnex {
        server_public_key: key,
        receipt_bytes: receipt.to_bytes(),
        attempt_num: p.attempt_num,
        mode: p.mode,
        score_s: p.score_s(),
        elo_before: p.elo_before(),
        elo_after: p.elo_after(),
        mission_elo_after: p.mission_elo_after(),
        timestamp_s: p.timestamp_s,
    });
    1
}

/// Build the printable dossier for the parsed (or captured) recorder.
#[no_mangle]
pub extern "C" fn mission_fdr_dossier() -> i32 {
    DOSSIER.lock().unwrap().clear();
    let Some(fdr) = RECORDER.lock().unwrap().clone() else {
        return 0;
    };
    let Some(geometry) = bridge::dossier_geometry() else {
        return 0;
    };
    let payload = dossier_payload(&fdr, None);
    let Ok(document) = odd_report::dossier::html(&fdr, &geometry, &payload) else {
        return 0;
    };
    *DOSSIER.lock().unwrap() = document;
    1
}

/// Render a dossier for a recorder supplied through the transport array,
/// without needing a live flight. Used when opening a shared #verify= link.
#[no_mangle]
pub extern "C" fn mission_fdr_dossier_from_record() -> i32 {
    DOSSIER.lock().unwrap().clear();
    let Some(fdr) = RECORDER.lock().unwrap().clone() else {
        return 0;
    };
    // The geometry comes from the replay that verification already ran, so
    // the printed figure is the verified flight and not a second run.
    let Some(geometry) = GEOMETRY.lock().unwrap().clone() else {
        return 0;
    };
    let receipt = SERVER_RECEIPT.lock().unwrap();
    let payload = dossier_payload(&fdr, receipt.as_ref());
    let Ok(document) =
        odd_report::dossier::html_with_receipt(&fdr, &geometry, &payload, receipt.as_ref())
    else {
        return 0;
    };
    *DOSSIER.lock().unwrap() = document;
    1
}

/// Load a recorder from a `#verify=` payload in the byte channel.
#[no_mangle]
pub extern "C" fn mission_fdr_from_payload() -> i32 {
    *SERVER_RECEIPT.lock().unwrap() = None;
    *VERDICT.lock().unwrap() = None;
    *GEOMETRY.lock().unwrap() = None;
    *RECORDER.lock().unwrap() = None;
    JSON.lock().unwrap().clear();
    DOSSIER.lock().unwrap().clear();
    let Ok(fdr) = Fdr::from_verify_payload(&identity_bridge::input_text()) else {
        return 0;
    };
    *JSON.lock().unwrap() = fdr.to_json();
    *RECORDER.lock().unwrap() = Some(fdr);
    1
}

fn output_string(field: u32) -> Option<String> {
    match field {
        0 => Some(JSON.lock().unwrap().clone()),
        1 => Some(DOSSIER.lock().unwrap().clone()),
        2 => Some(verdict_text()),
        3 => Some(RECORDER.lock().unwrap().as_ref()?.verify_payload()),
        4 => Some(
            odd_report::metrics::name(mission_fdr_mismatch_index().max(0) as usize).to_string(),
        ),
        _ => None,
    }
}

/// Length of an output string: 0 JSON, 1 dossier HTML, 2 verdict,
/// 3 verify payload, 4 mismatched field name.
#[no_mangle]
pub extern "C" fn mission_fdr_string_len(field: u32) -> u32 {
    output_string(field).map_or(0, |text| text.len() as u32)
}

#[no_mangle]
pub extern "C" fn mission_fdr_string_byte(field: u32, index: u32) -> f64 {
    output_string(field)
        .and_then(|text| text.as_bytes().get(index as usize).copied())
        .map_or(f64::NAN, |byte| byte as f64)
}

/// Write the recorder's own fields into the transport array so a caller can
/// read back exactly what was captured (used by the export path and tests).
#[no_mangle]
pub extern "C" fn mission_fdr_fields_from_record() -> u32 {
    let Some(fdr) = RECORDER.lock().unwrap().clone() else {
        return 0;
    };
    let mut fields = vec![
        fdr.schema_version as f64,
        fdr.contract_id as f64,
        fdr.attempt_mode as u8 as f64,
        fdr.epoch_s as f64,
        fdr.duration_us as f64,
        fdr.initial_mass_kg,
        fdr.dry_mass_kg,
        fdr.thrust_n,
        fdr.specific_impulse_s,
    ];
    // Header continues below with the fixed 31-field layout; the engine block
    // is appended after the elo fields to keep the historical indices stable.
    let engine_tail = match &fdr.engine {
        Some(engine) => [
            engine.propellant as u8 as f64,
            engine.of_ratio,
            engine.p_c_pa,
            engine.r_throat_m,
            engine.r_exit_m,
            engine.length_fraction,
        ],
        None => [f64::NAN; 6],
    };
    fields.extend_from_slice(&fdr.initial_state);
    fields.push(f64::from(fdr.node_metric));
    fields.extend_from_slice(&fdr.prediction_baseline);
    fields.push(fdr.commands.len() as f64);
    fields.push(fdr.elo_before);
    fields.push(fdr.elo_after);
    fields.extend_from_slice(&engine_tail);
    for command in &fdr.commands {
        fields.push(command.time_us as f64);
        fields.extend_from_slice(&command.delta_v_tnw);
        match command.guidance {
            Some(target) => {
                fields.push(1.0);
                fields.extend_from_slice(&target);
            }
            None => fields.extend_from_slice(&[0.0; 7]),
        }
    }
    fields.extend_from_slice(&fdr.metrics);
    fields.extend(fdr.final_state_hash.iter().map(|byte| *byte as f64));
    fields.extend(fdr.public_key.iter().map(|byte| *byte as f64));
    fields.extend(fdr.signature.iter().map(|byte| *byte as f64));
    if fdr.schema_version >= SCHEMA_VERSION {
        fields.push(f64::from(fdr.delegation_mask));
        fields.push(fdr.thermal_specs.len() as f64);
        for spec in &fdr.thermal_specs {
            fields.extend_from_slice(&[
                f64::from(spec.stage),
                spec.tank_volumes_m3[0],
                spec.tank_volumes_m3[1],
                f64::from(spec.propellant_id),
                f64::from(spec.mli_layers),
                spec.epsilon_eff,
                f64::from(spec.tank_shell_material),
                spec.absorptivity,
                spec.outer_epsilon,
                spec.strut_conductance_w_k,
                spec.relief_pressure_pa,
                spec.radiator_area_m2,
                spec.radiator_epsilon,
                spec.cryocooler_w,
                spec.cryocooler_carnot_fraction,
                spec.array_w_at_1au,
                spec.avionics_w,
                spec.electric_propulsion_w,
                spec.structure_temperature_k,
                spec.sink_temperature_k,
            ]);
        }
        fields.extend_from_slice(&fdr.boil_off_kg);
        fields.push(fdr.cryocooler_energy_j);
        fields.push(fdr.vent_events.len() as f64);
        for event in &fdr.vent_events {
            fields.extend_from_slice(&[
                event.time_us as f64,
                f64::from(event.tank),
                event.pressure_pa,
                event.temperature_k,
            ]);
        }
    }
    if fdr.schema_version >= V4_SCHEMA_VERSION {
        // Count first, then the windows at a variable stride, then the two
        // flagged scalars — exactly what `parse_fields` reads back. Emitting
        // them in a different order still produced a plausible field count,
        // which is why the browser signer must follow this layout too.
        fields.push(fdr.flown_burn_windows.len() as f64);
        for (start, end) in &fdr.flown_burn_windows {
            fields.extend_from_slice(&[*start, *end]);
        }
        match fdr.flameout_time_s {
            Some(time) => fields.extend_from_slice(&[1.0, time]),
            None => fields.extend_from_slice(&[0.0, 0.0]),
        }
        match fdr.propellant_remaining_kg {
            Some(kg) => fields.extend_from_slice(&[1.0, kg]),
            None => fields.extend_from_slice(&[0.0, 0.0]),
        }
    }
    let count = fields.len() as u32;
    *FIELDS.lock().unwrap() = fields;
    count
}

#[no_mangle]
pub extern "C" fn mission_fdr_field_at(index: u32) -> f64 {
    FIELDS
        .lock()
        .unwrap()
        .get(index as usize)
        .copied()
        .unwrap_or(f64::NAN)
}

/// A verified ranked receipt gets a public content-addressed link, so a
/// scanner can retrieve both the record and its server certificate. Local
/// and practice dossiers retain the self-contained, offline-verifiable FDR.
/// The caller may pass only a receipt already verified and bound to `fdr`.
fn dossier_payload(fdr: &Fdr, receipt: Option<&odd_report::dossier::ReceiptAnnex>) -> String {
    let fragment = if receipt.is_some_and(|r| r.mode == AttemptMode::Ranked) {
        format!("#flight={}", odd_report::hex::encode(&fdr.digest()))
    } else {
        fdr.verify_payload()
    };
    let origin = ORIGIN.lock().unwrap().clone();
    if origin.is_empty() {
        return fragment;
    }
    format!("{origin}{fragment}")
}

/// Clear the deployment URL.
#[no_mangle]
pub extern "C" fn mission_fdr_origin_reset() {
    ORIGIN.lock().unwrap().clear();
}

/// Absorb one UTF-8 byte of the deployment page URL (no fragment). Only
/// `http://` and `https://` are accepted, so a `javascript:` or `data:` URL
/// can never be printed as a scan target. Returns 0 when rejected.
#[no_mangle]
pub extern "C" fn mission_fdr_origin_byte(byte: f64) -> i32 {
    if !byte.is_finite() || !(0.0..=255.0).contains(&byte) || byte.fract() != 0.0 {
        return 0;
    }
    let mut origin = ORIGIN.lock().unwrap();
    if origin.len() >= MAX_ORIGIN {
        return 0;
    }
    origin.push(byte as u8 as char);
    if !origin.starts_with("http://") && !origin.starts_with("https://") {
        // Still building the scheme: allow the prefix, reject anything else.
        if !"http://".starts_with(origin.as_str()) && !"https://".starts_with(origin.as_str()) {
            origin.clear();
            return 0;
        }
    }
    1
}

/// Validate the accumulated URL. Returns 1 when it is usable as a QR base.
#[no_mangle]
pub extern "C" fn mission_fdr_origin_commit() -> i32 {
    let mut origin = ORIGIN.lock().unwrap();
    let usable = (origin.starts_with("http://") || origin.starts_with("https://"))
        && origin.len() > "https://".len()
        && !origin.contains('#')
        && !origin.contains('?')
        && origin.bytes().all(|b| b.is_ascii_graphic());
    if !usable {
        origin.clear();
        return 0;
    }
    1
}

#[cfg(test)]
mod tests {
    /// A phone camera cannot resolve a bare fragment, so the printed scan
    /// target must be the absolute deployment URL when the page supplies it.
    #[test]
    fn the_scan_target_is_absolute_when_the_page_states_its_origin() {
        let _guard = bridge::test_lock();
        let fdr = flown_recorder();
        mission_fdr_origin_reset();
        let url = "https://flights.example.org/mission.html";
        for byte in url.bytes() {
            assert_eq!(mission_fdr_origin_byte(f64::from(byte)), 1);
        }
        assert_eq!(mission_fdr_origin_commit(), 1);
        let payload = dossier_payload(&fdr, None);
        assert!(payload.starts_with(url));
        assert!(payload.ends_with(&fdr.verify_payload()));
        assert_eq!(payload, format!("{url}{}", fdr.verify_payload()));
        mission_fdr_origin_reset();
        assert_eq!(dossier_payload(&fdr, None), fdr.verify_payload());
    }

    /// A hostile or malformed origin must never reach the printed page.
    #[test]
    fn a_non_http_origin_is_refused() {
        let _guard = bridge::test_lock();
        for bad in [
            "javascript:alert(1)",
            "data:text/html,x",
            "file:///etc/passwd",
            "https://x.test/p#frag",
            "https://x.test/p?q=1",
            "https://",
        ] {
            mission_fdr_origin_reset();
            for byte in bad.bytes() {
                if mission_fdr_origin_byte(f64::from(byte)) != 1 {
                    break;
                }
            }
            assert_eq!(mission_fdr_origin_commit(), 0, "{bad} must be refused");
            assert!(ORIGIN.lock().unwrap().is_empty());
        }
        // Non-finite and out-of-range bytes are refused too.
        mission_fdr_origin_reset();
        assert_eq!(mission_fdr_origin_byte(f64::NAN), 0);
        assert_eq!(mission_fdr_origin_byte(300.0), 0);
        assert_eq!(mission_fdr_origin_byte(104.5), 0);
    }

    use super::*;

    fn roundtrip_fields(fdr: &Fdr) -> Fdr {
        *RECORDER.lock().unwrap() = Some(fdr.clone());
        let count = mission_fdr_fields_from_record();
        assert!(count > 0);
        let fields = FIELDS.lock().unwrap().clone();
        parse_fields(&fields, &fdr.callsign).expect("layout must round trip")
    }

    /// The v4 block's *order* matters, not just its width: the writer and the
    /// reader disagreed once and the round-trip test passed anyway because the
    /// fixture had no windows and two empty flags.
    #[test]
    fn the_v4_block_round_trips_a_non_empty_flight() {
        let license = odd_identity::License::from_entropy([5u8; 16]);
        let mut fdr = transport_fixture(&license);
        // 101's window is 10,800 s, so a window past it must be rejected —
        // and one inside it must survive the round trip.
        fdr.flown_burn_windows = vec![(61.0, 160.0), (2_995.0, 2_995.82)];
        fdr.flameout_time_s = Some(2_995.82);
        fdr.propellant_remaining_kg = Some(120.0);
        fdr.sign(&license).expect("the fixture signs");
        let back = roundtrip_fields(&fdr);
        assert_eq!(back.flown_burn_windows, fdr.flown_burn_windows);
        assert_eq!(back.flameout_time_s, fdr.flameout_time_s);
        assert_eq!(back.propellant_remaining_kg, fdr.propellant_remaining_kg);
    }

    fn transport_fixture(license: &odd_identity::License) -> Fdr {
        let mut fdr = Fdr {
            schema_version: SCHEMA_VERSION,
            thermal_specs: Vec::new(),
            delegation_mask: 0,
            vent_events: Vec::new(),
            boil_off_kg: [0.0; 2],
            cryocooler_energy_j: 0.0,
            contract_id: 101,
            attempt_mode: AttemptMode::Ranked,
            callsign: "BRIDGE TEST".into(),
            epoch_s: 820_497_600,
            duration_us: 10_800_000_000,
            initial_mass_kg: 1200.0,
            dry_mass_kg: 1020.0,
            thrust_n: 2400.0,
            specific_impulse_s: 310.0,
            engine: Some(odd_contracts::STORABLE_BASELINE),
            initial_state: [6_638_137.0, 0.0, 0.0, 0.0, 7_748.0, 0.0],
            node_metric: false,
            prediction_baseline: [180.0, 550_000.0, 260_000.0, 120.0, 10_800.0, f64::NAN],
            commands: vec![
                Command {
                    time_us: 200_000_000,
                    delta_v_tnw: [81.5, 0.0, 0.0],
                    guidance: None,
                },
                Command {
                    time_us: 2_995_000_000,
                    delta_v_tnw: [98.0, 0.0, 0.0],
                    guidance: Some([6_928_137.0, 0.0, 0.0, 0.0, 7_500.0, 0.0]),
                },
            ],
            metrics: [f64::NAN; REPORT_LEN],
            final_state_hash: [1u8; 32],
            elo_before: 1200.0,
            elo_after: 1201.0,
            flown_burn_windows: vec![],
            flameout_time_s: None,
            propellant_remaining_kg: None,
            public_key: [0; 32],
            signature: [0; 64],
        };
        fdr.metrics[0] = 1080.0;
        fdr.sign(license).unwrap();
        fdr
    }

    #[test]
    fn transport_layout_round_trips_every_field() {
        let license = odd_identity::License::from_entropy([3u8; 16]);
        let fdr = transport_fixture(&license);
        let parsed = roundtrip_fields(&fdr);
        assert_eq!(parsed.canonical_bytes(), fdr.canonical_bytes());
        assert_eq!(parsed.public_key, fdr.public_key);
        assert_eq!(parsed.signature, fdr.signature);
        assert_eq!(parsed.verify_signature(), Ok(()));
        // A NaN metric survives the numeric transport as NaN, not as zero.
        assert!(parsed.metrics[3].is_nan());
        assert!(parsed.prediction_baseline[5].is_nan());
    }

    /// Run contract 101 end to end and capture the executed flight.
    /// The caller must hold `bridge::test_lock()` for its whole body: both the
    /// mission statics and this module's recorder statics are process-wide.
    fn flown_recorder() -> Fdr {
        let mu: f64 = 3.986_004_418e14;
        let radius: f64 = 6_378_137.0 + 260_000.0;
        assert_eq!(bridge::mission_contract(101), 1);
        let a: f64 = (radius + 6_378_137.0 + 550_000.0) / 2.0;
        let coast = std::f64::consts::PI * (a * a * a / mu).sqrt();
        let v1 = (mu / radius).sqrt();
        let v2 = (mu / (6_378_137.0 + 550_000.0)).sqrt();
        assert_eq!(
            bridge::mission_add_node(200.0, v1 * ((a / radius).sqrt() - 1.0), 0.0, 0.0),
            1
        );
        assert_eq!(
            bridge::mission_add_node(
                200.0 + coast,
                v2 * (1.0 - ((6_378_137.0 + 260_000.0) / a).sqrt()),
                0.0,
                0.0
            ),
            1
        );
        assert_eq!(bridge::mission_compile(), 1);
        assert_eq!(
            bridge::mission_lock(180.0, 550_000.0, 260_000.0, 120.0, 10_800.0, f64::NAN),
            1
        );
        assert_eq!(
            mission_commit_prepare(),
            1,
            "preflight commitment must be available before flight"
        );
        let hashes = *COMMIT_HASHES.lock().unwrap();
        assert_eq!(bridge::mission_execute(), 1);
        assert_eq!(
            mission_commit_prepare(),
            0,
            "preflight commitment must not be created after execution"
        );
        identity_bridge::set_license_for_test(odd_identity::License::from_entropy([21u8; 16]));
        identity_bridge::mission_input_reset();
        for byte in b"GATE PILOT" {
            assert_eq!(identity_bridge::mission_input_absorb(*byte as f64), 1);
        }
        assert_eq!(mission_fdr_capture(0, 1200.0, 1204.0), 1);
        assert_eq!(mission_fdr_sign(), 1);
        let fdr = RECORDER.lock().unwrap().clone().unwrap();
        assert_eq!(&hashes[..32], &odd_worker::commitment::design_hash(&fdr));
        assert_eq!(
            &hashes[32..],
            &odd_worker::commitment::prediction_hash(&fdr)
        );
        fdr
    }

    #[test]
    fn oracle_tamper_proof_fdr_verification() {
        let _guard = bridge::test_lock();
        let fdr = flown_recorder();
        assert_eq!(fdr.verify_signature(), Ok(()));
        // The whole point: a real flight reproduces bit for bit.
        assert_eq!(verify_and_replay(&fdr), Ok(()));
        let replayed = REPLAYED.lock().unwrap().unwrap();
        for (index, (recorded, actual)) in fdr.metrics.iter().zip(replayed.iter()).enumerate() {
            assert!(
                odd_report::fdr::same_metric(*recorded, *actual),
                "metric {index} must replay exactly"
            );
        }
        assert!(GEOMETRY.lock().unwrap().as_ref().unwrap().level3.len() > 100);

        // One ULP of a metric: the signature catches it before physics runs.
        let mut ulp = fdr.clone();
        ulp.metrics[1] = f64::from_bits(fdr.metrics[1].to_bits() + 1);
        assert_eq!(verify_and_replay(&ulp), Err(FdrError::SignatureMismatch));

        // One microsecond of command timing: same.
        let mut micro = fdr.clone();
        micro.commands[0].time_us += 1;
        assert_eq!(verify_and_replay(&micro), Err(FdrError::SignatureMismatch));

        // A forger who re-signs a false metric passes the signature check and
        // must then be caught by the replay. This is what makes the physical
        // half of the gate meaningful rather than decorative.
        let license = odd_identity::License::from_entropy([21u8; 16]);
        let mut forged = fdr.clone();
        forged.metrics[1] = fdr.metrics[1] + 1.0;
        forged.sign(&license).unwrap();
        assert_eq!(forged.verify_signature(), Ok(()));
        assert_eq!(
            verify_and_replay(&forged),
            Err(FdrError::PhysicsReplayMismatch {
                field: "metrics",
                index: 1
            })
        );

        // A re-signed command at a different epoch changes the flight itself.
        let mut moved = fdr.clone();
        moved.commands[0].time_us += 1;
        moved.sign(&license).unwrap();
        assert_eq!(moved.verify_signature(), Ok(()));
        assert!(
            matches!(
                verify_and_replay(&moved),
                Err(FdrError::PhysicsReplayMismatch { .. })
            ),
            "a re-signed one-microsecond shift must fail the replay"
        );

        // A re-signed terminal hash cannot be guessed either.
        let mut hashed = fdr.clone();
        hashed.final_state_hash[0] ^= 1;
        hashed.sign(&license).unwrap();
        assert_eq!(
            verify_and_replay(&hashed),
            Err(FdrError::FinalStateMismatch)
        );
    }

    #[test]
    fn a_captured_flight_exports_a_deterministic_dossier() {
        let _guard = bridge::test_lock();
        let fdr = flown_recorder();
        *RECORDER.lock().unwrap() = Some(fdr.clone());
        assert_eq!(mission_fdr_dossier(), 1);
        let first = DOSSIER.lock().unwrap().clone();
        assert_eq!(mission_fdr_dossier(), 1);
        assert_eq!(first, DOSSIER.lock().unwrap().clone());
        assert!(first.contains("GATE PILOT"));
        assert!(first.contains("id=\"verification-qr\""));
        assert!(first.contains("id=\"coalition-table\""));
        // The JSON transport must carry the same flight back in.
        let json = JSON.lock().unwrap().clone();
        assert!(json.contains("\"attempt_mode\":\"Ranked\""));
        assert!(json.contains("\"callsign\":\"GATE PILOT\""));
        // And the verify payload must round trip through the parser.
        identity_bridge::mission_input_reset();
        for byte in fdr.verify_payload().as_bytes() {
            identity_bridge::mission_input_absorb(*byte as f64);
        }
        assert_eq!(mission_fdr_from_payload(), 1);
        assert_eq!(mission_fdr_verify(), 1);
        assert_eq!(
            mission_fdr_string_len(2),
            odd_report::VERIFIED_BANNER.len() as u32
        );
        assert_eq!(mission_fdr_dossier_from_record(), 1);
        assert!(DOSSIER.lock().unwrap().contains("id=\"trajectory-figure\""));
        // The schedule's per-burn swept angles must be the exact parts of the
        // reported total, not a second estimate.
        let geometry = GEOMETRY.lock().unwrap().clone().unwrap();
        assert_eq!(
            geometry.burns.len(),
            2,
            "the rescue example commands two burns"
        );
        let parts: f64 = geometry.burns.iter().map(|burn| burn.sweep_rad).sum();
        assert!(geometry
            .burns
            .iter()
            .all(|burn| burn.sweep_rad.is_finite() && burn.sweep_rad > 0.0));
        assert!(
            (parts - fdr.metrics[57]).abs() < 1e-12,
            "per-burn sweeps {parts} must sum to report[57] {}",
            fdr.metrics[57]
        );
    }

    #[test]
    fn flown_dossier_accepts_only_a_valid_bound_server_receipt() {
        let _guard = bridge::test_lock();
        let fdr = flown_recorder();
        *RECORDER.lock().unwrap() = Some(fdr.clone());
        assert_eq!(mission_fdr_verify(), 1);
        let key = odd_worker::ServerKey::from_seed([0x5e; 32]);
        let receipt = odd_worker::ServerReceipt::issue(
            odd_worker::ReceiptPayload::new(
                fdr.digest(),
                fdr.public_key,
                fdr.contract_id,
                1,
                AttemptMode::Ranked,
                fdr.metrics[4],
                1200.0,
                1205.0,
                799.0,
                1_700_000_000,
            ),
            &key,
        );
        let load = |r: &odd_worker::ServerReceipt| {
            identity_bridge::mission_input_reset();
            for byte in key.public_key().iter().chain(r.to_bytes().iter()) {
                assert_eq!(identity_bridge::mission_input_absorb(f64::from(*byte)), 1);
            }
            mission_fdr_receipt_load()
        };
        assert_eq!(load(&receipt), 1);
        assert_eq!(mission_fdr_dossier_from_record(), 1);
        let html = DOSSIER.lock().unwrap().clone();
        assert!(html.contains("id=\"annex-d\""));
        assert!(html.contains("CERTIFIED SERVER-RANKED RECEIPT"));
        assert!(html.contains(&odd_report::hex::encode(&receipt.to_bytes())));
        let target = format!("#flight={}", odd_report::hex::encode(&fdr.digest()));
        assert!(
            html.contains(&target),
            "ranked QR must resolve to the receipt"
        );
        let mut forged = receipt.clone();
        forged.payload.elo_after_bits = 2999.0f64.to_bits();
        assert_eq!(load(&forged), 0);
        assert_eq!(mission_fdr_dossier_from_record(), 1);
        let unverified = DOSSIER.lock().unwrap().clone();
        assert!(unverified.contains("UNVERIFIED LOCAL / PRACTICE"));
        assert!(unverified.contains("#verify="));
        assert!(!unverified.contains("#flight="));
        // Even a valid signature cannot certify a different record's digest.
        let mut other = receipt.payload.clone();
        other.fdr_hash[0] ^= 1;
        assert_eq!(load(&odd_worker::ServerReceipt::issue(other, &key)), 0);
    }

    #[test]
    fn a_wrong_length_or_bad_byte_is_refused() {
        assert!(parse_fields(&[], "").is_none());
        assert!(parse_fields(&[0.0; HEADER], "").is_none());
        assert!(parse_fields(&[0.5; HEADER + REPORT_LEN + 128], "").is_none());
        mission_fdr_reset();
        for _ in 0..MAX_FIELDS {
            assert_eq!(mission_fdr_field(1.0), 1);
        }
        assert_eq!(
            mission_fdr_field(1.0),
            0,
            "the transport array must be bounded"
        );
        assert_eq!(mission_fdr_field_count(), MAX_FIELDS as u32);
        mission_fdr_reset();
        assert_eq!(mission_fdr_field_count(), 0);
    }
}
