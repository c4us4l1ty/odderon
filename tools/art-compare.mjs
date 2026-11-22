#!/usr/bin/env node
// art-compare.mjs — put the reference frame and the current render side by side
// and print the numbers that actually drive the art direction.
//
//   node tools/art-compare.mjs                       # every view in tools/views.json
//   node tools/art-compare.mjs --only 04-street-machiya
//   node tools/art-compare.mjs --ref .artifacts/reference.png --sheet-only
//
// What it measures (all on a 640×360 downsample, so it is about *value
// structure*, not pixel-peeping):
//   mean / p5 / p25 / p50 / p75 / p95 luminance, % pixels below 0.15 (shade),
//   % above 0.85 (hot), mean saturation, and the share of pixels whose hue sits
//   in the warm (wood/sun) or cool (sky shadow) family.
//
// Output: .artifacts/compare/<view>.png — reference strip on top, render
// underneath, plus a small report.json.

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { ensureChromium, CHROMIUM_ARGS } from './ensure-chromium.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(HERE, '..');
const arg = (n, d) => { const i = process.argv.indexOf(`--${n}`); return i === -1 ? d : process.argv[i + 1]; };

const SHOTS = path.resolve(ROOT, arg('out', '.artifacts/screenshots'));
const OUT = path.resolve(ROOT, arg('cmp-out', '.artifacts/compare'));
const REF = path.resolve(ROOT, arg('ref', '.artifacts/reference-summer-noon.png'));
const ONLY = (arg('only', '') || '').split(',').filter(Boolean);

if (!fs.existsSync(SHOTS)) {
  console.error(`No screenshots in ${SHOTS}. Run: node tools/screenshot.mjs`);
  process.exit(1);
}
const files = fs.readdirSync(SHOTS).filter((f) => f.endsWith('.png') && (!ONLY.length || ONLY.some((o) => f.includes(o)))).sort();
if (!files.length) { console.error('No matching screenshots.'); process.exit(1); }

const b64 = (p) => 'data:image/png;base64,' + fs.readFileSync(p).toString('base64');
const refUrl = fs.existsSync(REF) ? b64(REF) : null;
const shots = files.map((f) => ({ name: f.replace(/\.png$/, ''), url: b64(path.join(SHOTS, f)) }));

fs.mkdirSync(OUT, { recursive: true });

const { chromium } = await import('playwright-core');
const { executablePath, env } = await ensureChromium();
const browser = await chromium.launch({ executablePath, args: CHROMIUM_ARGS, env: { ...process.env, ...env } });
const page = await browser.newPage({ viewport: { width: 1400, height: 900 }, deviceScaleFactor: 1 });

// All images are inlined as data URLs, so the canvas is never tainted and
// getImageData() works without any file:// flags.
await page.setContent('<html><body style="margin:0;background:#05070a"><canvas id="c"></canvas></body></html>');

const stats = await page.evaluate(async ({ refUrl, shots }) => {
  const load = (src) => new Promise((res, rej) => { const i = new Image(); i.onload = () => res(i); i.onerror = rej; i.src = src; });
  const W = 640, H = 360;
  const scratch = document.createElement('canvas');
  scratch.width = W; scratch.height = H;
  const sctx = scratch.getContext('2d', { willReadFrequently: true });
  const measure = (img) => {
    sctx.drawImage(img, 0, 0, W, H);
    const d = sctx.getImageData(0, 0, W, H).data;
    const luma = new Float32Array(W * H);
    let satSum = 0, warm = 0, cool = 0;
    for (let i = 0, p = 0; i < d.length; i += 4, p++) {
      const r = d[i] / 255, g = d[i + 1] / 255, b = d[i + 2] / 255;
      const l = 0.2126 * r + 0.7152 * g + 0.0722 * b;
      luma[p] = l;
      const mx = Math.max(r, g, b), mn = Math.min(r, g, b);
      satSum += mx > 0.002 ? (mx - mn) / mx : 0;
      // crude hue family: warm = r dominant, cool = b dominant
      if (l > 0.02) { if (r > b * 1.06) warm++; else if (b > r * 1.08) cool++; }
    }
    const sorted = Float32Array.from(luma).sort();
    const q = (f) => sorted[Math.min(sorted.length - 1, Math.max(0, Math.floor(f * sorted.length)))];
    let dark = 0, hot = 0;
    for (let p = 0; p < luma.length; p++) { if (luma[p] < 0.15) dark++; if (luma[p] > 0.85) hot++; }
    const n = luma.length;
    return {
      mean: +q(0.5).toFixed(3) * 0 + +(Array.prototype.reduce.call(luma, (a, b) => a + b, 0) / n).toFixed(3),
      p5: +q(0.05).toFixed(3), p25: +q(0.25).toFixed(3), p50: +q(0.5).toFixed(3),
      p75: +q(0.75).toFixed(3), p95: +q(0.95).toFixed(3),
      darkPct: +(100 * dark / n).toFixed(0), hotPct: +(100 * hot / n).toFixed(0),
      sat: +(satSum / n).toFixed(3),
      warmPct: +(100 * warm / n).toFixed(0), coolPct: +(100 * cool / n).toFixed(0),
    };
  };

  const ref = refUrl ? measure(await load(refUrl)) : null;
  const results = [];
  for (const s of shots) results.push({ name: s.name, stats: measure(await load(s.url)) });

  // ---- contact sheet: reference strip over each render -------------------
  const sheets = {};
  const cellW = 640, cellH = 360, pad = 8, labelH = 18;
  for (const s of shots) {
    const img = await load(s.url);
    const refImg = refUrl ? await load(refUrl) : null;
    const rows = refImg ? 2 : 1;
    const c = document.createElement('canvas');
    c.width = cellW + pad * 2; c.height = labelH * rows + cellH * rows + pad * (rows + 1);
    const ctx = c.getContext('2d');
    ctx.fillStyle = '#05070a'; ctx.fillRect(0, 0, c.width, c.height);
    ctx.font = '12px ui-monospace, monospace';
    let y = pad;
    if (refImg) {
      ctx.fillStyle = '#7dd3fc'; ctx.fillText('REFERENCE — "Shadows of a Summer Noon"', pad, y + 12);
      ctx.drawImage(refImg, pad, y + labelH, cellW, cellH); y += labelH + cellH + pad;
    }
    ctx.fillStyle = '#fcd34d'; ctx.fillText('ODDERON — ' + s.name, pad, y + 12);
    ctx.drawImage(img, pad, y + labelH, cellW, cellH);
    sheets[s.name] = c.toDataURL('image/png');
  }
  return { ref, results, sheets };
}, { refUrl, shots });

const fmt = (s) => `mean=${String(s.mean).padEnd(5)} p5=${String(s.p5).padEnd(5)} p25=${String(s.p25).padEnd(5)} ` +
  `p50=${String(s.p50).padEnd(5)} p75=${String(s.p75).padEnd(5)} p95=${String(s.p95).padEnd(5)} ` +
  `dark=${String(s.darkPct + '%').padEnd(4)} hot=${String(s.hotPct + '%').padEnd(4)} sat=${String(s.sat).padEnd(5)} ` +
  `warm=${String(s.warmPct + '%').padEnd(4)} cool=${s.coolPct}%`;

console.log('\nART DIRECTION REPORT (640×360 luminance statistics)\n');
if (stats.ref) console.log('  REF   ' + fmt(stats.ref));
for (const r of stats.results) console.log('  ' + r.name.slice(0, 5).padEnd(5) + ' ' + fmt(r.stats));

for (const [name, dataUrl] of Object.entries(stats.sheets)) {
  const buf = Buffer.from(dataUrl.split(',')[1], 'base64');
  fs.writeFileSync(path.join(OUT, `${name}.png`), buf);
}
fs.writeFileSync(path.join(OUT, 'report.json'), JSON.stringify({ reference: stats.ref, views: stats.results }, null, 2));
console.log(`\n  sheets -> ${path.relative(ROOT, OUT)}/\n`);

await browser.close();
