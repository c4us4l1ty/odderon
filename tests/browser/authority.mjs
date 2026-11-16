// Flown browser/HTTP/SQLite authority oracle. The test key below is PUBLIC
// and for tests only; production must pin a different, privately held seed.
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { readFile } from 'node:fs/promises';
import { resolve, extname } from 'node:path';
import { chromium } from 'playwright-core';
import bundledChromium from '@sparticuz/chromium';
import { startAuthority } from './support/authority-server.mjs';
import { startCloudAuthority } from './support/cloud-authority.mjs';
import { decodeQrFromSvg } from './support/qr.mjs';

const cloud = process.env.ODDERON_AUTHORITY === 'cloudflare';
const authority = await (cloud ? startCloudAuthority() : startAuthority());
console.log(`Authority browser gate: ${cloud ? 'Cloudflare workerd / D1 / R2' : 'native HTTP / SQLite'}`);
const { key } = authority;
const root = resolve('web');
const proxy = createServer(async (req, res) => {
  const url = new URL(req.url, 'http://test');
  if (url.pathname.startsWith('/api/v1/')) {
    authority.handle(req, res); return;
  }
  if (url.pathname === '/authority-config.js') {
    res.setHeader('content-type', 'text/javascript');
    res.end(`export const SERVER_PUBLIC_KEY = ${JSON.stringify(key)};`); return;
  }
  const path = resolve(root, '.' + url.pathname.replace(/\/$/, '/index.html'));
  if (!path.startsWith(root + '/')) { res.writeHead(403).end(); return; }
  try {
    const bytes = await readFile(path);
    res.setHeader('content-type', ({ '.html':'text/html', '.js':'text/javascript', '.css':'text/css', '.wasm':'application/wasm' })[extname(path)] || 'application/octet-stream');
    res.end(bytes);
  } catch { res.writeHead(404).end(); }
});
await new Promise(r => proxy.listen(0, '127.0.0.1', r));
const base = `http://127.0.0.1:${proxy.address().port}`;
let browser;
try {
  const C = bundledChromium.default ?? bundledChromium;
  browser = await chromium.launch({ executablePath: process.env.CHROME_BIN || await C.executablePath(), headless: true, args: ['--no-sandbox', '--disable-dev-shm-usage', '--no-proxy-server'] });
  const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
  const errors = [], submitBodies = [], commitBodies = [];
  page.on('pageerror', error => errors.push(String(error)));
  page.on('request', request => {
    if (request.method() !== 'POST') return;
    if (request.url().endsWith('/api/v1/submit')) submitBodies.push(request.postData());
    if (request.url().endsWith('/api/v1/commit')) commitBodies.push(request.postData());
  });
  await page.addInitScript(() => {
    window.authorityEvents = [];
    const fetchNative = window.fetch.bind(window);
    window.fetch = async (...args) => {
      const url = String(args[0]);
      if (url === '/api/v1/commit') window.authorityEvents.push('commit-request');
      if (url === '/api/v1/submit') window.authorityEvents.push('submit-request');
      const response = await fetchNative(...args);
      if (url === '/api/v1/commit') window.authorityEvents.push('commit-response');
      if (url === '/api/v1/submit') window.authorityEvents.push('submit-response');
      return response;
    };
    const NativeWorker = window.Worker;
    window.Worker = class extends NativeWorker {
      constructor(...args) {
        super(...args);
        this.missionWorker = String(args[0]).endsWith('/mission-worker.js') || String(args[0]) === './mission-worker.js';
        this.flight = false;
        this.addEventListener('message', () => { if (this.flight) window.authorityEvents.push('flight-result'); });
      }
      postMessage(data, ...rest) {
        if (this.missionWorker && data?.action === 'split') window.authorityEvents.push('numerical-targeting-start');
        if (this.missionWorker && !data?.action) {
          this.flight = true;
          window.authorityEvents.push(data?.locked ? 'flight-start' : 'UNLOCKED-FLIGHT-START');
        }
        return super.postMessage(data, ...rest);
      }
    };
  });
  await page.goto(`${base}/mission.html`);
  await page.mouse.move(4, 4);
  await page.waitForFunction(() => window.missionDiagnostics?.wasm?.() && !window.missionDiagnostics.pending, null, { timeout: 60000 });
  await page.waitForFunction(() => missionDiagnostics.profile().ratingSource === 'server', null, { timeout: 30000 });
  await page.click('#open-profile');
  await page.fill('#profile-callsign', 'Orbit-Pilot');
  await page.click('#profile-callsign-form button[type=submit]');
  await page.waitForFunction(() => document.getElementById('profile-callsign-status').textContent.includes('registered'));
  assert.equal(await page.evaluate(() => missionDiagnostics.profile().callsign), 'ORBIT-PILOT');
  await page.click('#profile-close');

  await page.click('#example');
  await page.waitForFunction(() => !missionDiagnostics.pending, null, { timeout: 120000 });
  // Signed FDRs distinguish -0 from +0. Exercise an actual browser input so
  // receipt attachment, exported JSON, verification and cache must retain it.
  await page.fill('#node-n', '-0');
  assert.equal(await page.evaluate(() => Object.is(missionDiagnostics.nodes[0].dv[1], -0)), true);
  await page.click('#prefill');
  assert.equal(await page.locator('#attempt-mode').inputValue(), 'ranked');
  // Include *all* flight workers, not just locked ones: an unlocked numerical
  // preview would reveal the answer and invalidate the ordering assertion.
  await page.waitForTimeout(700);
  assert.deepEqual(await page.evaluate(() => authorityEvents), [], 'no modeled flight or numerical targeting before commit');
  assert.equal(await page.evaluate(() => missionDiagnostics.report), null);
  await page.click('#execute');
  await page.waitForFunction(() => !document.getElementById('debrief').hidden && !missionDiagnostics.pending, null, { timeout: 180000 });
  assert.match(await page.locator('#certification-status').textContent(), /CERTIFIED SERVER-RANKED RECEIPT/,
    `launch: ${await page.locator('#execute-status').textContent()} / ${await page.locator('#elo-result').textContent()}`);
  const first = await page.evaluate(() => ({ record: missionDiagnostics.recorder(), profile: missionDiagnostics.profile(), events: authorityEvents }));
  assert.equal(first.record.document.attempt_mode, 'Ranked');
  assert.equal(first.record.document.server_receipt.attempt_num, 1);
  assert.equal(first.record.document.server_receipt.fdr_hash, first.record.document.canonical_digest);
  assert.notEqual(first.profile.elo, 1200);
  // Settle the automatic post-receipt refresh so the deliberately delayed
  // GET below is the only player request being controlled by this test.
  await page.evaluate(async () => (await import('./profile.js')).syncProfile());
  // A player GET can have been issued before the server accepted this
  // receipt. Deliver its older snapshot afterwards; it must not roll back
  // the newly verified rating or label the stale value as current server data.
  const staleRating = await page.evaluate(async ({ text, receipt }) => {
    const identity = await import('./identity.js'), profile = await import('./profile.js');
    const nativeFetch = window.fetch;
    let release;
    window.fetch = () => new Promise(resolve => { release = resolve; });
    try {
      const sync = profile.syncProfile();
      const saved = identity.recordReceipt(text);
      release(new Response(JSON.stringify({ pubkey: receipt.pubkey, elo: receipt.elo_before,
        ranked_count: 0, callsign: null }), { status: 200, headers: { 'content-type': 'application/json' } }));
      const oldSync = await sync;
      return { saved, oldSync, elo: identity.profile().elo, source: identity.profile().ratingSource };
    } finally { window.fetch = nativeFetch; }
  }, { text: first.record.json, receipt: first.record.document.server_receipt });
  assert.deepEqual(staleRating, { saved: true, oldSync: null, elo: first.profile.elo, source: 'server' },
    'a stale player response must not overwrite a later receipt');
  const syncRace = await page.evaluate(async receipt => {
    const identity = await import('./identity.js'), profile = await import('./profile.js');
    const nativeFetch = window.fetch, pending = [];
    window.fetch = () => new Promise(resolve => pending.push(resolve));
    const response = elo => new Response(JSON.stringify({ pubkey: receipt.pubkey, elo,
      ranked_count: 1, callsign: null }), { status: 200, headers: { 'content-type': 'application/json' } });
    try {
      const older = profile.syncProfile(), newer = profile.syncProfile();
      pending[1](response(receipt.elo_after));
      const newerResult = await newer;
      pending[0](response(receipt.elo_before));
      const olderResult = await older;
      return { newerResult, olderResult, elo: identity.profile().elo, source: identity.profile().ratingSource };
    } finally { window.fetch = nativeFetch; }
  }, first.record.document.server_receipt);
  assert.deepEqual(syncRace, { newerResult: true, olderResult: null, elo: first.profile.elo, source: 'server' },
    'a slower earlier sync must not overwrite a newer one');
  const at = event => first.events.indexOf(event);
  assert.ok(at('commit-request') >= 0 && at('commit-request') < at('commit-response'));
  assert.ok(at('commit-response') < at('flight-start'), first.events.join(' > '));
  assert.ok(at('flight-start') < at('flight-result'));
  assert.ok(at('flight-result') < at('submit-request') && at('submit-request') < at('submit-response'), first.events.join(' > '));
  assert.deepEqual(first.events, ['commit-request', 'commit-response', 'flight-start', 'flight-result', 'submit-request', 'submit-response']);
  console.log('PASS ranked browser order:', first.events.join(' > '));

  // The native helper replays the actual browser FDR to build checkpoints.
  // Both transports must return the exact original receipt for the chain,
  // and reject rewritten metadata even after all hash links are recomputed.
  const fullRequest = JSON.parse(submitBodies.at(-1));
  const chain = JSON.parse(execFileSync(resolve('target/debug/examples/segment_chain'), [first.record.payload], { encoding: 'utf8' }));
  const postSegments = segments => fetch(`${base}/api/v1/submit-segment`, { method: 'POST', headers: { 'content-type':'application/json' }, body: JSON.stringify({ ...fullRequest, segments }) });
  const validChain = await postSegments(chain);
  assert.equal(validChain.status, 200, `valid segment replay: ${await validChain.clone().text()}`);
  assert.deepEqual((await validChain.json()).receipt, first.record.document.server_receipt);
  const missingChain = await postSegments(null);
  assert.equal(missingChain.status, 400);
  assert.equal((await missingChain.json()).code, 'SegmentMismatch');
  const rehash = checkpoints => {
    let prior = Buffer.from('odderon-segment-chain-v1');
    for (const cp of checkpoints) {
      const data = Buffer.alloc(8 + 7 * 8 + 8 + 2 * 8);
      data.writeBigInt64BE(BigInt(cp.t_us)); let offset = 8;
      for (const n of [...cp.state_vector, cp.prop_kg, ...cp.tank_temps]) { data.writeDoubleBE(n, offset); offset += 8; }
      prior = createHash('sha256').update(prior).update(data).digest();
      cp.cum_hash = prior.toString('hex');
    }
  };
  for (const field of ['propellant', 'temperature']) {
    const forged = structuredClone(chain);
    if (field === 'propellant') forged[1].prop_kg += 1;
    else forged[1].tank_temps[0] = 25;
    rehash(forged);
    const rejected = await postSegments(forged);
    assert.equal(rejected.status, 400, `${field} was accepted after rehashing`);
    assert.equal((await rejected.json()).code, 'SegmentMismatch');
  }
  console.log('PASS full/segmented replay parity and rehashed checkpoint forgery rejection');

  const download = page.waitForEvent('download');
  await page.click('#export-fdr');
  const saved = JSON.parse(await readFile(await (await download).path(), 'utf8'));
  assert.deepEqual(saved.server_receipt, first.record.document.server_receipt);
  assert.equal(Object.is(saved.command_timeline[0].delta_v_tnw[1], -0), true, 'export must retain the signed negative zero');
  assert.equal(await page.evaluate(() => missionDiagnostics.isRanked(101)), true, 'the valid ranked recorder must be cached');
  await page.click('#verify-flight');
  await page.waitForFunction(() => missionDiagnostics.verification()?.code === 1, null, { timeout: 180000 });
  assert.match(await page.locator('#verify-certification').textContent(), /CERTIFIED SERVER-RANKED/);
  const verified = await page.evaluate(() => missionDiagnostics.verification());
  assert.match(verified.dossier, /id="annex-d"/);
  assert.ok(verified.dossier.includes(first.record.document.server_receipt.server_sig));
  assert.equal(Object.is(JSON.parse(verified.json).command_timeline[0].delta_v_tnw[1], -0), true,
    'the verification worker must also export the original signed zero');
  const verifiedDownload = page.waitForEvent('download');
  await page.click('#verify-download-json');
  const verifiedText = await readFile(await (await verifiedDownload).path(), 'utf8');
  await page.evaluate(text => missionDiagnostics.openVerify(text), verifiedText);
  await page.waitForFunction(() => missionDiagnostics.verification()?.code === 1, null, { timeout: 180000 });
  assert.match(await page.locator('#verify-certification').textContent(), /CERTIFIED SERVER-RANKED/,
    'a verifier-exported recorder must still verify and retain its receipt');
  // A separate browser context acts as a phone/recruiter with no pilot
  // storage. Independently decode the printed QR and follow the public link;
  // certification must come back from the pinned-key-checked receipt.
  const qr = decodeQrFromSvg(verified.dossier);
  assert.equal(qr, `${base}/mission.html#flight=${first.record.document.canonical_digest}`);
  const scanner = await browser.newPage({ viewport: { width: 1000, height: 800 } });
  const fetched = [];
  scanner.on('request', request => { if (request.url().includes('/api/v1/verify/')) fetched.push(request.url()); });
  await scanner.goto(qr);
  await scanner.waitForFunction(() => window.missionDiagnostics?.verification()?.code === 1
    && document.getElementById('verify-certification')?.textContent.includes('SERVER-RANKED'), null, { timeout: 180000 });
  assert.ok(fetched.some(url => url.endsWith(first.record.document.canonical_digest)), 'QR must fetch the public FDR and receipt');
  assert.match((await scanner.evaluate(() => missionDiagnostics.verification())).dossier, /id="annex-d"/);
  await scanner.close();
  await page.click('#verify-close');
  console.log('PASS ranked dossier QR independently decodes and certifies in a fresh browser');

  // Complete old work *after* a newer invalid input. A verification verdict
  // belongs to one document; neither a stale worker nor a delayed public
  // fetch may replace the result for the newer document.
  const verificationRaces = await page.evaluate(async ({ text, payload, receipt }) => {
    const verify = await import('./verify.js');
    const NativeWorker = window.Worker, nativeFetch = window.fetch;
    const workers = [];
    window.Worker = class {
      constructor() { this.terminated = false; workers.push(this); }
      postMessage() {}
      terminate() { this.terminated = true; }
    };
    try {
      verify.openVerify(text);
      const old = workers.at(-1);
      verify.verifyText('{invalid');
      old.onmessage({ data: { verification: { code: 1, verdict: 'STALE VERIFIED', certification: '★ STALE', json: '', dossier: null } } });
      const workerRace = { code: verify.result()?.code, terminated: old.terminated };

      verify.openVerify(text);
      const edited = workers.at(-1);
      const input = document.getElementById('verify-input');
      input.value = '{edited';
      input.dispatchEvent(new Event('input', { bubbles: true }));
      edited.onmessage({ data: { verification: { code: 1, verdict: 'STALE VERIFIED', certification: '★ STALE', json: '', dossier: null } } });
      const editRace = { code: verify.result()?.code ?? null, terminated: edited.terminated,
        verdict: document.getElementById('verify-verdict').textContent };

      let releaseFetch;
      window.fetch = () => new Promise(resolve => { releaseFetch = resolve; });
      const fetching = verify.openPublicFlight(receipt.fdr_hash);
      const count = workers.length;
      verify.verifyText('{newer-invalid');
      releaseFetch(new Response(JSON.stringify({ fdr_payload: payload, receipt }),
        { status: 200, headers: { 'content-type': 'application/json' } }));
      await fetching;
      const fetchRace = { code: verify.result()?.code, startedWorkers: workers.length - count };

      const closedFetch = verify.openPublicFlight(receipt.fdr_hash);
      const beforeReopen = workers.length;
      document.getElementById('verify-dialog').close();
      verify.openVerify(); // close events are delivered asynchronously
      releaseFetch(new Response(JSON.stringify({ fdr_payload: payload, receipt }),
        { status: 200, headers: { 'content-type': 'application/json' } }));
      await closedFetch;
      const reopenRace = { code: verify.result()?.code ?? null, startedWorkers: workers.length - beforeReopen,
        busy: !document.getElementById('verify-busy').hidden };

      let releaseFile;
      const fileInput = document.getElementById('verify-import');
      Object.defineProperty(fileInput, 'files', { configurable: true,
        value: [{ text: () => new Promise(resolve => { releaseFile = resolve; }) }] });
      fileInput.dispatchEvent(new Event('change'));
      verify.verifyText('{newer-than-file');
      const beforeFile = workers.length;
      releaseFile(text);
      await Promise.resolve();
      await Promise.resolve();
      const fileRace = { code: verify.result()?.code, startedWorkers: workers.length - beforeFile,
        input: document.getElementById('verify-input').value };
      delete fileInput.files;

      verify.verifyFields([], 'test');
      const fieldsInput = document.getElementById('verify-input').value;
      return { workerRace, editRace, fetchRace, reopenRace, fileRace, fieldsInput };
    } finally {
      window.Worker = NativeWorker;
      window.fetch = nativeFetch;
      document.getElementById('verify-dialog').close();
    }
  }, { text: first.record.json, payload: first.record.payload, receipt: first.record.document.server_receipt });
  assert.deepEqual(verificationRaces.workerRace, { code: 0, terminated: true }, 'an invalid newer input cancels the old replay');
  assert.deepEqual(verificationRaces.editRace, { code: null, terminated: true, verdict: '' }, 'editing input clears and cancels its old verdict');
  assert.deepEqual(verificationRaces.fetchRace, { code: 0, startedWorkers: 0 }, 'an old public fetch cannot replace newer input');
  assert.deepEqual(verificationRaces.reopenRace, { code: null, startedWorkers: 0, busy: false }, 'closing and reopening cancels an old fetch');
  assert.deepEqual(verificationRaces.fileRace, { code: 0, startedWorkers: 0, input: '{newer-than-file' }, 'an old file read cannot replace newer input');
  assert.equal(verificationRaces.fieldsInput, '', 'a field-based replay must not display an unrelated document');
  console.log('PASS stale replay workers, file reads and public fetches cannot overwrite newer verification');

  // A queued close event from the old dialog must not cancel the refresh of
  // an immediately reopened leaderboard.
  const leaderboardRace = await page.evaluate(async () => {
    const leaderboard = await import('./leaderboard.js');
    const nativeFetch = window.fetch, pending = [];
    window.fetch = () => new Promise(resolve => pending.push(resolve));
    let dialog;
    try {
      leaderboard.openLeaderboard();
      dialog = document.getElementById('leaderboard-dialog');
      const closed = new Promise(resolve => dialog.addEventListener('close', resolve, { once: true }));
      dialog.close();
      leaderboard.openLeaderboard();
      await closed;
      for (const resolve of pending) resolve(new Response('[]', { status: 200, headers: { 'content-type': 'application/json' } }));
      const status = document.getElementById('leaderboard-status');
      return await new Promise(resolve => {
        const observer = new MutationObserver(() => {
          if (!status.textContent.startsWith('Loading')) {
            clearTimeout(timer); observer.disconnect(); resolve(status.textContent);
          }
        });
        const timer = setTimeout(() => { observer.disconnect(); resolve(status.textContent); }, 5000);
        observer.observe(status, { childList: true, characterData: true, subtree: true });
        if (!status.textContent.startsWith('Loading')) {
          clearTimeout(timer); observer.disconnect(); resolve(status.textContent);
        }
      });
    } finally {
      window.fetch = nativeFetch;
      if (dialog?.open) dialog.close();
    }
  });
  assert.match(leaderboardRace, /^No ranked flights on page 1/, 'an old close event must not cancel the new refresh');
  console.log('PASS rapid leaderboard reopen retains its new refresh');

  await page.click('#open-leaderboard');
  await page.waitForSelector('.leaderboard-replay', { timeout: 30000 });
  assert.match(await page.locator('#leaderboard-rows').textContent(), /ORBIT-PILOT/);
  await page.click('.leaderboard-replay');
  await page.waitForFunction(() => missionDiagnostics.verification()?.code === 1 && document.getElementById('verify-certification').textContent.includes('SERVER-RANKED'), null, { timeout: 180000 });
  assert.equal(await page.locator('#verify-dialog').isVisible(), true);
  await page.click('#verify-close');
  console.log('PASS live leaderboard opens a public, independently verified replay');

  // A valid FDR with an edited server rating still replays physically, but
  // may never display a certified banner or a certified dossier annex.
  const tampered = structuredClone(saved);
  tampered.server_receipt.elo_after = 2999;
  await page.evaluate(async doc => missionDiagnostics.openVerify((await import('./recorder.js')).stringifySigned(doc)), tampered);
  await page.waitForFunction(() => missionDiagnostics.verification() !== null, null, { timeout: 180000 });
  const tamperedVerdict = await page.evaluate(() => missionDiagnostics.verification());
  assert.equal(tamperedVerdict.code, 1, JSON.stringify(tamperedVerdict));
  assert.equal(await page.locator('#verify-certification').textContent(), 'UNVERIFIED LOCAL / PRACTICE');
  assert.match((await page.evaluate(() => missionDiagnostics.verification())).dossier, /UNVERIFIED LOCAL \/ PRACTICE/);
  await page.click('#verify-close');

  // localStorage cannot manufacture a certified rating; when the authority
  // is unavailable on reload, the forged cache is discarded. On sync the
  // authority's actual rating is restored.
  await page.evaluate(async doc => {
    const key = 'odderon.flightLicense.v1', s = JSON.parse(localStorage.getItem(key));
    s.elo = 2999;
    s.ranked['101'].elo = 2999;
    s.ranked['101'].recorder = (await import('./recorder.js')).stringifySigned(doc);
    localStorage.setItem(key, JSON.stringify(s));
  }, tampered);
  const blockPlayer = route => route.abort();
  await page.route('**/api/v1/player/**', blockPlayer);
  await page.reload();
  await page.waitForFunction(() => missionDiagnostics?.wasm?.() && !missionDiagnostics.pending, null, { timeout: 60000 });
  assert.notEqual(await page.evaluate(() => missionDiagnostics.profile().elo), 2999);
  assert.equal(await page.evaluate(() => missionDiagnostics.isRanked(101)), false);
  assert.equal(await page.locator('#topbar-elo').textContent(), '—');
  await page.unroute('**/api/v1/player/**', blockPlayer);
  await page.click('#open-profile');
  await page.click('#profile-sync');
  await page.waitForFunction(() => missionDiagnostics.profile().ratingSource === 'server');
  assert.equal(await page.evaluate(() => missionDiagnostics.profile().elo), first.profile.elo);
  await page.click('#profile-close');
  console.log('PASS oracle_devtools_localstorage_tamper_defeated (browser + real authority sync)');

  // A second commit must be forced to Practice by the server even after the
  // local cache was wiped; it cannot move the rating or enter the board.
  await page.click('#example');
  await page.waitForFunction(() => !missionDiagnostics.pending, null, { timeout: 120000 });
  await page.click('#prefill');
  await page.click('#execute');
  await page.waitForFunction(() => !document.getElementById('debrief').hidden && !missionDiagnostics.pending, null, { timeout: 180000 });
  assert.equal(await page.locator('#certification-status').textContent(), 'SERVER-VERIFIED PRACTICE');
  const second = await page.evaluate(() => ({ record: missionDiagnostics.recorder(), profile: missionDiagnostics.profile() }));
  assert.equal(second.record.document.attempt_mode, 'Practice');
  assert.equal(second.record.document.server_receipt.attempt_num, 2);
  assert.equal(second.profile.elo, first.profile.elo);
  await page.click('#open-leaderboard');
  await page.waitForSelector('.leaderboard-replay');
  assert.equal(await page.locator('.leaderboard-replay').count(), 1);
  await page.click('#leaderboard-close');
  console.log('PASS later committed flight forced to server-verified practice, not ranked');

  // Identical later practice FDRs have the same content hash even though
  // their server receipts have different attempt numbers. Retrying the
  // earlier ticket must still return its original receipt.
  const secondBody = submitBodies.at(-1);
  await page.click('#unlock'); await page.click('#example'); await page.click('#prefill'); await page.click('#execute');
  await page.waitForFunction(() => !document.getElementById('debrief').hidden && !missionDiagnostics.pending, null, { timeout: 180000 });
  const third = await page.evaluate(() => missionDiagnostics.recorder().document);
  assert.equal(third.server_receipt.attempt_num, 3);
  assert.equal(third.canonical_digest, second.record.document.canonical_digest, 'identical practice flights share their content address');
  const oldRetry = await fetch(`${base}/api/v1/submit`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: secondBody });
  assert.equal(oldRetry.status, 200, `old practice retry: ${await oldRetry.clone().text()}`);
  assert.deepEqual((await oldRetry.json()).receipt, second.record.document.server_receipt);

  // A fresh practice ticket may carry exactly the same signed bytes as a
  // bare base64url string instead of a #verify= fragment. Both adapters must
  // compare canonical records, not falsely call their spelling a collision.
  const freshCommit = await fetch(`${base}/api/v1/commit`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: commitBodies.at(-1) });
  assert.equal(freshCommit.status, 200, await freshCommit.clone().text());
  const freshTicket = await freshCommit.json();
  assert.equal(freshTicket.attempt_num, 4);
  const variant = JSON.parse(submitBodies.at(-1));
  variant.ticket = freshTicket;
  variant.fdr_payload = variant.fdr_payload.replace(/^#verify=/, '');
  const sameFlight = await fetch(`${base}/api/v1/submit`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(variant) });
  assert.equal(sameFlight.status, 200, `equivalent payload: ${await sameFlight.clone().text()}`);
  const variantReceipt = (await sameFlight.json()).receipt;
  assert.equal(variantReceipt.fdr_hash, third.canonical_digest);
  assert.equal(variantReceipt.attempt_num, 4);
  const stillRetry = await fetch(`${base}/api/v1/submit`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: secondBody });
  assert.deepEqual((await stillRetry.json()).receipt, second.record.document.server_receipt);
  console.log('PASS equivalent practice encodings share a flight while retries keep their original receipts');

  // Treat this as the revision at which an earlier submission began. Later
  // ranked flights will update the same profile before its response arrives.
  const oldRequestRevision = await page.evaluate(async () => (await import('./identity.js')).profileRevision());

  // Exercise cross-target server replay for the remaining canonical contracts,
  // including a genuinely delegated cryogenic tug. No numerical preview runs
  // before these first ranked commitments.
  await page.click('#unlock'); await page.selectOption('#mission-select', '102');
  await page.click('#example'); await page.click('#prefill'); await page.click('#execute');
  await page.waitForFunction(() => !document.getElementById('debrief').hidden && !missionDiagnostics.pending, null, { timeout: 180000 });
  assert.equal(await page.locator('#certification-status').textContent(), '★ CERTIFIED SERVER-RANKED RECEIPT');
  assert.equal(await page.evaluate(() => missionDiagnostics.recorder().document.server_receipt.contract_id), 102);
  await page.click('#unlock'); await page.selectOption('#mission-select', '103');
  await page.click('#delegate-1'); await page.click('#example');
  assert.equal(await page.locator('#split-kick').isDisabled(), true);
  assert.equal(await page.evaluate(() => missionDiagnostics.report), null);
  await page.fill('#node-t', '700'); await page.click('#prefill'); await page.click('#execute');
  await page.waitForFunction(() => !document.getElementById('debrief').hidden && !missionDiagnostics.pending, null, { timeout: 240000 });
  assert.equal(await page.locator('#certification-status').textContent(), '★ CERTIFIED SERVER-RANKED RECEIPT');
  const tug = await page.evaluate(() => ({ record: missionDiagnostics.recorder().document, boil: missionDiagnostics.boilOff }));
  assert.equal(tug.record.server_receipt.contract_id, 103);
  assert.equal(tug.record.delegation_mask, 2);
  assert.ok(tug.record.thermal_specs.length > 0 && tug.boil && !tug.boil.inert);
  const cryoRequest = { ...JSON.parse(submitBodies.at(-1)), segments: [] };
  const unsupported = await fetch(`${base}/api/v1/submit-segment`, { method: 'POST', headers: { 'content-type':'application/json' }, body: JSON.stringify(cryoRequest) });
  assert.equal(unsupported.status, 400);
  assert.equal((await unsupported.json()).code, 'ThermalSegmentsUnsupported', 'unsupported cryogenic checkpoint temperatures must fail closed');
  await page.click('#verify-flight');
  await page.waitForFunction(() => missionDiagnostics.verification()?.code === 1 && document.getElementById('verify-certification').textContent.includes('SERVER-RANKED'), null, { timeout: 180000 });
  await page.click('#verify-close');
  console.log('PASS contracts 102 and 103: server-ranked replay, including delegated cryogenic physics');

  // A delayed old receipt, an idempotent retry and a practice receipt are
  // authentic but historical. None may roll back a newer local rating, even
  // though the ranked certificate still needs to be retained.
  const receiptRace = await page.evaluate(async ({ ranked, practice, oldRevision }) => {
    const identity = await import('./identity.js');
    const before = identity.profile();
    const savedOld = identity.recordReceipt(ranked, { expectedRevision: oldRevision });
    const afterOld = identity.profile();
    const savedDuplicate = identity.recordReceipt(ranked, { expectedRevision: identity.profileRevision() });
    const afterDuplicate = identity.profile();
    const savedPractice = identity.recordReceipt(practice, { expectedRevision: identity.profileRevision() });
    const afterPractice = identity.profile();
    return { before: before.elo, afterOld: afterOld.elo, afterDuplicate: afterDuplicate.elo,
      afterPractice: afterPractice.elo, savedOld, savedDuplicate, savedPractice,
      certificate: !!identity.certificate(101) };
  }, { ranked: first.record.json, practice: second.record.json, oldRevision: oldRequestRevision });
  assert.ok(receiptRace.savedOld && receiptRace.savedDuplicate && receiptRace.savedPractice && receiptRace.certificate);
  assert.equal(receiptRace.afterOld, receiptRace.before, 'a delayed receipt must not roll back newer rating state');
  assert.equal(receiptRace.afterDuplicate, receiptRace.before, 'an already cached receipt is historical');
  assert.equal(receiptRace.afterPractice, receiptRace.before, 'practice cannot replace a newer ranked rating');
  console.log('PASS delayed, duplicate and practice receipts cannot roll back a newer rating');

  // Reproduce the same ordering through the actual UI: the server commits
  // contract 101, its response is held, then this pilot flies 102 before the
  // older response arrives. Both certificates must survive and the profile
  // must converge to the server's latest rating.
  const orderingPage = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
  let releaseOld, markAccepted, heldResponse;
  const holdOld = new Promise(resolve => { releaseOld = resolve; });
  const oldAccepted = new Promise(resolve => { markAccepted = resolve; });
  try {
    await orderingPage.goto(`${base}/mission.html`);
    await orderingPage.waitForFunction(() => missionDiagnostics?.wasm?.() && !missionDiagnostics.pending, null, { timeout: 60000 });
    await orderingPage.waitForFunction(() => missionDiagnostics.profile().ratingSource === 'server', null, { timeout: 30000 });
    await orderingPage.route('**/api/v1/submit', async route => {
      if (!heldResponse) {
        const response = await route.fetch();
        heldResponse = await response.json();
        markAccepted();
        await holdOld;
        await route.fulfill({ response });
      } else await route.continue();
    });
    await orderingPage.click('#example'); await orderingPage.click('#prefill'); await orderingPage.click('#execute');
    let timer;
    try {
      await Promise.race([oldAccepted, new Promise((_, reject) => { timer = setTimeout(() => reject(Error('old receipt was not accepted')), 120000); })]);
    } finally { clearTimeout(timer); }
    assert.equal(heldResponse.receipt.contract_id, 101);
    await orderingPage.click('#unlock'); await orderingPage.selectOption('#mission-select', '102');
    await orderingPage.click('#example'); await orderingPage.click('#prefill'); await orderingPage.click('#execute');
    await orderingPage.waitForFunction(() => missionDiagnostics.recorder()?.document?.server_receipt?.contract_id === 102,
      null, { timeout: 180000 });
    const current = await orderingPage.evaluate(async () => {
      const key = missionDiagnostics.recorder().document.public_key;
      return (await fetch(`/api/v1/player/${key}`)).json();
    });
    assert.equal(current.ranked_count, 2);
    assert.notEqual(current.elo, heldResponse.receipt.elo_after);
    await orderingPage.waitForFunction(elo => missionDiagnostics.profile().elo === elo, current.elo, { timeout: 30000 });
    releaseOld();
    await orderingPage.waitForFunction(() => missionDiagnostics.isRanked(101) && !missionDiagnostics.pending, null, { timeout: 60000 });
    const final = await orderingPage.evaluate(() => ({ profile: missionDiagnostics.profile(), second: missionDiagnostics.isRanked(102) }));
    assert.equal(final.profile.elo, current.elo, 'a late first response must not replace the latest server rating');
    assert.equal(final.second, true);
  } finally {
    releaseOld();
    await orderingPage.close();
  }
  console.log('PASS overlapping real ranked submissions preserve both certificates and the latest rating');

  // A separate pilot's very first tug is manually fitted with a powered
  // cooler. This exercises active cooling across browser WASM and both real
  // server runtimes, rather than only a practice or passive cryogenic flight.
  const cooled = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
  await cooled.goto(`${base}/mission.html`);
  await cooled.waitForFunction(() => missionDiagnostics?.wasm?.() && !missionDiagnostics.pending, null, { timeout: 60000 });
  assert.equal(await cooled.locator('#thermal-apply-card').isDisabled(), true, 'storable contracts have no cryogenic card');
  await cooled.selectOption('#mission-select', '103');
  await cooled.waitForFunction(() => !missionDiagnostics.pending && missionDiagnostics.assembly?.cryogenic, null, { timeout: 60000 });
  const plainMass = await cooled.evaluate(() => missionDiagnostics.assembly.dryMass);
  await cooled.click('#thermal-apply-card');
  await cooled.waitForFunction(() => !missionDiagnostics.pending && missionDiagnostics.assembly?.coolerLift === 2000 && missionDiagnostics.assembly?.arrayWatts === 12000);
  const card = await cooled.evaluate(() => missionDiagnostics.assembly);
  assert.ok(card.dryMass > plainMass, 'the recommended hardware must be charged as dry mass');
  assert.equal(card.mliLayers, 40);
  assert.equal(card.strutConductance, 0.15);
  await cooled.click('#example'); await cooled.fill('#node-t', '700'); await cooled.click('#prefill'); await cooled.click('#execute');
  await cooled.waitForFunction(() => !document.getElementById('debrief').hidden && !missionDiagnostics.pending, null, { timeout: 240000 });
  assert.equal(await cooled.locator('#certification-status').textContent(), '★ CERTIFIED SERVER-RANKED RECEIPT');
  const active = await cooled.evaluate(() => ({ record: missionDiagnostics.recorder().document, boil: missionDiagnostics.boilOff }));
  assert.equal(active.record.server_receipt.attempt_num, 1);
  assert.equal(active.record.server_receipt.contract_id, 103);
  assert.equal(active.record.delegation_mask, 0);
  assert.equal(active.record.thermal_specs[0].cryocooler_w, 2000);
  assert.equal(active.record.thermal_specs[0].array_w_at_1au, 12000);
  assert.equal(active.record.thermal_specs[0].epsilon_eff, card.epsilon, 'the recommended emittance must survive the worker snapshot');
  assert.ok(active.boil && !active.boil.inert && active.boil.coolerJoules > 0, JSON.stringify(active.boil));
  assert.ok(active.record.cryocooler_energy_j > 0);
  await cooled.click('#verify-flight');
  await cooled.waitForFunction(() => missionDiagnostics.verification()?.code === 1
    && document.getElementById('verify-certification').textContent.includes('SERVER-RANKED'), null, { timeout: 240000 });
  const activeDossier = await cooled.evaluate(() => missionDiagnostics.verification().dossier);
  assert.match(activeDossier, /id="annex-c"/);
  assert.match(activeDossier, /id="annex-d"/);
  await cooled.click('#verify-close');
  console.log('PASS paid recommended cryogenic card reaches first-attempt ranked flight and verified dossier');

  // Numerical targeting is a practice-only teaching aid. Prove in the real
  // browser that it targets the assembled stage rather than the baseline:
  // first the paid manual card, then all four delegated chiefs. Both actual
  // trajectories must reach the catalog objective, not just have Δv margin.
  // These local flights are transport-independent, so run them once here.
  if (!cloud) {
    for (const [name, mask, total] of [['manual card', 0, 3240], ['fully delegated', 15, 3252]]) {
      await cooled.click('#unlock');
      if (mask) for (let i = 0; i < 4; i++) await cooled.click(`#delegate-${i}`);
      await cooled.selectOption('#attempt-mode', 'practice');
      assert.equal(await cooled.evaluate(() => missionDiagnostics.assembly.mask), mask);
      await cooled.click('#example');
      await cooled.fill('#node-t', String(total));
      await cooled.click('#split-kick');
      await cooled.waitForFunction(() => missionDiagnostics.nodes.length === 3 && !missionDiagnostics.pending,
        null, { timeout: 240000 });
      await cooled.click('#prefill'); await cooled.click('#execute');
      await cooled.waitForFunction(() => !document.getElementById('debrief').hidden && !missionDiagnostics.pending,
        null, { timeout: 240000 });
      const flown = await cooled.evaluate(() => ({ report: missionDiagnostics.report, record: missionDiagnostics.recorder().document }));
      assert.equal(flown.record.delegation_mask, mask, name);
      assert.equal(flown.report[30], 0, name);
      assert.ok(flown.report[35] >= 0.8, `${name} objective ${flown.report[35]}`);
      assert.ok(Math.abs(flown.report[1] + 6378137 - 380000000) <= 15000000, name);
    }
    console.log('PASS practice targeter flies manual and fully delegated assembled tugs to the objective');
  }
  await cooled.close();

  // A failed commit must not start physics. If the submit response is lost
  // after the server has applied it, retry the *same* ticket and payload and
  // prove that Elo and the ranking move exactly once.
  const retryPage = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
  await retryPage.addInitScript(() => {
    window.flightStarts = 0;
    const Native = window.Worker;
    window.Worker = class extends Native {
      constructor(...args) { super(...args); this.isMission = String(args[0]).endsWith('mission-worker.js'); }
      postMessage(data, ...rest) { if (this.isMission && !data?.action) window.flightStarts++; return super.postMessage(data, ...rest); }
    };
  });
  await retryPage.goto(`${base}/mission.html`);
  await retryPage.waitForFunction(() => missionDiagnostics?.wasm?.() && !missionDiagnostics.pending, null, { timeout: 60000 });
  await retryPage.click('#example'); await retryPage.click('#prefill');
  const rejectCommit = route => route.abort();
  await retryPage.route('**/api/v1/commit', rejectCommit);
  await retryPage.click('#execute');
  await retryPage.waitForFunction(() => document.getElementById('execute-status').textContent.includes('Ranked launch stopped') && !missionDiagnostics.pending, null, { timeout: 30000 });
  assert.equal(await retryPage.evaluate(() => flightStarts), 0, 'commit failure must not run even an unlocked worker');
  assert.equal(await retryPage.locator('#debrief').isHidden(), true);
  await retryPage.unroute('**/api/v1/commit', rejectCommit);
  await retryPage.click('#unlock');
  let submitted = 0, lostResponse = null;
  await retryPage.route('**/api/v1/submit', async route => {
    if (++submitted === 1) {
      const response = await route.fetch();
      lostResponse = await response.json();
      await route.abort();
    } else await route.continue();
  });
  await retryPage.click('#prefill'); await retryPage.click('#execute');
  await retryPage.waitForSelector('#retry-submit', { timeout: 180000 });
  assert.equal(lostResponse?.accepted, true, 'server actually accepted the response that the network lost');
  assert.equal(await retryPage.locator('#certification-status').textContent(), 'UNVERIFIED LOCAL / PRACTICE');
  // Model an intervening player snapshot whose Elo happens to equal the old
  // receipt's before-value (ratings can revisit a value). If a GET is then
  // unavailable, the retry must not treat the old receipt as a new update.
  await retryPage.route('**/api/v1/player/**', blockPlayer);
  await retryPage.evaluate(async receipt => {
    const identity = await import('./identity.js');
    identity.syncPlayer({ pubkey: receipt.pubkey, elo: receipt.elo_before, ranked_count: 2, callsign: null });
  }, lostResponse.receipt);
  await retryPage.click('#retry-submit');
  await retryPage.waitForFunction(() => document.getElementById('certification-status').textContent.includes('CERTIFIED SERVER-RANKED') && !missionDiagnostics.pending, null, { timeout: 180000 });
  assert.equal(await retryPage.evaluate(() => missionDiagnostics.profile().elo), lostResponse.receipt.elo_before,
    'an old retry must use the revision captured before its first submission');
  await retryPage.unroute('**/api/v1/player/**', blockPlayer);
  await retryPage.evaluate(async () => (await import('./profile.js')).syncProfile());
  const retry = await retryPage.evaluate(async () => {
    const record = missionDiagnostics.recorder().document;
    const player = await (await fetch(`/api/v1/player/${record.public_key}`)).json();
    const board = await (await fetch('/api/v1/leaderboard?contract=101&page=1')).json();
    return { record, player, board };
  });
  assert.equal(submitted, 2);
  assert.deepEqual(retry.record.server_receipt, lostResponse.receipt, 'retry returns the original signed receipt');
  assert.equal(retry.player.ranked_count, 1);
  assert.equal(retry.player.elo, retry.record.server_receipt.elo_after);
  assert.equal(retry.board.filter(row => row.pubkey === retry.record.public_key).length, 1);
  await retryPage.close();
  console.log('PASS failed commit runs no physics; lost submit response retries without a second rating');

  // An unconfigured/offline deployment stays usable, but can only produce a
  // local practice record. Even direct unlocked worker messages are rejected.
  const offline = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
  const offlinePosts = [];
  offline.on('request', request => { if (request.method() === 'POST' && request.url().includes('/api/v1/')) offlinePosts.push(request.url()); });
  await offline.route('**/authority-config.js', route => route.fulfill({ status: 200, contentType: 'text/javascript', body: "export const SERVER_PUBLIC_KEY = '';" }));
  await offline.route('**/api/v1/**', route => route.abort());
  await offline.goto(`${base}/mission.html`);
  await offline.waitForFunction(() => missionDiagnostics?.wasm?.() && !missionDiagnostics.pending, null, { timeout: 60000 });
  assert.equal(await offline.locator('#attempt-mode').inputValue(), 'practice');
  assert.equal(await offline.locator('#attempt-mode option[value=ranked]').isDisabled(), true);
  const rejection = await offline.evaluate(() => new Promise(resolve => {
    const worker = new Worker('./mission-worker.js', { type: 'module' });
    worker.onmessage = event => { worker.terminate(); resolve(event.data.error); };
    worker.postMessage({ contract: 101, nodes: [] });
  }));
  assert.match(rejection, /Lock a prediction/);
  await offline.click('#example'); await offline.click('#prefill'); await offline.click('#execute');
  await offline.waitForFunction(() => !document.getElementById('debrief').hidden && !missionDiagnostics.pending, null, { timeout: 180000 });
  const local = await offline.evaluate(() => ({ record: missionDiagnostics.recorder().document, profile: missionDiagnostics.profile() }));
  assert.equal(local.record.attempt_mode, 'Practice');
  assert.equal(local.record.server_receipt, undefined);
  assert.equal(local.profile.elo, 1200);
  assert.equal(await offline.locator('#certification-status').textContent(), 'UNVERIFIED LOCAL / PRACTICE');
  assert.deepEqual(offlinePosts, []);
  await offline.close();
  console.log('PASS offline practice works without certification; unlocked flight workers are rejected');
  assert.deepEqual(errors, []);
} finally {
  if (browser) await browser.close();
  await new Promise(r => proxy.close(r));
  await authority.close();
}
