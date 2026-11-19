// Canonical authority wire layouts. Signatures are checked by the same
// Ed25519 WASM implementation as the flight license, not by JSON properties.
import { SERVER_PUBLIC_KEY } from './authority-config.js';
import * as recorder from './recorder.js';

const encoder = new TextEncoder();
export const toHex = bytes => Array.from(bytes, b => b.toString(16).padStart(2, '0')).join('');
export function fromHex(hex, length) {
  if (typeof hex !== 'string' || hex.length !== 2 * length || !/^[0-9a-f]+$/i.test(hex)) throw Error('invalid hex');
  return Uint8Array.from({ length }, (_, i) => parseInt(hex.slice(i * 2, i * 2 + 2), 16));
}
export function configured() { return /^[0-9a-f]{64}$/i.test(SERVER_PUBLIC_KEY); }
export function serverKey() { return fromHex(SERVER_PUBLIC_KEY, 32); }

class Writer {
  constructor(domain) { this.bytes = [...encoder.encode(domain)]; }
  raw(bytes) { this.bytes.push(...bytes); return this; }
  number(value, size, type) {
    const buffer = new ArrayBuffer(size), view = new DataView(buffer);
    if (type === 'u16') view.setUint16(0, value, false);
    else if (type === 'u32') view.setUint32(0, value, false);
    else if (type === 'i64') view.setBigInt64(0, BigInt(value), false);
    else view.setFloat64(0, value, false);
    return this.raw(new Uint8Array(buffer));
  }
  u8(value) { this.bytes.push(value); return this; }
  u16(value) { return this.number(value, 2, 'u16'); }
  u32(value) { return this.number(value, 4, 'u32'); }
  i64(value) { return this.number(value, 8, 'i64'); }
  f64(value) { return this.number(value, 8, 'f64'); }
  finish() { return Uint8Array.from(this.bytes); }
}
const uint32 = value => Number.isInteger(value) && value >= 0 && value <= 0xffffffff;
const integer = value => Number.isSafeInteger(value);

export function commitBytes(contract, pubkey, design, prediction) {
  if (!uint32(contract)) throw Error('invalid contract');
  return new Writer('odderon-client-commit-v1').u32(contract).raw(pubkey).raw(design).raw(prediction).finish();
}
export function callsignBytes(name, pubkey) {
  const text = encoder.encode(name);
  if (text.length > 24) throw Error('invalid callsign');
  return new Writer('odderon-callsign-v1').raw(pubkey).u16(text.length).raw(text).finish();
}
function ticketBytes(t) {
  if (!uint32(t.contract_id) || !uint32(t.attempt_num) || t.attempt_num < 1 || !integer(t.server_time_s)
      || t.mode !== (t.attempt_num === 1 ? 0 : 1)) throw Error('invalid ticket');
  return new Writer('odderon-commit-ticket-v1').u32(t.contract_id).raw(fromHex(t.pubkey, 32))
    .u32(t.attempt_num).u8(t.mode).raw(fromHex(t.design_hash, 32))
    .raw(fromHex(t.prediction_hash, 32)).i64(t.server_time_s).finish();
}
export function receiptBytes(r) {
  if (!r || r.schema_v3 !== 3 || !uint32(r.contract_id) || !uint32(r.attempt_num) || r.attempt_num < 1
      || r.mode !== (r.attempt_num === 1 ? 0 : 1) || !integer(r.timestamp_s)
      || ![r.score_s, r.elo_before, r.elo_after, r.mission_elo_after].every(Number.isFinite)
      || r.score_s < 0 || r.score_s > 1) throw Error('invalid receipt');
  return new Writer('odderon-server-receipt-v3').u16(r.schema_v3).raw(fromHex(r.fdr_hash, 32))
    .raw(fromHex(r.pubkey, 32)).u32(r.contract_id).u32(r.attempt_num).u8(r.mode)
    .f64(r.score_s).f64(r.elo_before).f64(r.elo_after).f64(r.mission_elo_after).i64(r.timestamp_s)
    .raw(fromHex(r.server_sig, 64)).finish();
}
function verifySignature(wasm, message, signature) {
  const key = serverKey();
  wasm.mission_input_reset();
  for (const b of message) if (!wasm.mission_input_absorb(b)) return false;
  wasm.mission_verify_reset();
  for (const b of key) if (!wasm.mission_verify_absorb(0, b)) return false;
  for (const b of signature) if (!wasm.mission_verify_absorb(1, b)) return false;
  return wasm.mission_verify() === 1;
}
export function verifyTicket(wasm, ticket, request) {
  try {
    if (ticket.contract_id !== request.contract_id || ticket.pubkey !== request.pubkey
        || ticket.design_hash !== request.design_hash || ticket.prediction_hash !== request.prediction_hash) return false;
    return verifySignature(wasm, ticketBytes(ticket), fromHex(ticket.server_sig, 64));
  } catch { return false; }
}

/// The FDR digest is recomputed inside WASM from typed fields, not taken from
/// the editable JSON digest. Also require the pilot's own valid signature.
export function verifyReceipt(wasm, receipt, document) {
  try {
    if (!configured()) return false;
    const bytes = receiptBytes(receipt);
    const { fields, callsign } = recorder.fieldsFromJson(document);
    recorder.loadFields(wasm, fields, callsign);
    if (wasm.mission_fdr_signature_valid() !== 1) return false;
    const digest = toHex(Array.from({ length: 32 }, (_, i) => wasm.mission_fdr_digest_byte(i)));
    if (receipt.fdr_hash !== digest || receipt.pubkey !== document.public_key
        || receipt.contract_id !== document.contract_id
        || receipt.mode !== (document.attempt_mode === 'Ranked' ? 0 : 1)
        || !Object.is(receipt.score_s, document.metrics[4])) return false;
    return verifySignature(wasm, bytes.subarray(0, bytes.length - 64), bytes.subarray(bytes.length - 64));
  } catch { return false; }
}

export function label(wasm, receipt, document) {
  if (!verifyReceipt(wasm, receipt, document)) return 'UNVERIFIED LOCAL / PRACTICE';
  return receipt.mode === 0 ? '★ CERTIFIED SERVER-RANKED RECEIPT' : 'SERVER-VERIFIED PRACTICE';
}
