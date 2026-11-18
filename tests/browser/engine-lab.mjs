// Focused browser exercise for the Engine CAD & Schlieren lab. Runs against
// the built wasm and the real page, so it catches bridge/layout mistakes
// without paying for the full observatory suite.
import { chromium } from 'playwright-core';
import bundledChromium from '@sparticuz/chromium';
import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { resolve, extname } from 'node:path';
import { execFileSync } from 'node:child_process';
import assert from 'node:assert/strict';

const root = resolve('web');
const types = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.wasm': 'application/wasm' };
const server = createServer(async (req, res) => {
  const path = resolve(root, '.' + new URL(req.url, 'http://test').pathname.replace(/\/$/, '/index.html'));
  if (!path.startsWith(root + '/')) { res.writeHead(403).end(); return; }
  try { const body = await readFile(path); res.setHeader('content-type', types[extname(path)] || 'application/octet-stream'); res.end(body); }
  catch { res.writeHead(404).end(); }
});
await new Promise(r => server.listen(0, '127.0.0.1', r));

const C = bundledChromium.default ?? bundledChromium;
let browser;
try {
  const executablePath = process.env.CHROME_BIN || (await C.executablePath()) || execFileSync('which', ['google-chrome'], { encoding: 'utf8' }).trim();
  browser = await chromium.launch({ executablePath, headless: true, args: ['--no-sandbox', '--disable-dev-shm-usage', '--no-proxy-server'] });
  const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
  const errors = [];
  page.on('pageerror', e => errors.push(String(e)));
  page.on('console', m => { if (m.type() === 'error') errors.push(m.text()); });
  await page.addInitScript(() => {
    const raf = window.requestAnimationFrame.bind(window);
    window.testFrames = 0;
    window.requestAnimationFrame = fn => raf(t => { window.testFrames++; fn(t); });
    window.testMessages = 0;
    const NativeWorker = window.Worker;
    window.Worker = class extends NativeWorker {
      constructor(...a) { super(...a); this.addEventListener('message', () => window.testMessages++); }
      postMessage(...a) { window.testMessages++; return super.postMessage(...a); }
    };
  });
  await page.goto(`http://127.0.0.1:${server.address().port}/mission.html`);
  await page.waitForFunction(() => window.missionDiagnostics?.engine?.solution, null, { timeout: 60000 });
  assert.deepEqual(errors, [], 'page errors');

  // The flight deck shows one station at a time, so the Propulsion workspace
  // must be opened before its canvas has a layout box to draw into.
  await page.locator('[data-flight-mode="propulsion"]').click();
  await page.waitForFunction(() => {
    const box = document.getElementById('engine-canvas').getBoundingClientRect();
    return box.width > 0 && box.height > 0 && missionDiagnostics.engine.raster;
  }, null, { timeout: 30000 });

  const initial = await page.evaluate(() => missionDiagnostics.engine);
  console.log('baseline', JSON.stringify({ code: initial.code, thrust: initial.solution.thrust, isp: initial.solution.isp, regime: initial.plume.regime, stations: initial.stations, raster: initial.raster }));
  assert.equal(initial.code, 1);
  assert.equal(initial.stations, 96);
  assert.equal(initial.raster, true);
  assert.ok(initial.solution.thrust > 2000 && initial.solution.thrust < 3000);

  await page.locator('#engine-canvas').scrollIntoViewIfNeeded();
  const before = await page.evaluate(() => missionDiagnostics.engine);
  const box = await page.locator('#engine-canvas').boundingBox();
  const exitHandle = before.handles.find(h => h.key === 'exit');
  assert.ok(exitHandle, 'exit handle must exist');
  await page.mouse.move(box.x + exitHandle.at[0], box.y + exitHandle.at[1]);
  await page.mouse.down();
  await page.mouse.move(box.x + exitHandle.at[0] + 4, box.y + exitHandle.at[1] - 26, { steps: 6 });
  await page.mouse.up();
  const dragged = await page.evaluate(() => missionDiagnostics.engine);
  console.log('after exit drag', JSON.stringify({ exit: dragged.spec.exit, thrust: dragged.solution?.thrust, draws: dragged.draws, solves: dragged.solves }));
  assert.ok(dragged.spec.exit > before.spec.exit, 'dragging the exit handle must widen the bell');
  assert.ok(dragged.draws > before.draws, 'the canvas must repaint');

  // Mixture ratio: peak Isp is fuel-rich, not stoichiometric.
  await page.locator('#engine-propellant').selectOption('0');
  const sweep = [];
  for (const of of [4.5, 5.0, 5.5, 6.0, 7.94]) {
    await page.locator('#engine-of').fill(String(of));
    await page.locator('#engine-of').dispatchEvent('input');
    const d = await page.evaluate(() => missionDiagnostics.engine);
    sweep.push({ of, isp: d.solution?.isp, tc: d.solution?.tc, molar: d.solution?.molar });
  }
  console.log('mixture sweep', JSON.stringify(sweep));
  const best = sweep.reduce((a, b) => (b.isp > a.isp ? b : a));
  assert.ok(best.of <= 6.0, `peak Isp must be fuel-rich, got ${best.of}`);
  assert.ok(sweep.at(-1).tc > sweep[0].tc, 'stoichiometric must run hotter');
  assert.ok(sweep.at(-1).molar > sweep[0].molar, 'fuel-rich must be lighter');

  // Altitude sweep on a big bell: separation at sea level, attached high up.
  await page.locator('#engine-propellant').selectOption('1');
  await page.locator('#engine-of').fill('2.36');
  await page.locator('#engine-of').dispatchEvent('input');
  await page.locator('#engine-pc').fill('5');
  await page.locator('#engine-throat').fill('50');
  await page.locator('#engine-exit').fill('447.2');
  const altitudes = [];
  for (const km of [0, 10, 35, 100]) {
    await page.locator('#engine-altitude').fill(String(km));
    await page.locator('#engine-altitude').dispatchEvent('input');
    const d = await page.evaluate(() => missionDiagnostics.engine);
    altitudes.push({ km, regime: d.plume?.regime, sep: d.solution?.separationX, isp: d.solution?.isp, cells: d.plume?.cellLength });
  }
  console.log('altitude sweep', JSON.stringify(altitudes));
  assert.equal(altitudes[0].regime, 3, 'sea level must separate');
  assert.ok(altitudes[0].sep > 0.1 && altitudes[0].sep < 0.9);
  assert.notEqual(altitudes[2].regime, 3, '35 km must flow full: the bell re-attaches');
  assert.ok(!Number.isFinite(altitudes[2].sep), 'no detachment line once attached');
  assert.equal(altitudes[3].regime, 0, '100 km is vacuum: adapted, no cells');
  assert.equal(altitudes[3].cells, 0);
  assert.ok(altitudes[3].isp > altitudes[0].isp, 'vacuum Isp must beat sea level');
  const causal = await page.locator('#engine-causal').textContent();
  console.log('engine causal at 100 km:', causal.slice(0, 160));

  await page.locator('#engine-altitude').fill('0');
  await page.locator('#engine-altitude').dispatchEvent('input');
  assert.match(await page.locator('#engine-causal').textContent(), /separates inside the bell/);

  // Apply a flyable design and check the stage picked up the derived numbers.
  await page.locator('#engine-reset').click();
  await page.waitForFunction(() => !missionDiagnostics.pending, null, { timeout: 60000 });
  await page.locator('#engine-apply').click();
  await page.waitForFunction(() => missionDiagnostics.engine?.solution, null, { timeout: 60000 });
  const applied = await page.evaluate(() => ({ lab: missionDiagnostics.engine, setup: window.missionDiagnostics.nodes }));
  console.log('applied stage', JSON.stringify({ wet: applied.lab.solution.stageWet, dry: applied.lab.solution.stageDry, propellant: applied.lab.solution.stagePropellant }));
  assert.ok(applied.lab.solution.stageWet > applied.lab.solution.stageDry);

  // Idle: two seconds with no input must schedule nothing.
  await page.waitForFunction(() => !missionDiagnostics.pending && !missionDiagnostics.engine.pending, null, { timeout: 120000 });
  await page.waitForTimeout(600);
  const quiet = await page.evaluate(() => ({ frames: testFrames, messages: testMessages, draws: missionDiagnostics.draws, labDraws: missionDiagnostics.engine.draws }));
  await page.waitForTimeout(2000);
  const after = await page.evaluate(() => ({ frames: testFrames, messages: testMessages, draws: missionDiagnostics.draws, labDraws: missionDiagnostics.engine.draws }));
  assert.deepEqual(after, quiet, 'the lab must settle to zero frames and zero worker messages');
  assert.deepEqual(errors, [], 'page errors at exit');
  console.log('Engine lab smoke test passed. Idle 2 s: 0 frames, 0 worker messages.');
} finally {
  await browser?.close();
  server.close();
}
