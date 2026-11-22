#!/usr/bin/env node
// screenshot.mjs — deterministic headless screenshots of the Odderon presenter.
//
// Why this exists: art direction that cannot be looked at is art direction that
// drifts. This renders the *shipped* WebGL2 path (same shaders, same wasm, same
// draw calls) in software, from a pose encoded in the URL, so two commits can be
// compared pixel for pixel by any agent or human without a GPU.
//
//   node tools/screenshot.mjs                        # all views -> .artifacts/screenshots
//   node tools/screenshot.mjs --only street,sky      # substring subset
//   node tools/screenshot.mjs --out /tmp/shots --width 1600 --height 900
//
// Views live in tools/views.json. A view is a camera pose plus optional overrides:
//   { "name": "...", "cam": [x,y,z,yawDeg,pitchDeg], "sun": [x,y,z], "warmth": 0.7 }
// Everything is served by the repo's own `server.mjs` (port $PORT, default 3000),
// started automatically if nothing is listening.

import fs from 'node:fs';
import path from 'node:path';
import net from 'node:net';
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { ensureChromium, CHROMIUM_ARGS } from './ensure-chromium.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(HERE, '..');

function arg(name, dflt) {
  const i = process.argv.indexOf(`--${name}`);
  return i === -1 ? dflt : process.argv[i + 1];
}

const PORT = Number(process.env.PORT || 3000);
const OUT = path.resolve(ROOT, arg('out', '.artifacts/screenshots'));
const WIDTH = Number(arg('width', 1280));
const HEIGHT = Number(arg('height', 720));
const ONLY = (arg('only', '') || '').split(',').filter(Boolean);
const VIEWS_FILE = path.resolve(ROOT, arg('views', 'tools/views.json'));
const SETTLE = Number(arg('settle', 1800));

function portOpen(port) {
  return new Promise((resolve) => {
    const s = net.connect({ port, host: '127.0.0.1' });
    s.on('connect', () => { s.destroy(); resolve(true); });
    s.on('error', () => resolve(false));
    setTimeout(() => { s.destroy(); resolve(false); }, 800);
  });
}

async function ensureServer() {
  if (await portOpen(PORT)) return null;
  const p = spawn(process.execPath, [path.join(ROOT, 'server.mjs')], {
    cwd: ROOT, stdio: 'ignore', env: { ...process.env, PORT: String(PORT) },
  });
  for (let i = 0; i < 60; i++) {
    if (await portOpen(PORT)) return p;
    await new Promise((r) => setTimeout(r, 250));
  }
  throw new Error('server.mjs did not start on port ' + PORT);
}

function viewUrl(v) {
  const q = new URLSearchParams({ capture: '1', hud: '0' });
  if (v.cam) q.set('cam', v.cam.join(','));
  if (v.sun) q.set('sun', v.sun.join(','));
  if (v.warmth !== undefined) q.set('warmth', String(v.warmth));
  return `http://127.0.0.1:${PORT}/?${q.toString()}`;
}

const { chromium } = await import('playwright-core');
const allViews = JSON.parse(fs.readFileSync(VIEWS_FILE, 'utf8')).views;
// `--only` takes substrings, as documented. A token that matches nothing is an
// error, not a no-op: silently rendering zero views used to overwrite a good
// report.json with an empty one and still exit 0.
const views = ONLY.length
  ? allViews.filter((v) => ONLY.some((o) => v.name.includes(o)))
  : allViews;
if (ONLY.length) {
  const matched = new Set(views.map((v) => v.name));
  const missing = ONLY.filter((o) => !views.some((v) => v.name.includes(o)));
  if (missing.length) {
    console.error(`--only matched no view for: ${missing.join(', ')}`);
    console.error(`available: ${allViews.map((v) => v.name).join(', ')}`);
    process.exit(1);
  }
}
if (!views.length) {
  console.error(`No views in ${VIEWS_FILE}`);
  process.exit(1);
}

fs.mkdirSync(OUT, { recursive: true });
const server = await ensureServer();
const { executablePath, env } = await ensureChromium();

const browser = await chromium.launch({
  executablePath,
  args: [...CHROMIUM_ARGS, `--window-size=${WIDTH},${HEIGHT}`],
  env: { ...process.env, ...env },
});

const report = [];
try {
  const page = await browser.newPage({ viewport: { width: WIDTH, height: HEIGHT }, deviceScaleFactor: 1 });
  const errors = [];
  page.on('pageerror', (e) => errors.push(e.message));
  page.on('console', (m) => { if (m.type() === 'error') errors.push(m.text()); });

  for (const v of views) {
    const t0 = Date.now();
    await page.goto(viewUrl(v), { waitUntil: 'load' });
    await page.waitForFunction(() => globalThis.__odderonCapture !== undefined, null, { timeout: 20000 });
    await page.waitForTimeout(v.settle ?? SETTLE);
    // Park the presenter first: with SwiftShader a single 720p frame can take ~1 s, so a
    // running loop starves the screenshot request on the main thread.
    await page.evaluate(() => globalThis.__odderonCapture.freeze());
    await page.waitForTimeout(400);
    const file = path.join(OUT, `${v.name}.png`);
    await page.screenshot({ path: file, timeout: 120000 });
    const frame = await page.evaluate(() => globalThis.__odderonCapture.frame());
    report.push({ name: v.name, file, frame, ms: Date.now() - t0 });
    console.log(`  ${v.name.padEnd(22)} ${String(frame.px[0]).padStart(4)}x${String(frame.px[1])}px ` +
      `${frame.draws} draws  ${frame.ms.toFixed(2)} ms/frame  -> ${path.relative(ROOT, file)}`);
  }
  if (errors.length) {
    console.log('\npage errors:\n  ' + [...new Set(errors)].slice(0, 6).join('\n  '));
    // A shader that fails to compile still produces a PNG - a black one - so a
    // silent exit 0 here would let a broken build look like a finished capture.
    process.exitCode = 1;
  }
} finally {
  await browser.close();
  if (server) server.kill();
}

if (!report.length) {
  console.error('nothing captured; leaving the previous report.json alone');
  process.exit(1);
}
fs.writeFileSync(path.join(OUT, 'report.json'), JSON.stringify(report, null, 2));
