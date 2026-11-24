#!/usr/bin/env node
/**
 * Serve `web/` and check that it boots, compiles its shaders, and reports a measurement.
 *
 * Why this exists: a WebGL shader that fails to compile produces a blank canvas and a console message
 * nobody reads. On the target hardware that looks identical to "the renderer is just slow", which is
 * the single most misleading failure mode this project could have — it would send someone optimising a
 * shader that never ran.
 *
 * What it CAN do here: verify the shader compiles, the page runs, the instrument produces numbers, and
 * render-on-demand actually stops drawing when idle.
 * What it CANNOT do: measure frame time. This sandbox is SwiftShader (a CPU rasteriser), so any timing
 * is meaningless. See tools/gl-info.mjs and docs/BUILD-PLAN.md §0 fact 3.
 *
 * Usage: node tools/check-shell.mjs
 */

import { createServer } from "node:http";
import { resolveStaticPath, DIST_ROOT } from "./static-path.mjs";
import { readFile, stat } from "node:fs/promises";
import { dirname, extname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, "..");

const MIME = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".mjs": "text/javascript; charset=utf-8",
  ".wasm": "application/wasm",
  ".json": "application/json",
  ".css": "text/css",
};

const server = createServer(async (req, res) => {
  try {
    // Same resolver as `tools/serve.mjs`. This file was already safe because it normalised with
    // `new URL()` before testing the `/dist/` prefix, but it was safe by accident -- the URL parser
    // was there for convenience, not containment -- so it was one refactor away from the bug that
    // actually landed in serve.mjs. Sharing the implementation is what stops that.
    const f = resolveStaticPath(req.url);
    if (f === null) throw new Error("forbidden");
    if (f === DIST_ROOT || f.startsWith(DIST_ROOT + "/")) {
      // wasm, when it exists
      const body = await readFile(f);
      res.writeHead(200, {
        "content-type": MIME[extname(f)] ?? "application/octet-stream",
        "access-control-allow-origin": "*",
      });
      res.end(body);
      return;
    }
    const s = await stat(f);
    if (!s.isFile()) throw new Error("not a file");
    const body = await readFile(f);
    res.writeHead(200, { "content-type": MIME[extname(f)] ?? "text/plain" });
    res.end(body);
  } catch {
    res.writeHead(404).end("not found");
  }
});

const port = 8931;
await new Promise((r) => server.listen(port, "0.0.0.0", r));

const { default: puppeteer } = await import("puppeteer-core");
const { default: chromium } = await import("@sparticuz/chromium");

const browser = await puppeteer.launch({
  args: [...chromium.args, "--enable-unsafe-swiftshader", "--use-gl=angle"],
  executablePath: await chromium.executablePath(),
  headless: true,
});

const page = await browser.newPage();
await page.setViewport({ width: 1280, height: 720 });

const fail0 = [];
const errors = [];
const logs = [];
page.on("console", (m) => logs.push(`[${m.type()}] ${m.text()}`));
page.on("pageerror", (e) => errors.push(String(e)));
page.on("requestfailed", (r) => errors.push(`request failed: ${r.url()} ${r.failure()?.errorText}`));

await page.goto(`http://127.0.0.1:${port}/`, { waitUntil: "networkidle0", timeout: 30000 });

// Wait for the boot burst to FINISH, rather than guessing how long it takes.
//
// The previous version slept a fixed 2.5 s and then reported "drew 3 frames while idle". That was not
// a renderer bug: the boot burst was still running, because on this sandbox's software rasteriser a
// fixed number of frames takes ten seconds where on a real GPU it takes under one. A test that hard-
// codes "how long settling takes" is measuring the machine's speed, not the renderer's behaviour.
async function settled(quietMs = 700, timeoutMs = 20000) {
  const t0 = Date.now();
  let lastCount = -1, lastChange = Date.now();
  while (Date.now() - t0 < timeoutMs) {
    const n = await page.evaluate(() => window.__odderon?.bench.framesDrawn ?? -1);
    if (n !== lastCount) {
      lastCount = n;
      lastChange = Date.now();
    }
    if (Date.now() - lastChange >= quietMs) return { settled: true, quietForMs: Date.now() - lastChange, frames: n };
    await new Promise((r) => setTimeout(r, 50));
  }
  return { settled: false, frames: lastCount };
}

const settle = await settled();
if (!settle.settled) fail0.push("never settled: frames kept being drawn for 20 s after load");

const report = await page.evaluate(() => {
  const o = window.__odderon;
  if (!o) return { fatal: "window.__odderon missing — the module did not run" };
  // The HUD's arrays were renamed when submit time and GPU time were separated. Read the GPU samples
  // when present, fall back to submit samples, and if neither exists say so rather than throwing a
  // TypeError from inside page.evaluate, which reports as a checker crash instead of a finding.
  const src = o.bench.gpuSamples?.length ? o.bench.gpuSamples
            : o.bench.submitSamples;
  if (!Array.isArray(src)) return { fatal: "bench arrays renamed again — update tools/check-shell.mjs" };
  const s = src.slice().sort((a, b) => a - b);
  const kind = o.bench.gpuSamples?.length ? "gpu" : "submit";
  const q = (p) => (s.length ? s[Math.min(s.length - 1, Math.floor(s.length * p))] : NaN);
  return {
    renderer: o.renderer(),
    framesDrawn: o.bench.framesDrawn,
    samples: s.length,
    kind,
    medianMs: q(0.5),
    p95Ms: q(0.95),
    canvas: [o.gl.drawingBufferWidth, o.gl.drawingBufferHeight],
    idle: o.state.needsFrame === false,
    glError: o.gl.getError(),
  };
});

// ---- render-on-demand: does it actually stop? ----
const before = await page.evaluate(() => window.__odderon?.bench.framesDrawn ?? 0);
await new Promise((r) => setTimeout(r, 1500));
const after = await page.evaluate(() => window.__odderon?.bench.framesDrawn ?? 0);
const drewWhileIdle = after - before;

// ---- is the image non-blank? A shader that compiles can still render black, which on the target
// hardware is indistinguishable from "slow". This samples from inside a draw, because
// preserveDrawingBuffer is false and an outside readback returns an empty buffer.
const pixels = await page.evaluate(async () => {
  const o = window.__odderon;
  if (!o) return { error: "window.__odderon missing" };
  const p = o.requestCapture(48);
  // Force exactly one frame through the same input path a user would use.
  const ev = new PointerEvent("pointermove", { clientX: 10, clientY: 10 });
  document.querySelector("canvas").dispatchEvent(ev);
  return await Promise.race([
    p,
    new Promise((r) => setTimeout(() => r({ error: "capture timed out" }), 4000)),
  ]);
});

await browser.close();
server.close();

const fail = [...fail0];
const pass = [];

if (report.fatal) fail.push(report.fatal);
if (errors.length) fail.push(...errors);
if (report.samples === 0) fail.push("no steady-state frame samples were recorded");
if (report.glError !== 0) fail.push(`GL error ${report.glError}`);
if (drewWhileIdle > 0) fail.push(`drew ${drewWhileIdle} frames while idle — render-on-demand is broken`);
else pass.push(`render-on-demand: 0 frames drawn while idle (settled after ${settle.quietForMs} ms quiet)`);
if (pixels.error) {
  fail.push(`pixel capture failed: ${pixels.error}`);
} else {
  if (pixels.distinctColours < 12) {
    fail.push(`image is nearly flat (${pixels.distinctColours} distinct colours) — the shader is not shading`);
  } else {
    pass.push(`image has ${pixels.distinctColours} distinct colours, mean luma ${pixels.meanLuma.toFixed(1)}`);
  }
  if (pixels.meanLuma < 8) fail.push("image is essentially black");
}

console.log("renderer:", report.renderer);
console.log("canvas  :", report.canvas?.join("×"));
console.log("frames  :", report.framesDrawn, "drawn,", report.samples, "steady-state samples");
console.log("timing  : median", Number(report.medianMs).toFixed(3), "ms  p95", Number(report.p95Ms).toFixed(3), "ms");
console.log("          ^ SwiftShader. This number is NOT a measurement of the renderer.");
console.log("");

for (const p of pass) console.log("  pass:", p);
for (const f of fail) console.log("  FAIL:", f);
if (logs.length) {
  console.log("\nconsole:");
  for (const l of logs.slice(0, 12)) console.log("  " + l);
}

process.exit(fail.length ? 1 : 0);
