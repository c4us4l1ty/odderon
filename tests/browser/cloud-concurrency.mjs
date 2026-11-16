// Workerd + real D1/R2 storage race gate. All requests reach the production
// Cloudflare Worker and the same Rust wasm authority the browser uses.
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
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
  let commitRace, raceFailure;
  await pages[0].route('**/api/v1/commit', async route => {
    try {
      const payload = route.request().postData();
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
} finally {
  if (release) release();
  if (browser) await browser.close();
  await new Promise(resolve => server.close(resolve));
  await authority.close();
}
