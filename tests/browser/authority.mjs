// Flown browser/HTTP/SQLite authority oracle. The test key below is PUBLIC
// and for tests only; production must pin a different, privately held seed.
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { resolve, extname } from 'node:path';
import { chromium } from 'playwright-core';
import bundledChromium from '@sparticuz/chromium';
import { startAuthority } from './support/authority-server.mjs';
import { startCloudAuthority } from './support/cloud-authority.mjs';

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
  const errors = [], submitBodies = [];
  page.on('pageerror', error => errors.push(String(error)));
  page.on('request', request => { if (request.method() === 'POST' && request.url().endsWith('/api/v1/submit')) submitBodies.push(request.postData()); });
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
  const at = event => first.events.indexOf(event);
  assert.ok(at('commit-request') >= 0 && at('commit-request') < at('commit-response'));
  assert.ok(at('commit-response') < at('flight-start'), first.events.join(' > '));
  assert.ok(at('flight-start') < at('flight-result'));
  assert.ok(at('flight-result') < at('submit-request') && at('submit-request') < at('submit-response'), first.events.join(' > '));
  assert.deepEqual(first.events, ['commit-request', 'commit-response', 'flight-start', 'flight-result', 'submit-request', 'submit-response']);
  console.log('PASS ranked browser order:', first.events.join(' > '));

  const download = page.waitForEvent('download');
  await page.click('#export-fdr');
  const saved = JSON.parse(await readFile(await (await download).path(), 'utf8'));
  assert.deepEqual(saved.server_receipt, first.record.document.server_receipt);
  await page.click('#verify-flight');
  await page.waitForFunction(() => missionDiagnostics.verification()?.code === 1, null, { timeout: 180000 });
  assert.match(await page.locator('#verify-certification').textContent(), /CERTIFIED SERVER-RANKED/);
  const verified = await page.evaluate(() => missionDiagnostics.verification());
  assert.match(verified.dossier, /id="annex-d"/);
  assert.ok(verified.dossier.includes(first.record.document.server_receipt.server_sig));
  await page.click('#verify-close');
  console.log('PASS receipt embedded in exported FDR and replayed dossier');

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
  await page.evaluate(doc => missionDiagnostics.openVerify(JSON.stringify(doc)), tampered);
  await page.waitForFunction(() => missionDiagnostics.verification() !== null, null, { timeout: 180000 });
  const tamperedVerdict = await page.evaluate(() => missionDiagnostics.verification());
  assert.equal(tamperedVerdict.code, 1, JSON.stringify(tamperedVerdict));
  assert.equal(await page.locator('#verify-certification').textContent(), 'UNVERIFIED LOCAL / PRACTICE');
  assert.match((await page.evaluate(() => missionDiagnostics.verification())).dossier, /UNVERIFIED LOCAL \/ PRACTICE/);
  await page.click('#verify-close');

  // localStorage cannot manufacture a certified rating; when the authority
  // is unavailable on reload, the forged cache is discarded. On sync the
  // authority's actual rating is restored.
  await page.evaluate(doc => {
    const key = 'odderon.flightLicense.v1', s = JSON.parse(localStorage.getItem(key));
    s.elo = 2999;
    s.ranked['101'].elo = 2999;
    s.ranked['101'].recorder = JSON.stringify(doc);
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
  console.log('PASS an older identical-practice retry keeps its original receipt');

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
  await page.click('#verify-flight');
  await page.waitForFunction(() => missionDiagnostics.verification()?.code === 1 && document.getElementById('verify-certification').textContent.includes('SERVER-RANKED'), null, { timeout: 180000 });
  await page.click('#verify-close');
  console.log('PASS contracts 102 and 103: server-ranked replay, including delegated cryogenic physics');

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
  await retryPage.click('#retry-submit');
  await retryPage.waitForFunction(() => document.getElementById('certification-status').textContent.includes('CERTIFIED SERVER-RANKED') && !missionDiagnostics.pending, null, { timeout: 180000 });
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
