// Signed flight recorder transport.
//
// The layout below mirrors odd-view/src/fdr_bridge.rs exactly. Numbers cross
// the boundary as a flat f64 array because the module shares no memory with
// the page; null in JSON means NaN (an undefined metric), never zero.
//
// The secret never enters a worker: the flight worker captures the unsigned
// fields, this module signs them on the main thread with the active license,
// and a one-shot worker does the expensive verification replay.
export const HEADER = 31;
export const COMMAND_STRIDE = 11;
export const METRIC_COUNT = 58;
const MODE = 2;
const ELO_BEFORE = 23;
const ELO_AFTER = 24;

const nullToNaN = value => (value === null || value === undefined ? NaN : Number(value));

function hexBytes(hex, count) {
  if (typeof hex !== 'string' || hex.length !== count * 2 || !/^[0-9a-f]+$/i.test(hex)) {
    throw Error(`expected ${count} bytes of hex`);
  }
  return Array.from({ length: count }, (_, i) => parseInt(hex.slice(i * 2, i * 2 + 2), 16));
}

/// Build the transport array from a parsed recorder JSON document.
export function fieldsFromJson(document) {
  const schema = document?.schema_version;
  if (schema !== 1 && schema !== 2 && schema !== 3 && schema !== 4) throw Error('unsupported recorder schema');
  const mode = document.attempt_mode === 'Ranked' ? 0 : document.attempt_mode === 'Practice' ? 1 : null;
  if (mode === null) throw Error('unknown attempt mode');
  const commands = document.command_timeline;
  const metrics = document.metrics;
  if (!Array.isArray(commands) || !Array.isArray(metrics) || metrics.length !== METRIC_COUNT) {
    throw Error('recorder is missing its command timeline or metrics');
  }
  if (document.initial_state?.length !== 6 || document.prediction_baseline?.length !== 6) {
    throw Error('recorder is missing its initial state or prediction');
  }
  const fields = [
    document.schema_version,
    document.contract_id,
    mode,
    document.epoch_s,
    document.duration_us,
    document.initial_mass_kg,
    document.dry_mass_kg,
    document.thrust_n,
    document.specific_impulse_s,
    ...document.initial_state.map(Number),
    document.node_metric ? 1 : 0,
    ...document.prediction_baseline.map(nullToNaN),
    commands.length,
    document.elo_before,
    document.elo_after,
    // Schema 2 signs the engine design. Legacy records carry NaN here, which
    // the bridge reads back as "no design".
    ...(schema >= 2
      ? [
          Number(document.engine_propellant_id),
          Number(document.engine_of_ratio),
          Number(document.engine_p_c_pa),
          Number(document.engine_r_throat_m),
          Number(document.engine_r_exit_m),
          Number(document.engine_length_fraction),
        ]
      : [NaN, NaN, NaN, NaN, NaN, NaN]),
  ];
  if (schema >= 2 && !Number.isInteger(document.engine_propellant_id)) {
    throw Error('recorder is missing its engine design');
  }
  for (const command of commands) {
    if (!Number.isInteger(command.time_us)) throw Error('command time must be integer microseconds');
    if (command.delta_v_tnw?.length !== 3) throw Error('command needs three delta-v components');
    const guided = Array.isArray(command.guidance);
    if (guided && command.guidance.length !== 6) throw Error('guidance needs six components');
    fields.push(
      command.time_us,
      ...command.delta_v_tnw.map(Number),
      guided ? 1 : 0,
      ...(guided ? command.guidance.map(Number) : [NaN, NaN, NaN, NaN, NaN, NaN]),
    );
  }
  fields.push(...metrics.map(nullToNaN));
  fields.push(...hexBytes(document.final_state_hash, 32));
  fields.push(...hexBytes(document.public_key, 32));
  fields.push(...hexBytes(document.ed25519_signature, 64));
  if (schema >= 3) {
    // FDR v3 trailer. The order and width mirror `mission_fdr_fields_from_record`
    // in the Rust bridge exactly; the signed bytes come from Rust, never from
    // this rendering, so a mismatch here fails the signature rather than
    // silently changing what the document means.
    const specs = document.thermal_specs;
    const vents = document.vent_events;
    const loss = document.boil_off_kg;
    if (!Array.isArray(specs) || !Array.isArray(vents) || !Array.isArray(loss) || loss.length !== 2) {
      throw Error('recorder is missing its thermal block');
    }
    if (!Number.isInteger(document.delegation_mask) || (document.delegation_mask & ~0b1111) !== 0) {
      throw Error('recorder has an invalid delegation mask');
    }
    fields.push(document.delegation_mask, specs.length);
    for (const spec of specs) {
      if (spec.tank_volumes_m3?.length !== 2) throw Error('a stage needs two tank volumes');
      fields.push(
        spec.stage,
        ...spec.tank_volumes_m3.map(Number),
        spec.propellant_id,
        spec.mli_layers,
        spec.epsilon_eff,
        spec.tank_shell_material,
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
      );
    }
    fields.push(...loss.map(Number), document.cryocooler_energy_j, vents.length);
    for (const vent of vents) {
      if (!Number.isInteger(vent.time_us)) throw Error('vent time must be integer microseconds');
      fields.push(vent.time_us, vent.tank, vent.pressure_pa, vent.temperature_k);
    }
  }
  if (schema >= 4) {
    // FDR v4 trailer: the windows actually flown, the flameout instant and the
    // propellant still aboard. Same one-field-per-entry layout the Rust bridge
    // writes, so the stride does not depend on how many burns were flown.
    const windows = document.flown_burn_windows ?? [];
    if (!Array.isArray(windows)) throw Error('flown burn windows must be an array');
    fields.push(windows.length);
    for (const window of windows) {
      if (!Array.isArray(window) || window.length !== 2) throw Error('a burn window is a start/end pair');
      fields.push(Number(window[0]), Number(window[1]));
    }
    // Count, then windows, then the two flagged scalars: the order
    // `mission_fdr_fields_from_record` and `parse_fields` agree on.
    const flameout = document.flameout_time_s;
    if (flameout === null || flameout === undefined) fields.push(0, NaN);
    else fields.push(1, Number(flameout));
    const remaining = document.propellant_remaining_kg;
    if (remaining === null || remaining === undefined) fields.push(0, NaN);
    else fields.push(1, Number(remaining));
  }
  return { fields, callsign: String(document.callsign ?? '') };
}

/// Stamp the attempt mode and rating change onto captured fields. The flight
/// worker cannot know them: Elo is settled after the debrief is assembled.
export function stampOutcome(fields, { ranked, before, after }) {
  const stamped = [...fields];
  stamped[MODE] = ranked ? 0 : 1;
  stamped[ELO_BEFORE] = before;
  stamped[ELO_AFTER] = after;
  return stamped;
}

function absorbText(wasm, text) {
  wasm.mission_input_reset();
  for (const byte of new TextEncoder().encode(text)) {
    if (!wasm.mission_input_absorb(byte)) throw Error('recorder text rejected');
  }
}

function readString(wasm, field) {
  const length = wasm.mission_fdr_string_len(field);
  const bytes = Uint8Array.from({ length }, (_, i) => wasm.mission_fdr_string_byte(field, i));
  return new TextDecoder().decode(bytes);
}

export function loadFields(wasm, fields, callsign) {
  wasm.mission_fdr_reset();
  for (const value of fields) {
    if (!wasm.mission_fdr_field(value)) throw Error('recorder field rejected');
  }
  absorbText(wasm, callsign);
  if (wasm.mission_fdr_parse() !== 1) throw Error('recorder layout is not valid');
}

/// Sign captured fields with the active license and return the JSON document.
export function sign(wasm, fields, callsign) {
  loadFields(wasm, fields, callsign);
  if (wasm.mission_fdr_sign() !== 1) throw Error('could not sign the flight recorder');
  const json = readString(wasm, 0);
  return { json, document: JSON.parse(json), payload: readString(wasm, 3) };
}

/// JSON.stringify normally changes -0 to +0, but FDR and receipt signatures
/// cover the IEEE-754 sign bit. Preserve negative zero in every nested field;
/// non-finite numbers retain JSON.stringify's normal null representation.
/// Choose a marker absent from the original JSON so even untrusted strings or
/// object keys cannot be mistaken for a numeric value during replacement.
export function stringifySigned(value) {
  const original = JSON.stringify(value);
  if (original === undefined) return original;
  let marker = '__odderon_negative_zero__';
  while (original.includes(JSON.stringify(marker))) marker += '_';
  const encoded = JSON.stringify(value, (_key, item) => Object.is(item, -0) ? marker : item, 2);
  return encoded.split(JSON.stringify(marker)).join('-0');
}

/// The receipt is a separate server-signed envelope; FDR v3 canonical bytes
/// remain unchanged. It travels with the downloadable recorder.
export function withReceipt(record, receipt) {
  const document = { ...record.document, server_receipt: receipt };
  return { ...record, document, json: stringifySigned(document) };
}

/// Turn either a JSON document or a #verify= payload into worker inputs.
export function prepareVerification(wasm, text) {
  const trimmed = String(text).trim();
  if (!trimmed) throw Error('paste a recorder file or a #verify= payload');
  if (trimmed.startsWith('{')) {
    const document = JSON.parse(trimmed);
    const { fields, callsign } = fieldsFromJson(document);
    return { fields, callsign, receipt: document.server_receipt ?? null };
  }
  absorbText(wasm, trimmed.includes('#verify=') ? trimmed.slice(trimmed.indexOf('#verify=')) : trimmed);
  if (wasm.mission_fdr_from_payload() !== 1) throw Error('that verification payload could not be decoded');
  const document = JSON.parse(readString(wasm, 0));
  return fieldsFromJson(document);
}

export function download(name, text, type) {
  const blob = new Blob([text], { type });
  const url = URL.createObjectURL(blob);
  const link = document.createElement('a');
  link.href = url;
  link.download = name;
  link.click();
  URL.revokeObjectURL(url);
}
