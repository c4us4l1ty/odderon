// Signed flight recorder transport.
//
// The layout below mirrors odd-view/src/fdr_bridge.rs exactly. Numbers cross
// the boundary as a flat f64 array because the module shares no memory with
// the page; null in JSON means NaN (an undefined metric), never zero.
//
// The secret never enters a worker: the flight worker captures the unsigned
// fields, this module signs them on the main thread with the active license,
// and a one-shot worker does the expensive verification replay.
export const HEADER = 25;
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
  if (document?.schema_version !== 1) throw Error('unsupported recorder schema');
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
  ];
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

/// Turn either a JSON document or a #verify= payload into worker inputs.
export function prepareVerification(wasm, text) {
  const trimmed = String(text).trim();
  if (!trimmed) throw Error('paste a recorder file or a #verify= payload');
  if (trimmed.startsWith('{')) {
    const { fields, callsign } = fieldsFromJson(JSON.parse(trimmed));
    return { fields, callsign };
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
