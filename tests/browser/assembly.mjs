// End-to-end assembly audit: does delegation actually reach a flown,
// signed flight, and is cryogenic boil-off visible to the player?
//
// This drives the real page (clicks the checkboxes, moves the sliders) and
// then inspects the executed flight and its certificate. It is the browser
// counterpart of odd-verify/tests/delegation_flown.rs.
import { chromium } from 'playwright-core';
import bundledChromium from '@sparticuz/chromium';
import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { resolve, extname } from 'node:path';

const root = resolve('web');
const types = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.wasm': 'application/wasm' };
const server = createServer(async (req, res) => {
  const path = resolve(root, '.' + new URL(req.url, 'http://test').pathname.replace(/\/$/, '/index.html'));
  if (!path.startsWith(root + '/')) { res.writeHead(403).end(); return; }
  try { const body = await readFile(path); res.setHeader('content-type', types[extname(path)] || 'application/octet-stream'); res.end(body); }
  catch { res.writeHead(404).end(); }
});
await new Promise(r => server.listen(0, '127.0.0.1', r));
const base = `http://127.0.0.1:${server.address().port}`;

const failures = [];
function check(name, ok, detail = '') {
  console.log(`${ok ? 'PASS' : 'FAIL'} ${name}${detail ? ` — ${detail}` : ''}`);
  if (!ok) failures.push(name);
}

const C = bundledChromium.default ?? bundledChromium;
const browser = await chromium.launch({
  executablePath: process.env.CHROME_BIN || (await C.executablePath()),
  headless: true,
  args: ['--no-sandbox', '--disable-dev-shm-usage', '--no-proxy-server'],
});
const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
const errors = [];
page.on('pageerror', e => errors.push(String(e)));
await page.goto(`${base}/mission.html`, { waitUntil: 'load' });
await page.mouse.move(4, 4);
await page.waitForFunction(() => window.missionDiagnostics?.wasm?.());
await page.waitForFunction(() => !window.missionDiagnostics.pending);

// --- 1. The assembly panel exists and reports canonical numbers. ---
const manual = await page.evaluate(() => window.missionDiagnostics.assembly);
check('assembly panel is present', manual !== null, JSON.stringify(manual));
check('starts fully manual', manual.mask === 0 && manual.margin === 0 && manual.cost === 1);

// --- 2. Clicking the chief checkboxes changes real mass and cost. ---
for (const bit of [0, 1, 2, 3]) {
  await page.click(`#delegate-${bit}`);
  await page.waitForFunction(() => !window.missionDiagnostics.pending);
}
const delegated = await page.evaluate(() => window.missionDiagnostics.assembly);
console.log('delegated assembly', JSON.stringify(delegated));
check('all four chiefs engaged', delegated.mask === 0b1111);
check('margin is real dry mass', delegated.margin > 0 && delegated.dryMass > manual.dryMass,
  `+${(delegated.dryMass - manual.dryMass).toFixed(3)} kg`);
check('cost multiplier is 1.30', Math.abs(delegated.cost - 1.30) < 1e-12);
check('efficiency cap is 0.65', Math.abs(delegated.cap - 0.65) < 1e-12);

// --- 3. Badges name the chief and the kilograms. ---
const badges = await page.locator('#delegation-badges').innerText();
console.log('badges:\n' + badges);
check('badges rendered', badges.length > 40);
check('badges name kilograms', /kg/.test(badges));
const violations = await page.evaluate(() => window.missionDiagnostics.acronymViolations);
check('no naked acronyms in the new UI', violations.length === 0, violations.join(','));

// --- 4. The cryogenic design is a lever, and it changes the hardware. ---
const beforeCryo = await page.evaluate(() => window.missionDiagnostics.assembly);
await page.fill('#thermal-strut', '2.5');
await page.dispatchEvent('#thermal-strut', 'change');
await page.waitForFunction(() => !window.missionDiagnostics.pending);
const afterCryo = await page.evaluate(() => window.missionDiagnostics.assembly);
check('strut conductance is settable', Math.abs(afterCryo.strutConductance - 2.5) < 1e-9);
check('a warmer mount changes the chief-sized mass', afterCryo.dryMass !== beforeCryo.dryMass,
  `${beforeCryo.dryMass.toFixed(3)} -> ${afterCryo.dryMass.toFixed(3)} kg`);

// A cooler without a radiator must be refused, not silently accepted.
await page.fill('#thermal-cooler', '400');
await page.dispatchEvent('#thermal-cooler', 'change');
const refused = await page.evaluate(() => window.missionDiagnostics.assembly.coolerLift);
check('cooler without radiator is refused', refused === 0);
await page.fill('#thermal-radiator', '40');
await page.dispatchEvent('#thermal-radiator', 'change');
await page.fill('#thermal-cooler', '400');
await page.dispatchEvent('#thermal-cooler', 'change');
await page.waitForFunction(() => !window.missionDiagnostics.pending);
const cooled = await page.evaluate(() => window.missionDiagnostics.assembly);
check('cooler accepted once a radiator exists', Math.abs(cooled.coolerLift - 400) < 1e-9);

// --- 5. Fly the delegated stage and read the executed outcome. ---
await page.click('#example');
await page.waitForFunction(() => !window.missionDiagnostics.pending);
await page.click('#prefill');
await page.click('#execute');
await page.waitForFunction(() => !window.missionDiagnostics.pending && window.missionDiagnostics.report, null, { timeout: 120000 });
const flown = await page.evaluate(() => ({
  report: window.missionDiagnostics.report,
  boilOff: window.missionDiagnostics.boilOff,
  debrief: document.getElementById('debrief-content').innerText,
  assembly: window.missionDiagnostics.assembly,
}));
console.log('flown score', flown.report[4], 'efficiency', flown.report[36]);
console.log('boil-off', JSON.stringify(flown.boilOff));
check('a delegated flight completes', Number.isFinite(flown.report[4]));
check('the flown efficiency is capped', flown.report[36] <= 0.65 + 1e-12, String(flown.report[36]));
check('the debrief no longer claims boil-off is unmodeled', !/Not modeled/i.test(flown.debrief));
check('the debrief reports boil-off', /boiled off|vented/i.test(flown.debrief));
check('the debrief reports delegation', /delegated/i.test(flown.debrief));
check('boil-off readout is live', flown.boilOff !== null);

// --- 6. The signed record carries the mask, and it verifies. ---
const rec = await page.evaluate(() => {
  const r = window.missionDiagnostics.recorder();
  return r ? { json: r.json ?? null, fields: r.fields ? Array.from(r.fields) : null } : null;
});
check('recorder captured', rec !== null);
if (rec?.json) {
  const parsed = JSON.parse(rec.json);
  check('signed schema is v3', parsed.schema_version === 3, String(parsed.schema_version));
  check('signed mask is the flown mask', parsed.delegation_mask === 0b1111, String(parsed.delegation_mask));
}

check('no page errors', errors.length === 0, errors.join(' | '));
await browser.close();
server.close();
console.log(failures.length ? `\nFAILED: ${failures.join(', ')}` : '\nALL ASSEMBLY CHECKS PASSED');
process.exit(failures.length ? 1 : 0);
