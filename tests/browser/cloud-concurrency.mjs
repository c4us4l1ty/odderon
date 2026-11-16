// Workerd + real D1/R2 storage race gate. All requests reach the production
// Cloudflare Worker and the same Rust wasm authority the browser uses.
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { resolve, extname } from 'node:path';
import { chromium } from 'playwright-core';
import bundledChromium from '@sparticuz/chromium';
import { startCloudAuthority } from './support/cloud-authority.mjs';

const authority = await startCloudAuthority();
const root = resolve('web');
const server = createServer(async (req, res) => {
  const url = new URL(req.url, 'http://test');
  if (url.pathname.startsWith('/api/v1/')) { await authority.handle(req, res); return; }
  if (url.pathname === '/authority-config.js') {
    res.setHeader('content-type', 'text/javascript');
    res.end(`export const SERVER_PUBLIC_KEY = ${JSON.stringify(authority.key)};`); return;
  }
  const path = resolve(root, '.' + url.pathname.replace(/\/$/, '/index.html'));
  if (!path.startsWith(root + '/')) { res.writeHead(403).end(); return; }
  try {
    const bytes = await readFile(path);
    res.setHeader('content-type', ({ '.html':'text/html', '.js':'text/javascript', '.css':'text/css', '.wasm':'application/wasm' })[extname(path)] || 'application/octet-stream');
    res.end(bytes);
  } catch { res.writeHead(404).end(); }
});
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
const base = `http://127.0.0.1:${server.address().port}`;
let browser, release;
try {
  const C = bundledChromium.default ?? bundledChromium;
  browser = await chromium.launch({ executablePath: process.env.CHROME_BIN || await C.executablePath(), headless: true, args: ['--no-sandbox', '--disable-dev-shm-usage', '--no-proxy-server'] });
  const pages = await Promise.all(Array.from({ length: 3 }, () => browser.newPage({ viewport: { width: 1440, height: 1000 } })));
  await Promise.all(pages.map(async page => {
    await page.goto(`${base}/mission.html`);
    await page.waitForFunction(() => missionDiagnostics?.wasm?.() && !missionDiagnostics.pending, null, { timeout: 60000 });
  }));
  const pubkeys = await Promise.all(pages.map(page => page.evaluate(() => missionDiagnostics.profile().publicKey.map(b => b.toString(16).padStart(2, '0')).join(''))));
  assert.equal(new Set(pubkeys).size, 3, 'the race needs three independent pilots');

  // Two authenticated pilots race for one case-insensitive name. Both read
  // snapshots before the writes may complete; exactly one must own it.
  const claim = page => page.evaluate(async () => {
    const identity = await import('./identity.js'), crypto = await import('./authority-crypto.js');
    const pubkey = Uint8Array.from(identity.profile().publicKey), name = 'RACE-PILOT';
    const body = { name, pubkey: crypto.toHex(pubkey), sig: crypto.toHex(identity.sign(crypto.callsignBytes(name, pubkey))) };
    const response = await fetch('/api/v1/callsign', { method: 'POST', headers: { 'content-type':'application/json' }, body: JSON.stringify(body) });
    return { status: response.status, body: await response.json() };
  });
  const claims = await Promise.all([claim(pages[0]), claim(pages[1])]);
  assert.deepEqual(claims.map(x => x.status).sort(), [200, 400]);
  assert.equal(claims.find(x => x.status === 400).body.code, 'CallsignTaken');
  const owners = (await authority.query('SELECT name,pubkey FROM callsigns WHERE name=?1', 'RACE-PILOT')).results;
  assert.equal(owners.length, 1);
  assert.ok(pubkeys.slice(0, 2).includes(owners[0].pubkey));
  console.log('PASS concurrent signed callsign claims have exactly one owner');

  // Hold the browser's very first commit and race eight identical signed
  // copies against D1. Return attempt 1 to that page so it can really fly.
  let commitRace, raceFailure, commitBody;
  await pages[0].route('**/api/v1/commit', async route => {
    try {
      const payload = route.request().postData();
      commitBody = payload;
      const replies = await Promise.all(Array.from({ length: 8 }, async () => {
        const r = await fetch(`${base}/api/v1/commit`, { method: 'POST', headers: { 'content-type':'application/json' }, body: payload });
        return { status: r.status, body: await r.json() };
      }));
      commitRace = replies;
      const first = replies.find(r => r.status === 200 && r.body.attempt_num === 1);
      if (!first) throw Error(`no first ticket: ${JSON.stringify(replies)}`);
      await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(first.body) });
    } catch (error) { raceFailure = error; await route.abort(); }
  });

  // Hold all three actual browser submissions, then issue them concurrently,
  // with three duplicate copies of the first. This exercises both lost-update
  // protection for mission Elo and idempotence for one attempt.
  const bodies = Array(3), results = Array(3);
  const arrivals = pages.map(() => {
    let resolve;
    const promise = new Promise(r => { resolve = r; });
    return { resolve, promise };
  });
  const released = new Promise(r => { release = r; });
  for (let i = 0; i < pages.length; i++) {
    await pages[i].route('**/api/v1/submit', async route => {
      bodies[i] = route.request().postData(); arrivals[i].resolve();
      await released;
      if (results[i]) await route.fulfill({ status: results[i].status, contentType: 'application/json', body: JSON.stringify(results[i].body) });
      else await route.abort();
    });
  }
  await Promise.all(pages.map(async page => { await page.click('#example'); await page.click('#prefill'); await page.click('#execute'); }));
  let timer;
  try {
    await Promise.race([Promise.all(arrivals.map(a => a.promise)), new Promise((_, reject) => { timer = setTimeout(() => reject(raceFailure || Error('concurrent flights did not reach submit')), 90000); })]);
  } finally { clearTimeout(timer); }
  if (raceFailure) throw raceFailure;
  assert.equal(commitRace.length, 8);
  assert.ok(commitRace.every(r => r.status === 200), JSON.stringify(commitRace));
  assert.deepEqual(commitRace.map(r => r.body.attempt_num).sort((a, b) => a - b), [1,2,3,4,5,6,7,8]);
  assert.equal(commitRace.filter(r => r.body.mode === 0).length, 1);
  assert.ok(commitRace.filter(r => r.body.attempt_num > 1).every(r => r.body.mode === 1));
  console.log('PASS eight simultaneous first commits yield exactly one ranked ticket');

  const beforeRevision = (await authority.query('SELECT revision FROM authority_revision WHERE id=1')).results[0].revision;
  const priorMission = (await authority.query('SELECT elo FROM missions WHERE contract=?1', 101)).results[0]?.elo ?? 800;
  const racedBodies = [...bodies, bodies[0], bodies[0], bodies[0]];
  const responses = await Promise.all(racedBodies.map(async body => {
    const r = await fetch(`${base}/api/v1/submit`, { method: 'POST', headers: { 'content-type':'application/json' }, body });
    return { status: r.status, body: await r.json() };
  }));
  assert.ok(responses.every(r => r.status === 200 && r.body.accepted), JSON.stringify(responses));
  for (const duplicate of responses.slice(3)) assert.deepEqual(duplicate.body.receipt, responses[0].body.receipt);
  for (let i = 0; i < 3; i++) results[i] = responses[i];
  release(); release = null;
  await Promise.all(pages.map(page => page.waitForFunction(() => !document.getElementById('debrief').hidden && !missionDiagnostics.pending, null, { timeout: 180000 })));
  for (const page of pages) assert.equal(await page.locator('#certification-status').textContent(), '★ CERTIFIED SERVER-RANKED RECEIPT');
  const afterRevision = (await authority.query('SELECT revision FROM authority_revision WHERE id=1')).results[0].revision;
  assert.equal(afterRevision - beforeRevision, 3, 'three unique submissions must commit exactly three mutations');
  for (const pk of pubkeys) {
    const row = (await authority.query('SELECT elo,ranked FROM players WHERE pubkey=?1', pk)).results[0];
    assert.equal(row.ranked, 1, 'each pilot is rated once');
  }
  const receipts = responses.slice(0, 3).map(r => r.body.receipt);
  // Reconstruct a serial ordering. Each receipt must use the mission Elo
  // produced by the previous accepted flight, not a shared stale snapshot.
  let mission = priorMission;
  const remaining = [...receipts];
  while (remaining.length) {
    const index = remaining.findIndex(r => {
      const expected = 1 / (1 + 10 ** ((mission - r.elo_before) / 400));
      return Math.abs(r.elo_after - (r.elo_before + 40 * (r.score_s - expected))) < 1e-8
        && Math.abs(r.mission_elo_after - (mission + 8 * (expected - r.score_s))) < 1e-8;
    });
    assert.ok(index >= 0, `no serial Elo order: start=${priorMission} remainder=${JSON.stringify(remaining)}`);
    mission = remaining.splice(index, 1)[0].mission_elo_after;
  }
  const storedMission = (await authority.query('SELECT elo FROM missions WHERE contract=?1', 101)).results[0].elo;
  assert.ok(Math.abs(storedMission - mission) < 1e-8, 'the database holds the last serialized mission Elo');
  console.log('PASS concurrent distinct flights serialize both player and mission Elo; duplicates are idempotent');

  // The R2 object is outside the D1 transaction. A corrupt or missing object
  // must not be returned as a verified public flight.
  const hash = responses[0].body.fdr_hash;
  const row = (await authority.query('SELECT r2_key FROM fdrs WHERE hash=?1', hash)).results[0];
  const original = (await authority.object(row.r2_key)).value;
  assert.ok(original && row.r2_key.startsWith(`v1/${hash}/`));
  try {
    await authority.object(row.r2_key, '#verify=corrupt');
    const corrupt = await fetch(`${base}/api/v1/verify/${hash}`);
    assert.equal(corrupt.status, 503, 'corrupt R2 data must fail closed');
    assert.doesNotMatch(await corrupt.text(), /5e{16}/);
  } finally { await authority.object(row.r2_key, original); }
  const restored = await fetch(`${base}/api/v1/verify/${hash}`);
  assert.equal(restored.status, 200);
  console.log('PASS corrupted R2 data fails closed and a restored public replay works');

  // Upgrade a genuine legacy 0001 database and hash-only R2 object. This
  // intentionally tests the old data shape, not just a fresh two-migration
  // database. It cannot repair Elo that the old adapter may have raced.
  const legacy = await startCloudAuthority({ migrateThrough: 1 });
  try {
    const f = (await authority.query('SELECT * FROM fdrs WHERE hash=?1', hash)).results[0];
    const r = (await authority.query('SELECT * FROM receipts WHERE pubkey=?1 AND contract=?2 AND attempt=?3', f.pubkey, f.contract, f.attempt)).results[0];
    await legacy.query('INSERT INTO fdrs(hash,pubkey,contract,attempt,mode,score,delegation_mask) VALUES (?1,?2,?3,?4,?5,?6,?7)', f.hash, f.pubkey, f.contract, f.attempt, f.mode, f.score, f.delegation_mask);
    await legacy.query('INSERT INTO receipts(pubkey,contract,attempt,receipt) VALUES (?1,?2,?3,?4)', r.pubkey, r.contract, r.attempt, r.receipt);
    await legacy.object(hash, original);
    const beforeMigration = await legacy.fetch(`/api/v1/verify/${hash}`);
    assert.equal(beforeMigration.status, 503, 'old schema must fail closed before the upgrade');
    await legacy.migrate(1);
    assert.equal((await legacy.query('SELECT r2_key FROM fdrs WHERE hash=?1', hash)).results[0].r2_key, null);
    const upgraded = await legacy.fetch(`/api/v1/verify/${hash}`);
    assert.equal(upgraded.status, 200, await upgraded.clone().text());
    const restoredLegacy = await upgraded.json();
    assert.equal(restoredLegacy.fdr_payload, original);
    assert.deepEqual(restoredLegacy.receipt, responses[0].body.receipt);
    assert.equal((await legacy.query('SELECT revision FROM authority_revision WHERE id=1')).results[0].revision, 0);
  } finally { await legacy.close(); }
  console.log('PASS legacy D1 migration preserves signed hash-only R2 public replays');

  // A missing revision must never degrade into unguarded writes. Restore the
  // singleton after this fault so the rest of the gate uses the same DB.
  const oldRevision = (await authority.query('SELECT revision FROM authority_revision WHERE id=1')).results[0].revision;
  await authority.query('DELETE FROM authority_revision WHERE id=1');
  try {
    const missing = await fetch(`${base}/api/v1/commit`, { method:'POST', headers:{'content-type':'application/json'}, body:commitBody });
    assert.equal(missing.status, 503);
    assert.equal((await authority.query('SELECT n FROM attempts WHERE pubkey=?1 AND contract=101', pubkeys[0])).results[0].n, 8);
  } finally { await authority.query('INSERT INTO authority_revision(id,revision) VALUES(1,?1)', oldRevision); }
  console.log('PASS missing revision fails closed without consuming an attempt');

  // Drive an actual new ranked flight while injecting two storage faults:
  // first the R2 write fails, then a D1 receipt insert fails after R2 wrote.
  // Neither failure may publish a receipt or partially move Elo. The latter
  // can leave an orphan, which an exact retry safely reuses.
  const failurePage = pages[2];
  await failurePage.unroute('**/api/v1/submit');
  await failurePage.click('#unlock');
  await failurePage.selectOption('#mission-select', '102');
  const playerBefore = (await authority.query('SELECT elo,ranked FROM players WHERE pubkey=?1', pubkeys[2])).results[0];
  let failedBody;
  failurePage.on('request', request => { if (request.method() === 'POST' && request.url().endsWith('/api/v1/submit')) failedBody = request.postData(); });
  await authority.failPut(true);
  await failurePage.click('#example'); await failurePage.click('#prefill'); await failurePage.click('#execute');
  await failurePage.waitForSelector('#retry-submit', { timeout:180000 });
  const failedDoc = await failurePage.evaluate(() => missionDiagnostics.recorder().document);
  const request = JSON.parse(failedBody);
  const orphanKey = `v1/${failedDoc.canonical_digest}/${createHash('sha256').update(request.fdr_payload).digest('hex')}`;
  assert.equal((await authority.object(orphanKey)).value, null, 'failed R2 put must not create a payload');
  assert.deepEqual((await authority.query('SELECT elo,ranked FROM players WHERE pubkey=?1', pubkeys[2])).results[0], playerBefore);
  assert.equal((await authority.query('SELECT * FROM receipts WHERE pubkey=?1 AND contract=102', pubkeys[2])).results.length, 0);
  assert.equal((await authority.query('SELECT * FROM missions WHERE contract=102')).results.length, 0);
  const preRetryRevision = (await authority.query('SELECT revision FROM authority_revision WHERE id=1')).results[0].revision;
  await authority.failPut(false);
  await authority.query("CREATE TRIGGER injected_receipt_failure BEFORE INSERT ON receipts BEGIN SELECT RAISE(ABORT,'injected D1 failure'); END");
  try {
    const retryReply = failurePage.waitForResponse(response => response.request().method() === 'POST' && response.url().endsWith('/api/v1/submit'));
    await failurePage.click('#retry-submit');
    assert.equal((await retryReply).status(), 503);
    await failurePage.waitForSelector('#retry-submit', { timeout:30000 });
    assert.equal((await authority.object(orphanKey)).value, request.fdr_payload, 'D1 failure may leave an unreferenced R2 object');
    assert.equal((await authority.query('SELECT * FROM fdrs WHERE hash=?1', failedDoc.canonical_digest)).results.length, 0);
    assert.equal((await authority.query('SELECT * FROM receipts WHERE pubkey=?1 AND contract=102', pubkeys[2])).results.length, 0);
    assert.deepEqual((await authority.query('SELECT elo,ranked FROM players WHERE pubkey=?1', pubkeys[2])).results[0], playerBefore);
    assert.equal((await authority.query('SELECT revision FROM authority_revision WHERE id=1')).results[0].revision, preRetryRevision);
  } finally { await authority.query('DROP TRIGGER injected_receipt_failure'); }
  await failurePage.click('#retry-submit');
  await failurePage.waitForFunction(() => document.getElementById('certification-status')?.textContent.includes('CERTIFIED SERVER-RANKED') && !missionDiagnostics.pending, null, { timeout:180000 });
  const accepted = await failurePage.evaluate(() => missionDiagnostics.recorder().document);
  assert.equal((await authority.query('SELECT r2_key FROM fdrs WHERE hash=?1', accepted.canonical_digest)).results[0].r2_key, orphanKey);
  assert.equal((await authority.query('SELECT ranked FROM players WHERE pubkey=?1', pubkeys[2])).results[0].ranked, playerBefore.ranked + 1);
  assert.equal((await authority.query('SELECT revision FROM authority_revision WHERE id=1')).results[0].revision, preRetryRevision + 1);
  const duplicate = await fetch(`${base}/api/v1/submit`, { method:'POST', headers:{'content-type':'application/json'}, body:failedBody });
  assert.equal(duplicate.status, 200);
  assert.deepEqual((await duplicate.json()).receipt, accepted.server_receipt);
  assert.equal((await authority.query('SELECT revision FROM authority_revision WHERE id=1')).results[0].revision, preRetryRevision + 1);
  console.log('PASS R2/D1 failures roll back rating, preserve safe orphan and retry exactly once');
} finally {
  if (release) release();
  if (browser) await browser.close();
  await new Promise(resolve => server.close(resolve));
  await authority.close();
}
