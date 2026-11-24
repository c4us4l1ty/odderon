#!/usr/bin/env node
/**
 * Verify the temporal cache in web/index.html.
 *
 * The cache is the one component that can be wrong while looking perfect. A cache that never
 * invalidates renders a beautiful, plausible, frozen image; a cache that invalidates on every frame
 * renders correctly and costs full price. Neither failure shows up in a screenshot.
 *
 * So this asserts the things that distinguish a working cache from both of those:
 *
 *   1. an unchanged frame takes the cheap path (blit only)
 *   2. every real change takes the expensive path (exposure, resize, render scale)
 *   3. the cached image actually CONTAINS the change - not just "a shade happened"
 *   4. the cache's dimensions match the drawing buffer, which is where resize bugs hide
 *
 * Usage: node tools/check-cache.mjs     (needs LD_LIBRARY_PATH=/tmp/al2023/lib)
 */

import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { extname } from "node:path";
import puppeteer from "puppeteer-core";
import chromium from "@sparticuz/chromium";
import { resolveStaticPath } from "./static-path.mjs";

const MIME = { ".html": "text/html; charset=utf-8", ".js": "text/javascript", ".mjs": "text/javascript" };

const server = createServer(async (req, res) => {
  try {
    // This used to be `readFile(join(webRoot, req.url))` with no containment check at all, so
    // `/../.git/config` was served from `<repo>/.git/config`. It was the third copy of the same
    // hand-rolled static server, and the only one with no check -- which is the argument for having
    // one implementation rather than three that are each almost right.
    const f = resolveStaticPath(req.url);
    if (f === null) throw new Error("forbidden");
    const b = await readFile(f);
    res.writeHead(200, { "content-type": MIME[extname(f)] ?? "text/plain" }).end(b);
  } catch {
    res.writeHead(404).end("not found");
  }
});
const port = 8931;
await new Promise((r) => server.listen(port, "0.0.0.0", r));

const browser = await puppeteer.launch({
  args: [...chromium.args, "--enable-unsafe-swiftshader", "--use-gl=angle"],
  executablePath: await chromium.executablePath(),
  headless: true,
  // The page deliberately idles, so every probe waits on a frame it has to ask for. A generous
  // protocol timeout keeps a genuine hang readable as a timeout instead of a vague protocol error.
  protocolTimeout: 60000,
});

const page = await browser.newPage();
const errors = [];
page.on("pageerror", (e) => errors.push(e.message));
await page.setViewport({ width: 900, height: 560 });
await page.goto(`http://127.0.0.1:${port}/`, { waitUntil: "networkidle0" });

// let the boot burst finish before measuring anything
await new Promise((r) => setTimeout(r, 2500));

const counters = () =>
  page.evaluate(() => {
    const c = window.__odderon?.cache;
    return c ? { shades: c.fullShades, blits: c.blits, w: c.w, h: c.h } : null;
  });

if (!(await counters())) {
  console.error("FAIL: window.__odderon.cache missing - the module did not run, or the cache is gone");
  await browser.close();
  server.close();
  process.exit(1);
}

const results = [];
const check = (name, ok, detail = "") => results.push({ name, ok, detail });

/** run `fn`, wait for it to settle, and report which paths the cache took */
async function paths(fn, waitMs = 550) {
  const a = await counters();
  await fn();
  await new Promise((r) => setTimeout(r, waitMs));
  const b = await counters();
  return { shades: b.shades - a.shades, blits: b.blits - a.blits, w: b.w, h: b.h };
}

// 1. an unchanged frame must be cheap
let r = await paths(() => page.evaluate(() => window.__odderon.requestFrame()));
check("unchanged frame -> blit only", r.shades === 0 && r.blits === 1, JSON.stringify(r));

r = await paths(() => page.evaluate(() => window.__odderon.requestFrame()));
check("unchanged again  -> blit only", r.shades === 0 && r.blits === 1, JSON.stringify(r));

// 2. every real change must be expensive
r = await paths(() => page.click('#exposures button[data-ev="1.6"]'));
check("exposure change  -> full shade", r.shades === 1, JSON.stringify(r));

r = await paths(() => page.setViewport({ width: 820, height: 520 }), 1200);
check("resize           -> full shade", r.shades >= 1, JSON.stringify(r));

const dims = await page.evaluate(() => ({
  cache: [window.__odderon.cache.w, window.__odderon.cache.h],
  canvas: [document.querySelector("canvas").width, document.querySelector("canvas").height],
}));
check(
  "cache size follows the drawing buffer",
  dims.cache[0] === dims.canvas[0] && dims.cache[1] === dims.canvas[1],
  JSON.stringify(dims),
);

r = await paths(() => page.click('#scales button[data-scale="0.5"]'));
check("render scale     -> full shade", r.shades >= 1, JSON.stringify(r));

r = await paths(() => page.evaluate(() => window.__odderon.requestFrame()));
check("static again     -> blit only", r.shades === 0 && r.blits === 1, JSON.stringify(r));

// 3. the decisive one: the cache must CONTAIN the change, not merely have re-shaded
const before = await page.evaluate(() => window.__odderon.capturePixels());
await page.click('#exposures button[data-ev="0.62"]');
await new Promise((r) => setTimeout(r, 600));
const after = await page.evaluate(() => window.__odderon.capturePixels());
check(
  "cached image actually updates",
  Math.abs(after.meanLuma - before.meanLuma) > 3,
  `luma ${before.meanLuma.toFixed(1)} -> ${after.meanLuma.toFixed(1)}`,
);

// ---------------------------------------------------------------------------------------------
// THE LIVE PASS AND THE MEASURED PASS MUST BE THE SAME CODE.
//
// `measure frame time` is what R2 is judged on. If it measured a different operation from the one the
// loop draws, the panel would report a number for a mechanism that is not running -- which is exactly
// the error that cost this project its R2 claim once already: an ablation row and a renderer that had
// drifted apart, reported as if they were the same measurement.
//
// `blitFrame()` is the single primitive both call. This asserts that structurally, because a comment
// saying "these are the same" is not a check.
{
  const src = await readFile(new URL("../web/index.html", import.meta.url), "utf8");
  const strip = (t) => t.replace(/\/\/[^\n]*/g, "");
  const fnStart = src.indexOf("function blitFrame()");
  check("blitFrame() exists as the single per-frame primitive", fnStart > 0);
  if (fnStart > 0) {
    let i = src.indexOf("{", fnStart), depth = 0, end = -1;
    for (let k = i; k < src.length; k++) {
      if (src[k] === "{") depth++;
      else if (src[k] === "}") { depth--; if (!depth) { end = k + 1; break; } }
    }
    const body = strip(src.slice(fnStart, end));
    check("blitFrame() contains the blitFramebuffer call", /blitFramebuffer\s*\(/.test(body));
    check("blitFrame() falls back to a draw", /drawArrays\s*\(/.test(body));
    // No raw per-frame draw left in the measurement function.
    const mStart = src.indexOf("function measureFrame(");
    let j = src.indexOf("{", mStart), d2 = 0, mEnd = -1;
    for (let k = j; k < src.length; k++) {
      if (src[k] === "{") d2++;
      else if (src[k] === "}") { d2--; if (!d2) { mEnd = k + 1; break; } }
    }
    const measured = strip(src.slice(mStart, mEnd));
    check("measureFrame() calls blitFrame()", /blitFrame\s*\(\)/.test(measured));
    // The only drawArrays allowed in measureFrame is the SHADE, which is a different pass.
    const draws = (measured.match(/drawArrays\s*\(/g) || []).length;
    check("measureFrame() has no second copy of the blit", draws === 1, `drawArrays calls: ${draws}`);
  }
}

// ---------------------------------------------------------------------------------------------
// A DEVICE WITH NO GPU TIMER MUST STILL GET A USABLE STATEMENT, AND AN HONEST ONE.
//
// Reference B has no EXT_disjoint_timer_query_webgl2, so `gl.finish()` there returns one refresh period
// (5.00 ms at 200 Hz) regardless of what the renderer did. The panel used to report exactly that as
// "vsync-bound", which is truthful but useless: it is the display's number, not the renderer's.
//
// The replacement is an UPPER BOUND -- K completed frames over T ms gives T/K -- and the whole value of
// it is that it cannot overstate the renderer's speed. So this asserts the bound is measured with
// `finish()` and NOT with a pixel readback (which measured 9.3 ms against `finish()`'s 0.008 ms on
// ANGLE, i.e. it would have measured the readback), and that the panel calls it a bound.
{
  const src = await readFile(new URL("../web/index.html", import.meta.url), "utf8");
  const strip = (t) => t.replace(/\/\/[^\n]*/g, "");
  const start = src.indexOf("function upperBoundBlit(");
  check("upperBoundBlit() exists for devices without a GPU timer", start > 0);
  if (start > 0) {
    let i = src.indexOf("{", start), depth = 0, end = -1;
    for (let k = i; k < src.length; k++) {
      if (src[k] === "{") depth++;
      else if (src[k] === "}") { depth--; if (!depth) { end = k + 1; break; } }
    }
    const body = strip(src.slice(start, end));
    check("the bound forces completion with finish(), not a readback",
      /gl\.finish\s*\(/.test(body) && !/readPixels/.test(body));
    check("the bound blits from the never-sampled probe target, not the live cache",
      /cache\.probeFbo/.test(body) && !/cache\.fbo\b/.test(body));
  }

  // Simulate the phone: no timer extension at all.
  const pg = await browser.newPage();
  await pg.evaluateOnNewDocument(() => {
    const orig = WebGL2RenderingContext.prototype.getExtension;
    WebGL2RenderingContext.prototype.getExtension = function (n) {
      if (n === "EXT_disjoint_timer_query_webgl2") return null;
      return orig.call(this, n);
    };
  });
  await pg.setViewport({ width: 900, height: 560 });
  await pg.goto(`http://127.0.0.1:${port}/`, { waitUntil: "networkidle0" });
  await new Promise((r) => setTimeout(r, 2500));
  const r = await pg.evaluate(async () => {
    const t0 = performance.now();
    await window.__odderon.measureFrame(12);
    const wall = performance.now() - t0;
    document.getElementById("m-r2").textContent = document.getElementById("m-r2").textContent;
    return {
      wall,
      upper: window.__odderon.bench.blitUpperMs,
      timer: window.__odderon.bench.gpuTimerState,
      applyRun: (() => { document.getElementById("measure").click(); return true; })(),
    };
  });
  await new Promise((r) => setTimeout(r, 1200));
  const r2 = await pg.evaluate(() => document.getElementById("m-r2").textContent);
  check("no timer on this device, as on Reference B", r.timer === "absent", r.timer);
  check("the upper bound is measured rather than NaN", Number.isFinite(r.upper), String(r.upper));
  check("the panel calls it an upper bound", /upper bound/.test(r2 || ""), r2 || "");
  check("an inconclusive bound is NOT reported as a pass",
    !/^\s*\d.*< 1\.5\s*$/.test(r2 || "") || /real cost is lower/.test(r2 || ""), r2 || "");
  // The button must stay responsive: 12 samples with a wall-clock cap, not 12 x 30 frames.
  check("measuring stays responsive without a GPU timer", r.wall < 600, `${r.wall.toFixed(0)} ms`);
  await pg.close();
}

// ---------------------------------------------------------------------------------------------
// THE BLIT MUST RENDER THE SAME IMAGE AS THE FALLBACK.
//
// web/index.html's per-frame pass is now `blitFramebuffer` because Reference B measured it at 1.27 ms
// against 1.65 ms for the texture-read triangle, and R2's budget is 1.5 ms -- so the mechanism IS the
// margin, and it is load-bearing. A blit that copies nothing, copies the wrong rect, or is silently
// refused by the driver would show as a fast frame time and a black screen, which is precisely the
// failure a timing test cannot see. `?blit=0` forces the fallback so the two can be compared.
//
// The comparison deliberately ignores the HUD: it is expected to differ, because it names which
// mechanism is live. It requires ZERO differing pixels outside that panel, allowing only differences of
// one level, because `NEAREST` is exact and anything larger is a real rendering difference.
{
  const shotOf = async (query) => {
    const pg = await browser.newPage();
    // Freeze the clock: two loads seconds apart render different sun positions, which would make the
    // comparison measure elapsed time. The date is arbitrary; what matters is that both see the same.
    await pg.evaluateOnNewDocument((t) => {
      const RealDate = Date;
      class FakeDate extends RealDate {
        constructor(...a) { return a.length ? new RealDate(...a) : new RealDate(t); }
        static now() { return t; }
      }
      window.Date = FakeDate;
    }, new Date("2026-06-21T12:00:00Z").getTime());
    await pg.setViewport({ width: 900, height: 560 });
    await pg.goto(`http://127.0.0.1:${port}/${query}`, { waitUntil: "networkidle0" });
    await new Promise((r) => setTimeout(r, 3000));
    const label = await pg.evaluate(() => document.getElementById("m-path")?.textContent ?? "");
    const png = await pg.screenshot({ encoding: "base64" });
    await pg.close();
    return { label, png };
  };

  const on = await shotOf("");
  const off = await shotOf("?blit=0");
  check("the live renderer takes the blit path", /blitFramebuffer/.test(on.label), on.label);
  check("?blit=0 really falls back", /tex read/.test(off.label), off.label);

  // Compare in the browser: decoding PNGs in node would mean a dependency, and the page already has
  // a canvas. Both screenshots are pushed through 2D contexts and differenced there.
  const diff = await page.evaluate(async (a, b) => {
    const load = (b64) => new Promise((res) => {
      const img = new Image();
      img.onload = () => res(img);
      img.src = "data:image/png;base64," + b64;
    });
    const [ia, ib] = await Promise.all([load(a), load(b)]);
    const w = ia.width, h = ia.height;
    if (ib.width !== w || ib.height !== h) return { error: "size mismatch" };
    const ctx = (img) => {
      const c = document.createElement("canvas");
      c.width = w; c.height = h;
      c.getContext("2d").drawImage(img, 0, 0);
      return c.getContext("2d").getImageData(0, 0, w, h).data;
    };
    const pa = ctx(ia), pb = ctx(ib);
    // The HUD occupies the left column; its text differs by design.
    const HUD = 340;
    let differing = 0, worst = 0, compared = 0;
    for (let y = 0; y < h; y++) {
      for (let x = HUD; x < w; x++) {
        const o = (y * w + x) * 4;
        compared++;
        const d = Math.max(
          Math.abs(pa[o] - pb[o]), Math.abs(pa[o + 1] - pb[o + 1]), Math.abs(pa[o + 2] - pb[o + 2]));
        if (d > 1) { differing++; if (d > worst) worst = d; }
      }
    }
    return { differing, worst, compared };
  }, on.png, off.png);

  check("blit and fallback render the same scene", !diff.error && diff.differing === 0,
    diff.error ?? `${diff.differing} pixels differ by >1 (worst ${diff.worst}) of ${diff.compared}`);
}

// ---------------------------------------------------------------------------------------------
await browser.close();
server.close();

const failed = results.filter((r) => !r.ok);
for (const r of results) {
  console.log(`  ${r.ok ? "pass" : "FAIL"}: ${r.name}${r.detail ? "  " + r.detail : ""}`);
}
if (errors.length) {
  console.log("\npage errors:");
  for (const e of errors) console.log("  " + e);
}
if (failed.length || errors.length) {
  console.log(`\n${failed.length} failed, ${errors.length} page error(s)`);
  process.exit(1);
}
console.log(`\nall ${results.length} cache checks passed`);
