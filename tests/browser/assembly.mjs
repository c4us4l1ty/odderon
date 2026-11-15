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

// --- 1b. The propellant ordering is read from Rust, not restated in JS. ---
const propellants = await page.evaluate(() => window.missionDiagnostics.engine.propellants);
const options = await page.evaluate(() => [...document.getElementById('engine-propellant').options]
  .map(o => ({ value: Number(o.value), label: o.textContent })));
console.log('propellant table', JSON.stringify(propellants));
check('the selector is built from the solver', propellants.length === 4 && options.length === 4);
check('selector values are the signed wire ids',
  propellants.every((p, i) => p.id === i && options[i].value === p.id));
check('selector labels are the solver labels',
  propellants.every((p, i) => options[i].label === p.label));
check('the calibration bands came from Rust',
  propellants.every(p => Number.isFinite(p.band[0]) && p.band[1] > p.band[0]));
// Solving each id must return the same propellant the label promises.
const bandAgreement = await page.evaluate(() => {
  const w = window.missionDiagnostics.wasm();
  const out = [];
  for (let i = 0; i < w.mission_propellant_count(); i++) {
    const lo = w.mission_propellant_band(i, 0), hi = w.mission_propellant_band(i, 1);
    // A mixture ratio just outside the band must be refused by the solver.
    const code = w.mission_engine_solve(i, lo - 0.05, 3.5e6, 0.0288, 0.3155, 0.8, 0);
    out.push({ id: i, lo, hi, code });
  }
  return out;
});
check('the solver enforces the band it advertises',
  bandAgreement.every(r => r.code !== 1), JSON.stringify(bandAgreement));

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

// --- 4. A storable stage has no cryogenic fit-out, and says so. ---
check('contract 101 is storable', manual.cryogenic === false);
check('cryo knobs are disabled on a storable stage',
  await page.locator('#thermal-mli').isDisabled());
check('the panel explains why', /storable/i.test(await page.locator('#thermal-status').innerText()));

// --- 5. Fly the delegated stage and read the executed outcome. ---
await page.click('#example');
await page.waitForFunction(() => !window.missionDiagnostics.pending);
await page.click('#prefill');
// Prefill can run a worker (the tug targets real periapses), so wait for the
// fields to be populated before locking.
await page.waitForFunction(() => document.getElementById('predict-dv').value !== ''
  && !window.missionDiagnostics.pending, null, { timeout: 120000 });
await page.click('#execute');
await page.waitForFunction(() => !window.missionDiagnostics.pending && window.missionDiagnostics.report
  && !document.getElementById('debrief').hidden, null, { timeout: 120000 });
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
check('the debrief reports a boil-off row', /Boil-off loss/i.test(flown.debrief));
check('storable boil-off is reported as zero', /storable propellants/i.test(flown.debrief));
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

// --- 6b. The dossier's scan target must be an absolute, scannable URL. ---
await page.click('#verify-flight');
await page.waitForFunction(() => {
  const v = window.missionDiagnostics.verification();
  return v && v.code === 1 && v.dossier;
}, null, { timeout: 180000 });
const verified = await page.evaluate(() => window.missionDiagnostics.verification());
const scanLine = verified.dossier.match(/Scan at:\s*<code>([^<]*)<\/code>/)?.[0] ?? '';
const scanUrl = verified.dossier.match(/Scan at:\s*<code>([^<]*)<\/code>/)?.[1] ?? '';
console.log('scan line:', scanLine.slice(0, 140));
check('the flight verifies end to end', verified.code === 1, verified.verdict);
check('the scan target is an absolute URL', /^https?:\/\//.test(scanUrl), scanUrl.slice(0, 80));
check('the scan target carries the verify fragment', scanUrl.includes('#verify='));
check('the scan target points at this page', scanUrl.startsWith(`${new URL(page.url()).origin}/mission.html#verify=`));
check('the dossier no longer prints a relative placeholder', !/relative to the serving origin/i.test(scanLine));

// --- 7. The cryogenic tug: the design lever must change mass AND loss. ---
await page.evaluate(() => document.getElementById('verify-dialog').close());
// The contract selector is locked during a flight; unlock the plan first.
await page.click('#unlock');
await page.selectOption('#mission-select', '103');
await page.waitForFunction(() => !window.missionDiagnostics.pending);
const tugManual = await page.evaluate(() => window.missionDiagnostics.assembly);
check('the tug is cryogenic', tugManual.cryogenic === true);
check('cryo knobs are live on the tug', !(await page.locator('#thermal-mli').isDisabled()));
check('the tug starts hand-sized again', tugManual.mask === 0);

// A cooler without a radiator must be refused, not silently accepted.
await page.fill('#thermal-cooler', '400');
await page.dispatchEvent('#thermal-cooler', 'change');
check('cooler without radiator is refused',
  (await page.evaluate(() => window.missionDiagnostics.assembly.coolerLift)) === 0);

// Fit real cooling: 40 layers, 40 m² of radiator, 400 W of lift.
await page.fill('#thermal-mli', '40');
await page.dispatchEvent('#thermal-mli', 'change');
await page.fill('#thermal-radiator', '40');
await page.dispatchEvent('#thermal-radiator', 'change');
await page.fill('#thermal-cooler', '400');
await page.dispatchEvent('#thermal-cooler', 'change');
await page.waitForFunction(() => !window.missionDiagnostics.pending);
const tugCooled = await page.evaluate(() => window.missionDiagnostics.assembly);
console.log('cooled tug', JSON.stringify(tugCooled));
check('the cooler is fitted', Math.abs(tugCooled.coolerLift - 400) < 1e-9);
check('the fit-out costs real dry mass', tugCooled.fitoutMass > 100 && tugCooled.dryMass > tugManual.dryMass,
  `+${tugCooled.fitoutMass.toFixed(3)} kg`);

// The single-burn example is the hyperbolic trap, so use the guided kicks:
// three periapse burns targeted by the same worker the player uses.
await page.click('#example');
await page.waitForFunction(() => !window.missionDiagnostics.pending);
await page.click('#split-kick');
await page.waitForFunction(() => !window.missionDiagnostics.pending
  && window.missionDiagnostics.nodes.length === 3, null, { timeout: 180000 });
await page.click('#prefill');
await page.waitForFunction(() => document.getElementById('predict-dv').value !== ''
  && !window.missionDiagnostics.pending, null, { timeout: 120000 });
await page.click('#execute');
try {
  await page.waitForFunction(() => !window.missionDiagnostics.pending && window.missionDiagnostics.report
    && !document.getElementById('debrief').hidden && window.missionDiagnostics.boilOff
    && !window.missionDiagnostics.boilOff.inert, null, { timeout: 180000 });
} catch (e) {
  console.log('TUG FLIGHT STATE', JSON.stringify(await page.evaluate(() => ({
    pending: window.missionDiagnostics.pending,
    report: window.missionDiagnostics.report?.slice(0, 6) ?? null,
    boilOff: window.missionDiagnostics.boilOff,
    hidden: document.getElementById('debrief').hidden,
    execute: document.getElementById('execute-status').textContent,
    plan: document.getElementById('plan-status').textContent,
    locked: window.missionDiagnostics.locked,
  }))));
  throw e;
}
const cryoFlight = await page.evaluate(() => ({
  boilOff: window.missionDiagnostics.boilOff,
  debrief: document.getElementById('debrief-content').innerText,
}));
console.log('cooled tug flight', JSON.stringify(cryoFlight.boilOff));
check('the cryogenic flight reports a thermal balance', cryoFlight.boilOff !== null && !cryoFlight.boilOff.inert);
// The baseline 3 kW array is saturated by the oxidizer, so hydrogen still
// boils. Size the array up and the same hardware must lose less.
check('active cooling reduces boil-off', cryoFlight.boilOff.lost < 521.0, `${cryoFlight.boilOff.lost.toFixed(3)} kg`);
check('the debrief shows the cooler electricity', /Cryocooler electricity/i.test(cryoFlight.debrief));

await page.click('#unlock');
await page.fill('#thermal-array', '16000');
await page.dispatchEvent('#thermal-array', 'change');
await page.waitForFunction(() => !window.missionDiagnostics.pending);
const powered = await page.evaluate(() => window.missionDiagnostics.assembly);
check('a bigger array costs mass', powered.dryMass > tugCooled.dryMass,
  `+${(powered.dryMass - tugCooled.dryMass).toFixed(3)} kg`);
// The redesign cleared the plan; the splitter only appears once a long burn
// exists, so seed the example again.
await page.click('#example');
await page.waitForFunction(() => !window.missionDiagnostics.pending);
await page.click('#split-kick');
await page.waitForFunction(() => !window.missionDiagnostics.pending
  && window.missionDiagnostics.nodes.length === 3, null, { timeout: 180000 });
await page.click('#prefill');
await page.waitForFunction(() => document.getElementById('predict-dv').value !== ''
  && !window.missionDiagnostics.pending, null, { timeout: 120000 });
await page.click('#execute');
await page.waitForFunction(() => !window.missionDiagnostics.pending
  && !document.getElementById('debrief').hidden && window.missionDiagnostics.boilOff
  && window.missionDiagnostics.assembly.arrayWatts === 16000, null, { timeout: 180000 });
const wellCooled = await page.evaluate(() => window.missionDiagnostics.boilOff);
console.log('16 kW array flight', JSON.stringify(wellCooled));
check('more cooling power means less boil-off', wellCooled.lost < cryoFlight.boilOff.lost,
  `${cryoFlight.boilOff.lost.toFixed(3)} -> ${wellCooled.lost.toFixed(3)} kg`);
check('the cooler draw is reported', cryoFlight.boilOff.coolerJoules > 0);

check('no page errors', errors.length === 0, errors.join(' | '));
await browser.close();
server.close();
console.log(failures.length ? `\nFAILED: ${failures.join(', ')}` : '\nALL ASSEMBLY CHECKS PASSED');
process.exit(failures.length ? 1 : 0);
