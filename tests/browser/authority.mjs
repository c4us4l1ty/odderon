// Flown browser/HTTP/SQLite authority oracle. The test key below is PUBLIC
// and for tests only; production must pin a different, privately held seed.
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { resolve, extname } from 'node:path';
import { chromium } from 'playwright-core';
import bundledChromium from '@sparticuz/chromium';
import { startAuthority } from './support/authority-server.mjs';

const authority = await startAuthority();
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
  const errors = [];
  page.on('pageerror', error => errors.push(String(error)));
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
        this.lockedFlight = false;
        this.addEventListener('message', () => { if (this.lockedFlight) window.authorityEvents.push('locked-flight-result'); });
      }
      postMessage(data, ...rest) {
        if (data?.locked) { this.lockedFlight = true; window.authorityEvents.push('locked-flight-start'); }
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
  assert.ok(at('commit-response') < at('locked-flight-start'), first.events.join(' > '));
  assert.ok(at('locked-flight-start') < at('locked-flight-result'));
  assert.ok(at('locked-flight-result') < at('submit-request') && at('submit-request') < at('submit-response'), first.events.join(' > '));
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
  assert.deepEqual(errors, []);
} finally {
  if (browser) await browser.close();
  await new Promise(r => proxy.close(r));
  await authority.close();
}
