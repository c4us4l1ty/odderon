// Cloudflare D1/R2 shell around the exact same Rust authority engine as the
// local HTTP server. A D1 batch is atomic, but a preceding snapshot is not:
// every write is guarded by a single global revision, and stale proposals
// are discarded and recomputed. R2 objects are immutable, content-addressed
// by both the canonical FDR hash and the SHA-256 of the stored payload.

const encoder = new TextEncoder();
const decoder = new TextDecoder('utf-8', { fatal: true });
const MAX_BODY = 256 * 1024;
const MAX_RETRIES = 16;
const ZERO_KEY = '00'.repeat(32);
const HEX32 = /^[0-9a-f]{64}$/i;

const response = (status, value) => new Response(JSON.stringify(value), {
  status, headers: { 'content-type': 'application/json; charset=utf-8', 'cache-control': 'no-store' },
});
const bad = (message = 'invalid request') => response(400, { error: message });
const num = value => Number.isInteger(value) && value >= 0 && value <= 0xffffffff;
const key = value => typeof value === 'string' && HEX32.test(value) ? value.toLowerCase() : null;
const hex = bytes => Array.from(bytes, b => b.toString(16).padStart(2, '0')).join('');

// The wasm instance is used synchronously, with no await between filling the
// input and reading the output. A Worker isolate executes JS on one thread.
export function bridge(wasm, input) {
  const data = encoder.encode(JSON.stringify(input));
  const ptr = wasm.authority_input(data.length);
  if (!ptr) throw Error('authority input exceeds bound');
  new Uint8Array(wasm.memory.buffer, ptr, data.length).set(data);
  if (wasm.authority_run() !== 1) throw Error('authority bridge failed');
  const len = wasm.authority_output_len(), out = wasm.authority_output_ptr();
  return JSON.parse(decoder.decode(new Uint8Array(wasm.memory.buffer, out, len)));
}

async function readBounded(req) {
  const declared = req.headers.get('content-length');
  if (declared !== null && (!/^\d+$/.test(declared) || Number(declared) > MAX_BODY)) throw RangeError('body too large');
  if (!req.body) return '';
  const reader = req.body.getReader(), chunks = [];
  let size = 0;
  while (true) {
    const { value, done } = await reader.read();
    if (done) break;
    size += value.byteLength;
    if (size > MAX_BODY) { await reader.cancel(); throw RangeError('body too large'); }
    chunks.push(value);
  }
  const data = new Uint8Array(size);
  let offset = 0;
  for (const chunk of chunks) { data.set(chunk, offset); offset += chunk.byteLength; }
  return decoder.decode(data);
}

async function first(db, sql, ...values) {
  return db.prepare(sql).bind(...values).first();
}
function statement(db, sql, ...values) { return db.prepare(sql).bind(...values); }
function revisionStmt(db, revision) {
  return statement(db, `UPDATE authority_revision
    SET revision=CASE WHEN revision=?1 AND revision<9007199254740990 THEN revision+1 ELSE NULL END
    WHERE id=1`, revision);
}
// If an administrator removed the singleton after our snapshot, an UPDATE
// of zero rows must fail rather than allowing an unguarded batch to commit.
function missingRevisionStmt(db) {
  return db.prepare(`INSERT INTO authority_revision (id,revision)
    SELECT 1,NULL WHERE NOT EXISTS (SELECT 1 FROM authority_revision WHERE id=1)`);
}
async function revision(db) {
  const row = await first(db, 'SELECT revision FROM authority_revision WHERE id=1');
  if (!row || !Number.isSafeInteger(row.revision) || row.revision < 0) throw Error('authority migration missing or invalid');
  return row.revision;
}
async function apply(db, before, statements) {
  try {
    await db.batch([revisionStmt(db, before), missingRevisionStmt(db), ...statements]);
    return true;
  } catch (error) {
    // Only a changed revision is a conflict. Other storage errors are not
    // converted into success or retried as though they were contention.
    if (await revision(db) !== before) return false;
    throw error;
  }
}
function playerStmt(db, pubkey, player) {
  return statement(db, `INSERT INTO players (pubkey,elo,ranked,callsign) VALUES (?1,?2,?3,?4)
    ON CONFLICT(pubkey) DO UPDATE SET elo=excluded.elo,ranked=excluded.ranked,callsign=excluded.callsign`,
    pubkey, player.elo, player.ranked, player.callsign);
}
async function getFlight(db, bucket, hash) {
  const row = await first(db, `SELECT f.hash,f.pubkey,f.contract,f.attempt,f.mode,f.score,
      f.delegation_mask,f.r2_key,r.receipt
    FROM fdrs f LEFT JOIN receipts r ON r.pubkey=f.pubkey AND r.contract=f.contract AND r.attempt=f.attempt
    WHERE f.hash=?1`, hash);
  if (!row) return null;
  if (!row.receipt) throw Error('stored flight is missing its receipt');
  const object = await bucket.get(row.r2_key || row.hash); // legacy read-only layout
  if (!object) throw Error('stored flight payload is missing');
  return { hash: row.hash, pubkey: row.pubkey, contract: row.contract, attempt: row.attempt,
    mode: row.mode, score: row.score, delegation_mask: row.delegation_mask,
    payload: await object.text(), receipt: row.receipt };
}
async function objectKey(flight) {
  const digest = new Uint8Array(await crypto.subtle.digest('SHA-256', encoder.encode(flight.payload)));
  return `v1/${flight.hash}/${hex(digest)}`;
}
function route(wasm, seed, method, path, body, now, subject, snapshot) {
  return bridge(wasm, { op: 'route', seed, method, path, body, now_s: now, subject, snapshot });
}
function engineResponse(result) {
  if (!Number.isInteger(result.status) || typeof result.body !== 'string') throw Error('authority rejected a storage snapshot');
  return new Response(result.body, { status: result.status,
    headers: { 'content-type': 'application/json; charset=utf-8', 'cache-control': 'no-store' } });
}

async function submitOrCommit(request, env, wasm, body, parsed, now) {
  const isCommit = new URL(request.url).pathname === '/api/v1/commit';
  const ticket = isCommit ? parsed : parsed.ticket;
  const pk = key(ticket?.pubkey), contract = ticket?.contract_id;
  if (!pk || !num(contract) || (!isCommit && (!num(ticket.attempt_num) || ticket.attempt_num < 1 || typeof parsed.fdr_payload !== 'string'))) return bad();
  let info;
  if (!isCommit) {
    info = bridge(wasm, { op: 'inspect', payload: parsed.fdr_payload });
    if (info.status !== 200 || !key(info.hash)) return bad('malformed or unsigned recorder');
  }
  const path = new URL(request.url).pathname;
  for (let attempt = 0; attempt < MAX_RETRIES; attempt++) {
    // Starting each retry on the primary gives the revision read and all
    // subsequent session reads a consistent lower bound on prior writes.
    const db = env.DB.withSession('first-primary');
    const rev = await revision(db);
    const subject = { pubkey: pk, contract };
    const snapshot = {};
    if (isCommit) {
      const row = await first(db, 'SELECT n FROM attempts WHERE pubkey=?1 AND contract=?2', pk, contract);
      snapshot.attempts = row?.n ?? 0;
    } else {
      snapshot.player = await first(db, 'SELECT elo,ranked,callsign FROM players WHERE pubkey=?1', pk);
      const mission = await first(db, 'SELECT elo FROM missions WHERE contract=?1', contract);
      snapshot.mission_elo = mission?.elo ?? null;
      snapshot.commitment = await first(db, `SELECT attempt,design_hash,prediction_hash,mode,server_time
        FROM commitments WHERE pubkey=?1 AND contract=?2 AND attempt=?3`, pk, contract, ticket.attempt_num);
      const prior = await first(db, 'SELECT attempt,receipt FROM receipts WHERE pubkey=?1 AND contract=?2 AND attempt=?3', pk, contract, ticket.attempt_num);
      snapshot.receipt = prior ? { attempt: prior.attempt, bytes: prior.receipt } : null;
      snapshot.fdr = await getFlight(db, env.FDRS, info.hash);
    }
    const result = route(wasm, env.SERVER_SEED, 'POST', path, body, now, subject, snapshot);
    if (result.status !== 200) {
      if (await revision(db) !== rev) continue;
      return engineResponse(result);
    }
    const parsedResponse = JSON.parse(result.body);
    if (isCommit) {
      // The signed ticket and persisted counter/commitment are published in
      // the same guarded D1 transaction, never in separate writes.
      const t = parsedResponse;
      const statements = [
        statement(db, `INSERT INTO attempts(pubkey,contract,n) VALUES (?1,?2,?3)
          ON CONFLICT(pubkey,contract) DO UPDATE SET n=excluded.n`, pk, contract, t.attempt_num),
        statement(db, `INSERT INTO commitments(pubkey,contract,attempt,design_hash,prediction_hash,mode,server_time)
          VALUES (?1,?2,?3,?4,?5,?6,?7)`, pk, contract, t.attempt_num, t.design_hash, t.prediction_hash, t.mode, t.server_time_s),
      ];
      if (await apply(db, rev, statements)) return engineResponse(result);
      continue;
    }
    // An already-issued receipt is immutable. The engine has checked that
    // this exact ticket, commitment and replay correspond to its FDR hash.
    if (snapshot.receipt) return engineResponse(result);
    const flight = result.flight, player = result.player;
    if (!flight || !player || flight.hash !== info.hash || !result.receipt_bytes) throw Error('authority proposal incomplete');
    const r2key = await objectKey(flight);
    await env.FDRS.put(r2key, flight.payload);
    const statements = [
      playerStmt(db, pk, player),
      statement(db, `INSERT INTO missions(contract,elo) VALUES (?1,?2)
        ON CONFLICT(contract) DO UPDATE SET elo=excluded.elo`, contract, result.mission_elo),
      statement(db, 'INSERT INTO receipts(pubkey,contract,attempt,receipt) VALUES (?1,?2,?3,?4)', pk, contract, ticket.attempt_num, result.receipt_bytes),
      statement(db, `INSERT INTO fdrs(hash,pubkey,contract,attempt,mode,score,delegation_mask,r2_key)
        VALUES (?1,?2,?3,?4,?5,?6,?7,?8)
        ON CONFLICT(hash) DO UPDATE SET pubkey=excluded.pubkey,contract=excluded.contract,attempt=excluded.attempt,
          mode=excluded.mode,score=excluded.score,delegation_mask=excluded.delegation_mask,r2_key=excluded.r2_key`,
        flight.hash, flight.pubkey, flight.contract, flight.attempt, flight.mode, flight.score, flight.delegation_mask, r2key),
    ];
    if (await apply(db, rev, statements)) return engineResponse(result);
  }
  return response(503, { error: 'authority busy, retry the same request', code: 'AuthorityBusy' });
}

async function callsign(env, wasm, body, parsed, now) {
  const pk = key(parsed?.pubkey), name = parsed?.name;
  if (!pk || typeof name !== 'string' || !/^[a-zA-Z0-9 _-]{1,24}$/.test(name) || name.trim() !== name) return bad();
  const normalized = name.toUpperCase();
  for (let attempt = 0; attempt < MAX_RETRIES; attempt++) {
    const db = env.DB.withSession('first-primary'), rev = await revision(db);
    const player = await first(db, 'SELECT elo,ranked,callsign FROM players WHERE pubkey=?1', pk);
    const owner = await first(db, 'SELECT name,pubkey FROM callsigns WHERE name=?1', normalized);
    const result = route(wasm, env.SERVER_SEED, 'POST', '/api/v1/callsign', body, now,
      { pubkey: pk, contract: 0 }, { player, owner });
    if (result.status !== 200) {
      if (await revision(db) !== rev) continue;
      return engineResponse(result);
    }
    if (!result.player) throw Error('authority callsign proposal incomplete');
    const writes = [
      statement(db, `INSERT INTO callsigns(name,pubkey) VALUES (?1,?2)
        ON CONFLICT(name) DO UPDATE SET pubkey=excluded.pubkey WHERE callsigns.pubkey=excluded.pubkey`, normalized, pk),
      playerStmt(db, pk, result.player),
    ];
    if (await apply(db, rev, writes)) return engineResponse(result);
  }
  return response(503, { error: 'authority busy, retry the same request', code: 'AuthorityBusy' });
}

async function board(db, url) {
  const seen = new Set();
  for (const [name] of url.searchParams) {
    if (!['contract', 'page'].includes(name) || seen.has(name)) return bad('bad query');
    seen.add(name);
  }
  const p = url.searchParams.get('page') ?? '1', c = url.searchParams.get('contract');
  if (!/^\d+$/.test(p) || Number(p) < 1 || Number(p) > 10000) return bad('bad page');
  if (c !== null && (!/^\d+$/.test(c) || ![101, 102, 103].includes(Number(c)))) return bad('unknown contract');
  const sql = c === null
    ? `SELECT p.callsign,f.pubkey,COALESCE(p.elo,1200) AS elo,f.score AS score_s,f.delegation_mask,f.hash AS fdr_hash
       FROM fdrs f LEFT JOIN players p ON f.pubkey=p.pubkey WHERE f.mode=0
       ORDER BY COALESCE(p.elo,1200) DESC,f.hash ASC LIMIT 25 OFFSET ?1`
    : `SELECT p.callsign,f.pubkey,COALESCE(p.elo,1200) AS elo,f.score AS score_s,f.delegation_mask,f.hash AS fdr_hash
       FROM fdrs f LEFT JOIN players p ON f.pubkey=p.pubkey WHERE f.mode=0 AND f.contract=?1
       ORDER BY f.score DESC,f.hash ASC LIMIT 25 OFFSET ?2`;
  const offset = (Number(p) - 1) * 25;
  const query = c === null ? statement(db, sql, offset) : statement(db, sql, Number(c), offset);
  const result = await query.all();
  return response(200, result.results);
}

export async function handle(request, env, wasm) {
  try {
    if (!key(env.SERVER_SEED)) return response(503, { error: 'authority signing key is not configured' });
    const url = new URL(request.url), path = url.pathname, method = request.method;
    const now = Math.floor(Date.now() / 1000);
    if (method === 'POST' && ['/api/v1/commit', '/api/v1/submit', '/api/v1/submit-segment', '/api/v1/callsign'].includes(path)) {
      let body, parsed;
      try { body = await readBounded(request); parsed = JSON.parse(body); }
      catch (error) { return error instanceof RangeError ? response(413, { error: 'request body too large' }) : bad(); }
      if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) return bad();
      if (path === '/api/v1/callsign') return await callsign(env, wasm, body, parsed, now);
      return await submitOrCommit(request, env, wasm, body, parsed, now);
    }
    const db = env.DB.withSession('first-primary');
    if (method === 'GET' && path === '/api/v1/leaderboard') return await board(db, url);
    if (method === 'GET' && path.startsWith('/api/v1/player/')) {
      const pk = key(path.slice('/api/v1/player/'.length));
      if (!pk) return bad('bad public key');
      const player = await first(db, 'SELECT elo,ranked,callsign FROM players WHERE pubkey=?1', pk);
      return engineResponse(route(wasm, env.SERVER_SEED, method, `/api/v1/player/${pk}`, '', now,
        { pubkey: pk, contract: 0 }, { player }));
    }
    if (method === 'GET' && path.startsWith('/api/v1/verify/')) {
      const hash = key(path.slice('/api/v1/verify/'.length));
      if (!hash) return bad('bad hash');
      const flight = await getFlight(db, env.FDRS, hash);
      if (!flight) return response(404, { error: 'not found' });
      return engineResponse(route(wasm, env.SERVER_SEED, method, `/api/v1/verify/${hash}`, '', now,
        { pubkey: flight.pubkey || ZERO_KEY, contract: flight.contract }, { fdr: flight }));
    }
    return response(404, { error: 'not found' });
  } catch (error) {
    // Do not include the request or seed in logs or the response.
    console.error('authority storage or execution failure', String(error?.message ?? error));
    return response(503, { error: 'authority storage or execution failure', code: 'Storage' });
  }
}
