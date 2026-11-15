// Persistent player identity and ranked Astro-Elo.
//
// The Flight License secret lives in localStorage so a reload never resets
// identity or rating. That is a deliberate trade: localStorage is readable by
// any script on this origin, so this is a local practice identity, not key
// custody. The profile dialog says so, and the export file carries the same
// warning. Nothing here schedules animation frames or workers.
const KEY = 'odderon.flightLicense.v1';
const SCHEMA = 1;
const START_ELO = 1200;
const MISSION_K = 8;
/// Identities kept in the local archive. Switching must not destroy progress,
/// but the archive cannot grow without bound in a fixed storage quota.
const MAX_PROFILES = 8;

let wasm = null;
let state = null;
const listeners = new Set();

export const eloK = ranked => (ranked < 20 ? 40 : 20);
export const missionK = MISSION_K;

function defaults() {
  return { schema: SCHEMA, entropyHex: null, callsign: '', elo: START_ELO, missionElo: {}, ranked: {}, flights: [], profiles: {} };
}

const finiteOr = (value, fallback) =>
  (typeof value === 'number' && Number.isFinite(value) ? value : fallback);

/// Read and repair the stored record field by field.
///
/// Spreading a parsed object over the defaults is not enough: a truncated
/// write, a hand-edited record or a future migration can leave `elo: null` or
/// `flights: null`, and the first `.toFixed()` or `.length` then throws and
/// takes the whole page down. Every field is validated here so a damaged
/// record degrades to defaults instead of bricking the app. The secret is the
/// one field that cannot be repaired; if it is unusable the record is ignored
/// and a fresh license is minted.
function read() {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw);
    if (parsed?.schema !== SCHEMA || typeof parsed.entropyHex !== 'string') return null;
    if (!/^[0-9a-f]{32}$/i.test(parsed.entropyHex)) return null;
    const profiles = {};
    for (const [hex, archived] of Object.entries(parsed.profiles ?? {})) {
      if (/^[0-9a-f]{32}$/.test(hex) && archived && typeof archived === 'object') {
        profiles[hex] = repairProfile(archived);
      }
    }
    return {
      schema: SCHEMA,
      entropyHex: parsed.entropyHex.toLowerCase(),
      ...repairProfile(parsed),
      profiles: trimProfiles(profiles),
    };
  } catch {
    return null;
  }
}

/// Validate one profile's progress fields. Used for both the active record and
/// every archived identity, so the two cannot drift apart.
function repairProfile(source) {
  const missionElo = {};
  for (const [id, rating] of Object.entries(source.missionElo ?? {})) {
    if (typeof rating === 'number' && Number.isFinite(rating)) missionElo[id] = rating;
  }
  const ranked = {};
  for (const [id, record] of Object.entries(source.ranked ?? {})) {
    if (!record || typeof record !== 'object') continue;
    if (!Number.isFinite(record.score) || !Number.isFinite(record.elo)) continue;
    ranked[id] = {
      score: record.score,
      elo: record.elo,
      delta: finiteOr(record.delta, 0),
      at: finiteOr(record.at, 0),
      ...(typeof record.recorder === 'string' ? { recorder: record.recorder } : {}),
    };
  }
  return {
    callsign: typeof source.callsign === 'string' && /^[\w \-]{1,24}$/.test(source.callsign) ? source.callsign : '',
    elo: finiteOr(source.elo, START_ELO),
    missionElo,
    ranked,
    flights: Array.isArray(source.flights)
      ? source.flights.filter(flight => flight && typeof flight === 'object').slice(-200)
      : [],
  };
}

/// Keep the most recently written identities. Insertion order is the recency
/// order because a switch re-inserts the profile it is leaving.
function trimProfiles(profiles) {
  const keys = Object.keys(profiles);
  for (const key of keys.slice(0, Math.max(0, keys.length - MAX_PROFILES))) delete profiles[key];
  return profiles;
}

/// The active identity's progress, without the secret or the archive.
function progress() {
  return {
    callsign: state.callsign,
    elo: state.elo,
    missionElo: { ...state.missionElo },
    ranked: { ...state.ranked },
    flights: [...state.flights],
  };
}

function persist() {
  try {
    localStorage.setItem(KEY, JSON.stringify(state));
    return true;
  } catch {
    // Private browsing or a full quota: keep the session usable in memory.
    return false;
  }
}

function notify() {
  for (const listener of listeners) listener(profile());
}

function absorb(text) {
  wasm.mission_input_reset();
  for (const byte of new TextEncoder().encode(text)) {
    if (!wasm.mission_input_absorb(byte)) throw Error('identity input rejected');
  }
}

function readString(field) {
  const length = wasm.mission_identity_string_len(field);
  const bytes = Uint8Array.from({ length }, (_, i) => wasm.mission_identity_string_byte(field, i));
  return new TextDecoder().decode(bytes);
}

function loadIntoWasm(entropyHex) {
  const words = [0, 8, 16, 24].map(i => parseInt(entropyHex.slice(i, i + 8), 16));
  return wasm.mission_identity_from_entropy(...words) === 1;
}

function identityFields() {
  return {
    publicKey: Array.from({ length: 32 }, (_, i) => wasm.mission_identity_pubkey(i)),
    publicId: readString(0),
    mnemonic: readString(1),
    flightKey: readString(2),
  };
}

/// Attach the loaded wasm module and restore or create the license.
export function init(module) {
  wasm = module;
  state = read();
  if (state && loadIntoWasm(state.entropyHex)) {
    if (!state.callsign) state.callsign = identityFields().publicId.slice(7, 13);
    notify();
    return profile();
  }
  // First visit (or an unreadable record): mint a fresh license from the
  // platform CSPRNG. Rust never generates randomness itself.
  const entropy = crypto.getRandomValues(new Uint8Array(16));
  const hex = Array.from(entropy, b => b.toString(16).padStart(2, '0')).join('');
  if (!loadIntoWasm(hex)) throw Error('could not create a flight license');
  state = { ...defaults(), entropyHex: hex, callsign: identityFields().publicId.slice(7, 13) };
  persist();
  notify();
  return profile();
}

export function profile() {
  if (!state || !wasm) return null;
  const identity = identityFields();
  return {
    ...identity,
    callsign: state.callsign,
    elo: state.elo,
    missionElo: { ...state.missionElo },
    ranked: { ...state.ranked },
    flights: state.flights.length,
    entropyHex: state.entropyHex,
  };
}

export function onChange(listener) {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function setCallsign(name) {
  const trimmed = String(name).trim();
  if (!trimmed || trimmed.length > 24 || !/^[\w \-]+$/.test(trimmed)) {
    return { ok: false, error: 'Use 1–24 letters, digits, spaces, dash or underscore.' };
  }
  state.callsign = trimmed;
  persist();
  notify();
  return { ok: true };
}

/// Validate without switching, for live feedback in the restore field.
export function checkRestore(text) {
  const trimmed = String(text).trim();
  if (!trimmed) return { ok: false, kind: null, error: '' };
  absorb(trimmed);
  if (/^odd-/i.test(trimmed)) {
    return wasm.mission_identity_key_valid() === 1
      ? { ok: true, kind: 'key', error: '' }
      : { ok: false, kind: 'key', error: 'Expected ODD- plus eight 4-character hex groups.' };
  }
  const words = trimmed.split(/\s+/).length;
  if (wasm.mission_identity_mnemonic_valid() === 1) return { ok: true, kind: 'mnemonic', error: '' };
  return {
    ok: false,
    kind: 'mnemonic',
    error: words < 12 ? `${words} of 12 words entered.` : 'Those twelve words fail the license checksum.',
  };
}

/// Switch to a license supplied as words, a flight key, or a backup file.
export function restore(text) {
  const trimmed = String(text).trim();
  absorb(trimmed);
  const isKey = /^odd-/i.test(trimmed);
  const loaded = isKey ? wasm.mission_identity_from_key() : wasm.mission_identity_from_mnemonic();
  if (loaded !== 1) {
    // Put the previous identity back so a failed restore is not destructive.
    loadIntoWasm(state.entropyHex);
    return { ok: false, error: isKey ? 'That flight key is not valid.' : 'Those words fail the license checksum.' };
  }
  const identity = identityFields();
  const hex = entropyHexFromKey(identity.flightKey);
  // Archive the identity being left, then resume the target's own progress.
  // Overwriting a single record would destroy the other pilot's rating and,
  // worse, erase its certificates — which would let the same contract be
  // certified twice by switching away and back.
  const profiles = { ...state.profiles };
  delete profiles[state.entropyHex];
  profiles[state.entropyHex] = progress();
  const resumed = profiles[hex];
  state = {
    ...defaults(),
    ...(resumed ?? {}),
    entropyHex: hex,
    callsign: resumed?.callsign || identity.publicId.slice(7, 13),
    profiles: trimProfiles(profiles),
  };
  persist();
  notify();
  return { ok: true, publicId: identity.publicId, resumed: Boolean(resumed) };
}

function entropyHexFromKey(flightKey) {
  return flightKey.slice(4).split('-').join('').toLowerCase();
}

export function licenseBackup() {
  const identity = identityFields();
  return {
    schema: SCHEMA,
    kind: 'odderon-flight-license',
    warning: 'Contains the secret entropy. Anyone with this file controls this identity. localStorage is readable by any script on this origin.',
    callsign: state.callsign,
    publicId: identity.publicId,
    publicKeyHex: identity.publicKey.map(b => b.toString(16).padStart(2, '0')).join(''),
    flightKey: identity.flightKey,
    mnemonic: identity.mnemonic,
    elo: state.elo,
    missionElo: state.missionElo,
    ranked: state.ranked,
  };
}

export function importBackup(text) {
  let parsed;
  try {
    parsed = JSON.parse(text);
  } catch {
    return { ok: false, error: 'That file is not valid JSON.' };
  }
  if (parsed?.kind !== 'odderon-flight-license' || parsed.schema !== SCHEMA) {
    return { ok: false, error: 'Not an Odderon flight license backup.' };
  }
  const secret = parsed.flightKey || parsed.mnemonic;
  if (!secret) return { ok: false, error: 'The backup carries no flight key or mnemonic.' };
  const result = restore(secret);
  if (!result.ok) return result;
  if (typeof parsed.callsign === 'string' && parsed.callsign.trim()) setCallsign(parsed.callsign);
  if (Number.isFinite(parsed.elo)) {
    state.elo = parsed.elo;
    if (parsed.missionElo && typeof parsed.missionElo === 'object') state.missionElo = { ...parsed.missionElo };
    if (parsed.ranked && typeof parsed.ranked === 'object') state.ranked = { ...parsed.ranked };
    persist();
    notify();
  }
  return result;
}

/// Attach the signed recorder to a ranked certificate.
///
/// The certificate is issued once, so it has to stay retrievable: without the
/// stored document a reload would leave the rating locked but the official
/// record unrecoverable except by flying again, which would be practice.
/// Only ranked certificates are kept, and the size is bounded so a hostile or
/// malformed document cannot exhaust the storage quota.
export function attachCertificate(contractId, recorderJson) {
  const record = state?.ranked?.[String(contractId)];
  if (!record || typeof recorderJson !== 'string' || recorderJson.length > 65536) return false;
  let document;
  try {
    document = JSON.parse(recorderJson);
  } catch {
    return false;
  }
  if (document?.attempt_mode !== 'Ranked' || String(document.contract_id) !== String(contractId)) return false;
  if (document.public_key !== hexOf(profile().publicKey)) return false;
  record.recorder = recorderJson;
  persist();
  notify();
  return true;
}

function hexOf(bytes) {
  return bytes.map(b => b.toString(16).padStart(2, '0')).join('');
}

/// The stored signed certificate for a contract, or null.
export function certificate(contractId) {
  return state?.ranked?.[String(contractId)]?.recorder ?? null;
}

/// Whether this contract already has a certified ranked certificate.
export function isRanked(contractId) {
  if (!state) return false;
  return Object.prototype.hasOwnProperty.call(state.ranked, String(contractId));
}

/// Record a completed flight. Only the first ranked execution of a contract
/// moves the persistent rating; later runs are practice and cannot farm Elo.
export function recordFlight({ contractId, score, mode, missionRating }) {
  if (!state) throw Error('flight license is not initialised');
  const id = String(contractId);
  const already = isRanked(id);
  const ranked = mode === 'ranked' && !already;
  const rating = Number.isFinite(missionRating) ? missionRating : (state.missionElo[id] ?? 1200);
  const before = state.elo;
  const k = eloK(state.flights.length);
  let after = before;
  let missionAfter = rating;
  if (ranked) {
    after = wasm.mission_elo(before, rating, score, k, 0);
    missionAfter = wasm.mission_elo(before, rating, score, k, 1);
    state.elo = after;
    state.missionElo[id] = missionAfter;
    state.ranked[id] = { score, elo: after, delta: after - before, at: Date.now() };
  }
  state.flights.push({ contractId: id, score, mode: ranked ? 'ranked' : 'practice', at: Date.now() });
  if (state.flights.length > 200) state.flights = state.flights.slice(-200);
  persist();
  notify();
  return { ranked, alreadyRanked: already, before, after, delta: after - before, missionBefore: rating, missionAfter, k };
}

/// Sign arbitrary bytes with the active license; returns 64 raw bytes.
export function sign(bytes) {
  wasm.mission_input_reset();
  for (const byte of bytes) {
    if (!wasm.mission_input_absorb(byte)) throw Error('signature input rejected');
  }
  if (wasm.mission_identity_sign() !== 1) throw Error('no active flight license');
  return Uint8Array.from({ length: wasm.mission_signature_len() }, (_, i) => wasm.mission_signature_byte(i));
}

export function reset() {
  try {
    localStorage.removeItem(KEY);
  } catch {
    // Storage may be unavailable; clearing the in-memory state is enough.
  }
  state = null;
  wasm.mission_identity_clear();
}
