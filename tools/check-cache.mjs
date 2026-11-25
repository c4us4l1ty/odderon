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

    // THE COMPLETION MUST BE OUTSIDE THE LOOP. This is the third time this exact fault has appeared in
    // this file: `gl.finish()` waits for the next refresh boundary on a phone, so a per-iteration finish
    // makes K iterations cost K refresh periods. Reference B reported "< 4.08 ms" at 250 Hz -- 4.00 ms,
    // the display, dressed as a bound. The per-iteration unit is the `blitOnce` closure, so that closure
    // is where the assertion goes: it must issue work and force nothing.
    const boStart = src.indexOf("const blitOnce = () =>");
    check("the per-iteration unit is a named closure", boStart > 0);
    if (boStart > 0) {
      let i = src.indexOf("{", boStart), depth = 0, end = -1;
      for (let k = i; k < src.length; k++) {
        if (src[k] === "{") depth++;
        else if (src[k] === "}") { depth--; if (!depth) { end = k + 1; break; } }
      }
      const unit = strip(src.slice(boStart, end));
      check("the per-iteration unit contains the blit", /blitFramebuffer\s*\(/.test(unit));
      check("the per-iteration unit does NOT force completion",
        !/finish\s*\(/.test(unit) && !/readPixels/.test(unit),
        "a per-iteration stall is how a bound becomes a measurement of vsync");
    }
    // ...and the batch must be large enough that one stall amortises to well under the budget.
    const km = /const K = (\d+);/.exec(body);
    check("the batch is large enough to amortise one stall", km !== null && Number(km[1]) >= 32,
      km ? `K = ${km[1]}` : "K not found as a literal");
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
  // THE ROW MUST SAY WHICH QUANTITY IT IS. It used to say "upper bound", and that was the claim under
  // test -- but a T/K figure from K *pipelined* blits bounds THROUGHPUT, not the cost of one frame, and
  // on this sandbox the two differ by an order of magnitude (0.014 ms T/K against 0.2 ms for one fenced
  // frame). A pipelined GPU answers throughput with a smaller number than any frame's latency, so the
  // old wording attested the very overclaim this change removes.
  //
  // The contract is not weakened: the row must still state the quantity and the budget comparison, and
  // it must now ALSO be honest about which one it is.
  check("the panel states the per-frame quantity and the budget",
    /per frame/.test(r2 || "") && /1\.5/.test(r2 || ""), r2 || "");
  check("the panel does not call a throughput bound a frame time",
    !/ms (upper bound|direct)\b/.test(r2 || "") || /per frame/.test(r2 || ""), r2 || "");

  // THE DIRECT MEASUREMENT MUST REFUSE RATHER THAN INVENT. This is the quantity claim 4 has been
  // parked on, so it is the one most likely to be flattered by a bad implementation -- and two were
  // written while building it. It may return a number or a stated reason, and a number it returns must
  // be a physical duration. What it must never do is return a negative (which it did: the panel read
  // "~-0.1 ms frame"), or a number while its own floor is above the budget it is being judged against.
  const direct = await pg.evaluate(async () => {
    const r = await window.__odderon.directBlitMs(16);
    return { ms: r.ms, reason: r.reason ?? null, floor: r.floor ?? null, np: !Number.isFinite(r.ms) };
  });
  check("the direct measurement either reports a duration or says why not",
    direct.ms === null ? typeof direct.reason === "string" && direct.reason.length > 10 : Number.isFinite(direct.ms),
    JSON.stringify(direct));
  if (direct.ms !== null) {
    check("the direct measurement is not negative", direct.ms >= 0, `${direct.ms} ms`);
    check("a resolved direct measurement means its floor was under budget",
      !(direct.floor !== null && direct.floor > 1.5), `floor ${direct.floor}`);
  } else {
    check("a refused direct measurement names its floor or its reason",
      direct.reason !== null, String(direct.reason));
  }
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
    // FREEZE BOTH CLOCKS, NOT JUST ONE.
    //
    // This froze `Date` and left `performance.now()` alone, which was enough while the scene's animated
    // quantity happened to be quantised. It is not enough: `drawFrame()` places the body with
    // `orbitPosition(nowMs / 1000)`, i.e. seconds since page load, so two loads a second apart put the
    // moon a second along its orbit. That is a real, hard-edged feature -- 349 pixels differing by up
    // to 102 levels, the identical count every time, which is what a displaced disc looks like and not
    // what timing smear looks like.
    //
    // `performance.now()` now advances only when the renderer asks it to, at a fixed 16 ms per drawn
    // frame, so both pages see the same instant and the same frame index.
    await pg.evaluateOnNewDocument((t) => {
      const RealDate = Date;
      class FakeDate extends RealDate {
        constructor(...a) { return a.length ? new RealDate(...a) : new RealDate(t); }
        static now() { return t; }
      }
      window.Date = FakeDate;
      let fake = 1000;
      const realNow = performance.now.bind(performance);
      Object.defineProperty(performance, "now", {
        configurable: true,
        value: () => fake,
      });
      // Advance only when a frame is actually requested, so the two pages cannot drift apart by
      // wall-clock and still agree on every drawn frame.
      window.__freezeTick = () => { fake += 16; };
      void realNow;
    }, new Date("2026-06-21T12:00:00Z").getTime());
    await pg.setViewport({ width: 900, height: 560 });
    await pg.goto(`http://127.0.0.1:${port}/${query}`, { waitUntil: "networkidle0" });
    // THE RENDERER IS RENDER-ON-DEMAND, SO "WAIT 3 SECONDS" IS NOT A SETTLE CONDITION.
    //
    // This comparison used a fixed 3 s sleep and then screenshotted. It failed intermittently with
    // 349 pixels differing by up to 102 levels, and passed on the next run with 0 -- on the same code.
    // A check that passes on retry is not a check.
    //
    // The page draws when something asks it to. Without a frame request, `page.screenshot()` returns
    // whatever the compositor last held, which on a freshly loaded page can be mid-flight. So the
    // settle condition is made explicit: ask for a frame, wait for it to be drawn, and only then look.
    //
    // MEASURED: with a flat sleep instead of this settle, this check failed 3 times in 8 runs (twice
    // before the clock was also frozen, once after). With it, 8 consecutive runs pass.
    await new Promise((r) => setTimeout(r, 600));              // let the kernel and first shade land
    await pg.evaluate(async () => {
      const od = window.__odderon;
      if (!od) return;
      // FREEZE THE FRAME INDEX TOO, and this one is new information rather than belt and braces.
      //
      // `emit()` mixes SENSOR GRAIN into every pixel as a hash of `gl_FragCoord + uFrame`, where
      // `uFrame` is `bench.framesDrawn % 1024`. The grain exists so an 8-bit gradient does not band, and
      // it is deliberately per-frame. So two page loads that draw a different number of frames before
      // settling -- and they do, because the kernel landing and the settle loop differ by a frame or two
      // -- produce two DIFFERENT noise draws over identical scenes.
      //
      // That made this comparison measure the noise: 149 085 pixels differing by up to 5 levels, on a
      // flat-colour probe, with the two per-frame passes proven exact. The reason it only started
      // failing now is worth recording: the interior used to return from `main()` BEFORE the grain
      // block, so most of the frame carried no grain at all and the difference stayed under the
      // threshold. Routing every path through one output stage -- which was the fix for the grain being
      // silently unreachable -- exposed that this assertion had been comparing a largely noiseless
      // image. The claim it was making was true by accident.
      //
      // Pinning the index makes both pages draw the same noise, so the check compares what it means to
      // compare: whether the per-frame pass reproduces the shaded image.
      const before = od.bench.framesDrawn;
      if (window.__freezeTick) window.__freezeTick();
      od.requestFrame();
      // Bounded: a renderer that never draws must fail this check, not hang it.
      for (let i = 0; i < 200 && od.bench.framesDrawn === before; i++) {
        await new Promise((r) => setTimeout(r, 10));
      }
    });
    await new Promise((r) => setTimeout(r, 250));              // one more presentation interval
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
