// Same-origin transport to the authority. The configured public key is a
// static trust anchor, never learned from a response or browser storage.
import * as identity from './identity.js';
import { configured, toHex, commitBytes, callsignBytes, verifyTicket } from './authority-crypto.js';

export { configured };
async function request(path, options = {}) {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 30000);
  try {
    const response = await fetch(`/api/v1/${path}`, { ...options, cache: 'no-store', signal: controller.signal });
    const body = await response.json().catch(() => null);
    if (!response.ok) throw Error(body?.error || body?.code || `Authority HTTP ${response.status}`);
    return body;
  } finally { clearTimeout(timeout); }
}
const post = (path, body) => request(path, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(body) });

export async function commit(wasm, contract) {
  if (!configured()) throw Error('No trusted server public key is configured. Select Practice to fly locally.');
  if (wasm.mission_commit_prepare() !== 1) throw Error('Could not prepare the locked design before flight.');
  const bytes = Uint8Array.from({ length: 64 }, (_, i) => wasm.mission_commit_hash_byte(i));
  const pubkey = Uint8Array.from(identity.profile().publicKey);
  const design = bytes.subarray(0, 32), prediction = bytes.subarray(32);
  const body = { contract_id: contract, pubkey: toHex(pubkey), design_hash: toHex(design), prediction_hash: toHex(prediction),
    client_sig: toHex(identity.sign(commitBytes(contract, pubkey, design, prediction))) };
  const ticket = await post('commit', body);
  if (!verifyTicket(wasm, ticket, body)) throw Error('The authority returned an invalid or mismatched ticket.');
  return ticket;
}
export function submit(payload, ticket) { return post('submit', { fdr_payload: payload, ticket, segments: null }); }
export async function registerCallsign(name) {
  if (!configured()) throw Error('No trusted authority configured.');
  const pubkey = Uint8Array.from(identity.profile().publicKey);
  return post('callsign', { name, pubkey: toHex(pubkey), sig: toHex(identity.sign(callsignBytes(name, pubkey))) });
}
export function player(pubkey) { return request(`player/${pubkey}`); }
export function leaderboard(contract, page = 1) {
  const query = new URLSearchParams({ page: String(page) });
  if (contract) query.set('contract', String(contract));
  return request(`leaderboard?${query}`);
}
export function publicFlight(hash) {
  if (!/^[0-9a-f]{64}$/i.test(hash)) throw Error('Enter a 64-character flight hash.');
  return request(`verify/${hash}`);
}
