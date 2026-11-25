#!/usr/bin/env node
/**
 * Claim 2's second half: **the room cools**, and you can see it happen.
 *
 * ## Why this exists
 *
 * `tests/thermal.rs` has measured the cooling curve for months -- 5.9 K in ten minutes, against a 44.4 min
 * time constant -- and the page could not show it. The wasm ABI exposed four STEADY-STATE numbers, so
 * flipping the curtain snapped the room from 1440 W to 120 W and from 73.7 C to its cooled resting value
 * with nothing in between. The room was a calculator. "The room is a spacecraft" is a claim about time.
 *
 * ## What makes this a gate rather than a screenshot test
 *
 * A trajectory is easy to fake. A linear ramp looks like cooling; a CSS transition looks like cooling; a
 * constant with a moving label looks like cooling. So this checks the things a fake gets wrong:
 *
 *   1. THE CURVE IS THE KERNEL'S. Sampled from the module at 0/60/300/600 s and compared against the
 *      numbers `tests/thermal.rs` asserts (5.9 K at 600 s, monotone, concave). An ABI that drifts from
 *      the model fails here, and the drift is caught at the boundary rather than in a picture.
 *   2. IT IS A FIRST-ORDER RESPONSE, NOT A RAMP. Cooling must be fast early and slow late:
 *      drop(0..60) > drop(240..300) > drop(540..600), each by a clear margin. A linear fade fails.
 *   3. THE PAGE'S CLOCK APPLIES ITS STATED RATE. The panel prints `at 300 s @ 60x`; this measures the
 *      wall time it took to get there and requires the two to agree. A clock that drifts from its own
 *      label is a number nobody can audit -- and the label is the only reason the preview is watchable.
 *   4. THE PICTURE MOVES, AND ONLY WHILE IT SHOULD. The frame must change during the cooling and must
 *      STOP changing once the room settles (that is R2: idle is nothing scheduled, and the clock is the
 *      one thing in this page that could break it).
 *
 * Usage: node tools/check-room-clock.mjs [--port N]
 * Needs a built wasm. A static half runs without one.
 */

import { readFile } from "node:fs/promises";
import { createServer } from "node:http";
import { extname, join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { resolveStaticPath } from "./static-path.mjs";

const HERE = dirname(fileURLToPath(import.meta.url));
const ROOT = join(HERE, "..");
const WASM = join(ROOT, "dist", "odderon_dmath_wasm.wasm");
const STATIC_ONLY = process.argv.includes("--static");

const args = process.argv.slice(2);
const portArg = args.indexOf("--port");
const port = portArg >= 0 ? Number(args[portArg + 1]) : 8971;

const fail = [];
const pass = [];
const check = (name, ok, detail = "") => (ok ? pass : fail).push(detail ? `${name}  ${detail}` : name);
const report = () => {
  for (const m of pass) console.log(`  ok   ${m}`);
  for (const m of fail) console.log(`  FAIL ${m}`);
  console.log("");
  console.log(fail.length ? "room-clock check FAILED" : "room-clock check passed");
  process.exit(fail.length ? 1 : 0);
};

// --- 1: the kernel's trajectory, against the Rust test's own numbers ----------------------------
const src = await readFile(join(ROOT, "web", "index.html"), "utf8");
check("the page asks the kernel for a temperature at a given time", /room_temperature_k\s*\(/.test(src));
check("the page asks for the settled temperature too", /room_settled_k\s*\(/.test(src));
check("the clock is labelled with its rate, not implied", /clockScale/.test(src) && /@ \$\{/.test(src));
// THE CACHE MUST DEPEND ON THE TEMPERATURE. This one line is the entire bug this gate was blind to for
// four runs: without it the clock ticks, the panel counts down, and every frame re-presents the original
// image. A consequence-based assertion could not see it, so the cause is asserted directly.
check(
  "the cache is invalidated by the room's temperature, not only by the sun",
  /lastRoomTempK/.test(src) && /ROOM_SHADE_K/.test(src),
);
// THE ROOM MUST REMEMBER. `fromK` is the only thing that carries a trajectory across a curtain change, and
// the page must read it BEFORE flipping the state or it captures the destination. A `room_settled_k(0)`
// in `roomTemperature()` is the old teleport, and it read as plausible for a whole session.
check("the page continues the trajectory from the temperature it is leaving", /roomState\.fromK = Number\.isFinite/.test(src));
// `roomTemperature()` must be ONE call to the trajectory, with the curtain state as an argument to it.
// The defect was a ternary whose two branches were different laws -- a curve one way, a constant the other.
// The first version of this line looked for one exact phrasing and a mutant with the defect written
// differently walked straight through it, which is the "threshold a broken build clears" failure again.
const tempFn = (/function roomTemperature\(nowMsArg\) \{([\s\S]*?)\n\}/.exec(src) || [])[1] || "";
check(
  "roomTemperature() is one trajectory call, not a different law per curtain state",
  /room_temperature_from_k\(/.test(tempFn) && !/room_settled_k\(/.test(tempFn),
  tempFn.includes("room_settled_k") ? "it reads room_settled_k() -- the teleport" : "one call, no settled fallback",
);
check("the trajectory runs in both directions, so an open curtain can still be moving",
  /function roomMoving/.test(src) && /roomMoving\(\)/.test(src));
// ...AND THE COUPLING MUST BE THE PHYSICAL ONE, not linear-in-T dressed up as radiation.
check(
  "the temperature drives a T^4 term (Stefan-Boltzmann), not a linear one",
  /th \* th \* th \* th/.test(src),
);
check("the visible-band claim is not made anywhere in the shader", !/radiates in proportion to its absolute temperature/.test(src));
// A PAGE WITH NO KERNEL MUST STILL BE ABLE TO BLIT. `NaN < ROOM_SHADE_K` is false, so a naive comparison
// makes every frame a full shade -- the whole cost model defeated by an absent file, on exactly the page
// `check-missing-kernel` exists to keep honest. This was a real defect, caught by CI's no-wasm job after
// passing locally with a kernel present. The guard is asserted here so it cannot come back silently.
// The assertion is on the GUARDED COMPARISON, not on the presence of `Number.isFinite` anywhere in the
// file -- the first version of this line matched the uniform-upload code and passed a mutant that had the
// bug back. A guard that a broken build clears is not a guard.
check(
  "a missing kernel does not invalidate the cache every frame",
  /Number\.isFinite\(roomState\.temperatureK\)\s*&&[\s\S]{0,120}ROOM_SHADE_K/.test(src),
);
// The window and the rate are one decision with two numbers, and both must match the derivation stated
// beside them. A page that ran 600 s and called it settled while 80 % of the fall was still ahead was the
// version this replaced.
const settle = Number((/^const ROOM_SETTLE_S = (\d+);/m.exec(src) || [])[1]);
const rate = Number((/clockScale: (\d+),/.exec(src) || [])[1]);
const shadeK = Number((/^const ROOM_SHADE_K = ([\d.]+);/m.exec(src) || [])[1]);
check("the settle window is the derived one", settle === 4800, `${settle} s of simulated time`);
check("the clock rate is the derived one", rate === 240, `${rate}x`);
check("the re-shade quantum is the measured one", shadeK === 2.0, `${shadeK} K`);
check(
  "the whole run fits in one wall-clock breath at the stated rate",
  settle / rate <= 25,
  `${settle} s / ${rate}x = ${(settle / rate).toFixed(1)} s of wall time`,
);

if (STATIC_ONLY) {
  console.log(`  (static mode: ${pass.length + fail.length} source assertions, no wasm and no browser)`);
  report();
}

let ex = null;
try {
  const bytes = await readFile(WASM);
  ex = (await WebAssembly.instantiate(bytes, {})).instance.exports;
} catch (e) {
  fail.push(`could not load the kernel at dist/odderon_dmath_wasm.wasm: ${e.message}`);
}

if (ex) {
  const K = (t) => ex.room_temperature_k(t) - 273.15;
  const t0 = K(0);
  const t60 = K(60);
  const t300 = K(300);
  const t600 = K(600);

  // THE RUST TEST'S NUMBER. `the_room_cools_by_several_kelvin_over_minutes` asserts > 3 K and < 12 K at
  // 600 s and documents 5.9. This pins the documented figure, so the ABI cannot quietly become a
  // different model -- which is exactly what happened when the exports first invented their own fixture
  // and produced a tidy, self-consistent, wrong 72 % draw drop.
  const drop600 = t0 - t600;
  check(
    "the kernel cools 5.9 K in ten minutes, as the Rust test measures",
    Math.abs(drop600 - 5.9) < 0.15,
    `${drop600.toFixed(2)} K`,
  );

  // CONCAVE, NOT LINEAR. The whole difference between a first-order response and a fade.
  const d1 = t0 - t60;      // 0..60
  const d5 = K(240) - t300; // 240..300
  const d10 = K(540) - t600;// 540..600
  // THE EXPECTED RATIO IS A CONSEQUENCE OF THE TIME CONSTANT, not a number I get to pick. On a 44.4 min
  // constant, a 60 s window early in the fall loses 1.225x what a 60 s window at ten minutes loses
  // (exp(-a/tau) differences). The first version demanded 1.5x and 1.2x -- a guess that the physics
  // does not support -- so a CORRECT curve failed. What distinguishes first-order from a linear ramp is
  // that the ratio is > 1 and consistent with tau, so that is what is asserted.
  const TAU_S = 44.4 * 60;
  const expected = (Math.exp(-0 / TAU_S) - Math.exp(-60 / TAU_S)) / (Math.exp(-540 / TAU_S) - Math.exp(-600 / TAU_S));
  check(
    "the curve decelerates by the ratio a 44.4 min time constant predicts",
    d1 > d5 && d5 > d10 && Math.abs(d1 / d10 - expected) / expected < 0.12,
    `${d1.toFixed(3)} > ${d5.toFixed(3)} > ${d10.toFixed(3)} K per 60 s; ratio ${(d1 / d10).toFixed(3)} vs ${expected.toFixed(3)} predicted`,
  );

  // Monotone: no oscillation, no numerical ringing. Forward Euler at the stability bound rings; this is
  // half of it, and the assertion is what makes that claim checkable rather than asserted.
  let prev = t0;
  let monotone = true;
  for (let t = 30; t <= 1200; t += 30) {
    const v = K(t);
    if (v > prev + 1e-9) monotone = false;
    prev = v;
  }
  check("the trajectory is monotone over twenty minutes (no ringing)", monotone);

  // THE PAGE STOPS DRAWING AT 600 s, AND THE ROOM IS NOT DONE AT 600 s. That was the assumption behind
  // an earlier version of this line, which asserted the curve was "flat" past the cutoff < 1 K. It is not:
  // 4.74 K remain, which is 80 % of the entire cooling. On a 44.4 min time constant the ten-minute mark
  // is a fifth of a tau, and a first-order system has barely started.
  //
  // So the cutoff is a PRESENTATION choice with a cost, and the honest assertion is that the cost is
  // bounded and stated rather than that it does not exist. `check-room.mjs` and the panel say 600 s; this
  // says how much cooling happens after the frame stops, so nobody can later believe the room is settled.
  // What is left on the table past the window that actually ships. Recomputed from the window, so this
  // assertion cannot drift from the page: the old one hard-coded 600 s and went on passing after the page
  // changed. Measured: 4.8 K remains at 4800 s, against 13.3 K at the 600 s this page used to stop at.
  const remAtCut = K(settle) - K(7200);
  const remAt600 = K(600) - K(7200);
  check(
    "the shipped window stops near rest, not a fifth of the way through",
    remAtCut < 6.0 && remAt600 > 10.0,
    `${remAtCut.toFixed(2)} K remains at ${settle} s (${remAt600.toFixed(2)} K did at 600 s)`,
  );
  // The magnitude the shader is entitled to paint, from the kernel itself rather than from a hand sum.
  // It must be THIS ratio and not the settled-closed one: the shipped window ends at 7200 s, where the
  // room is still 1.94 K above its closed rest, so K(7200) is what the picture actually sees. The first
  // version of this line asserted 0.702, which is (317.5/346.8)^4 -- the SETTLED-closed ratio -- against
  // a K(7200) that gives 0.7196, and failed its own check. The lesson is the one already in this file:
  // derive the expected value from the trajectory, not from a nearby hand computation.
  // `K()` above returns CELSIUS, which is why the first version of this line read 0.156 and failed: a
  // ratio of temperatures in Celsius is not a physical quantity at all. T^4 needs kelvin, so this goes
  // to the module directly. (Found by the check failing, not by review -- the arithmetic looked right.)
  const radiated = Math.pow(ex.room_temperature_k(7200) / ex.room_temperature_k(0), 4);
  check(
    "the visible-band claim is not made: the cooling is at 10 um, worth a T^4 ratio",
    Math.abs(radiated - 0.7196) < 0.002,
    `(K(7200)/K(0))^4 = ${radiated.toFixed(4)}, i.e. 28 % of the radiant power`,
  );
  check("a nonsense time is clamped rather than looping forever", Number.isFinite(K(1e12)) && Number.isFinite(K(-5)));

  // The finding, reported rather than hidden: the settled temperatures are hot, and that is the model.
  console.log(
    `  note settled: ${(ex.room_settled_k(0) - 273.15).toFixed(1)} C open, ` +
      `${(ex.room_settled_k(1) - 273.15).toFixed(1)} C closed ` +
      `(UA = 45 W/K, 1440 W of sun: arithmetic verified, real rooms have more conductance)`,
  );
}

// --- 3 & 4: the page's clock and the frame ------------------------------------------------------
let browser = null;
let server = null;
try {
  const puppeteer = (await import("puppeteer-core")).default;
  const chromium = (await import("@sparticuz/chromium")).default;

  server = createServer(async (req, res) => {
    try {
      const p = resolveStaticPath(req.url.split("?")[0]);
      const body = await readFile(p);
      const ext = extname(p);
      const type = ext === ".html" ? "text/html" : ext === ".wasm" ? "application/wasm" : "text/plain";
      res.writeHead(200, { "content-type": type });
      res.end(body);
    } catch {
      res.writeHead(404);
      res.end("not found");
    }
  });
  await new Promise((r) => server.listen(port, "127.0.0.1", r));

  browser = await puppeteer.launch({
    executablePath: await chromium.executablePath(),
    args: [...chromium.args, "--enable-unsafe-swiftshader", "--use-gl=angle"],
    defaultViewport: { width: 1200, height: 700 },
    headless: true,
  });
  const page = await browser.newPage();
  await page.goto(`http://127.0.0.1:${port}/`, { waitUntil: "networkidle0", timeout: 90000 });
  await new Promise((r) => setTimeout(r, 2000));

  const before = await page.evaluate(() => ({
    row: document.getElementById("m-temp").textContent,
    // `null` is deliberate when the curtain is open: there is no trajectory, so there is no timestamp.
    hasTrajectory: /at \d+ s/.test(document.getElementById("m-temp").textContent),
  }));
  check("an open, settled room shows a temperature and no clock", !before.hasTrajectory && /C/.test(before.row), before.row);

  // Close the curtain and watch the clock. The rate claim is measured against WALL time.
  const startedAt = Date.now();
  await page.evaluate(() => document.getElementById("curtain").click());
  await new Promise((r) => setTimeout(r, 1200));
  const mid = await page.evaluate(() => ({
    elapsed: window.__odderon.roomState.elapsedS,
    scale: window.__odderon.roomState.clockScale,
    row: document.getElementById("m-temp").textContent,
    tK: window.__odderon.roomState.temperatureK,
  }));
  const wallS = (Date.now() - startedAt) / 1000;
  const impliedRate = mid.elapsed / wallS;

  check("the clock is running after the curtain closes", mid.elapsed > 10, `${Math.round(mid.elapsed)} s`);
  check(
    "the panel's stated rate is the rate actually applied",
    Math.abs(impliedRate - mid.scale) / mid.scale < 0.35,
    `measured ${impliedRate.toFixed(0)}x against a stated ${mid.scale}x`,
  );
  // The row says `at 65 s` while the state says 70 s, because the panel is painted on the frame that drew
  // it and the state is read a moment later. What has to agree is that both are talking about the same
  // clock at the same order -- an off-by-a-frame is the panel doing its job, an off-by-10x is a label
  // lying about its rate. (The first version demanded exact equality and failed a correct page.)
  // The tolerance is a WALL-TIME lag, not a simulated-time number, and that distinction is the whole
  // point: `updateHud` runs on a 100 ms interval, so at 240x the panel is legitimately up to 24 simulated
  // seconds behind the state. The first version of this allowed 10 simulated seconds, which was a
  // leftover from the 60x rate -- correct there by luck, and wrong here. 150 ms of wall time is the
  // honest bound, and it fails at anything above ~1.5x the real interval.
  const rowS = Number((/at (\d+) s/.exec(mid.row) || [])[1]);
  const lagAllowed = 0.15 * mid.scale;
  check(
    "the panel's elapsed time lags the state by less than 150 ms of wall time",
    Number.isFinite(rowS) && Math.abs(rowS - mid.elapsed) <= lagAllowed,
    `row ${rowS} s vs state ${Math.round(mid.elapsed)} s (allowed ${Math.round(lagAllowed)} s at ${mid.scale}x)`,
  );

  // THE PICTURE MUST MOVE WHILE COOLING, AND THIS MUST BE MEASURED FROM THE CACHE.
  //
  // The first version of this assertion compared two screenshots and required 3000 differing pixels. It
  // reported 583, then 602, and failed -- correctly, as it turned out, but not for the reason the comment
  // said. A screenshot of this page has the PANEL drawn on it, and the panel is printing a stopwatch. So
  // the two "different" frames differed in the panel's own digits, inside the HUD box, while the scene
  // below y=300 moved by 0.008 levels: the gate was measuring its instrument.
  //
  // Reading the cache answers the question the gate is actually asking -- did the shaded image change --
  // and it is the same argument the page already records on `captureNow()`. Measured from the cache at
  // the trajectory's endpoints: 809,467 of 840,000 pixels differ, up to 16 levels, mean 13.8.
  const cacheDiff = (a, b) => {
    const P = Buffer.from(a.b64, "base64");
    const Q = Buffer.from(b.b64, "base64");
    let moved = 0, maxd = 0, sum = 0;
    for (let i = 0; i < P.length; i += 4) {
      const d = Math.max(Math.abs(P[i] - Q[i]), Math.abs(P[i + 1] - Q[i + 1]), Math.abs(P[i + 2] - Q[i + 2]));
      if (d > 0) moved++;
      if (d > maxd) maxd = d;
      sum += d;
    }
    return { moved, maxd, mean: sum / (P.length / 4) };
  };

  // Freeze the clock at a chosen simulated time and read the cache. `room_temperature_k` takes t as an
  // argument precisely so this is possible: seeking is exact, so the two frames are the same room at two
  // stated times and not two guesses at a running clock.
  const readAt = (t) =>
    page.evaluate(async (T) => {
      const o = window.__odderon;
      o.roomState.clockScale = 1e-9;                       // frozen
      o.roomState.curtainChangedAt = performance.now() - T * 1e9 * 1000;
      o.cache.valid = false;                               // force a real shade
      o.requestFrame();
      await new Promise((r) => setTimeout(r, 400));
      o.requestFrame();
      await new Promise((r) => setTimeout(r, 400));
      const gl = o.gl, c = o.cache;
      const buf = new Uint8Array(c.w * c.h * 4);
      const prev = gl.getParameter(gl.READ_FRAMEBUFFER_BINDING);
      gl.bindFramebuffer(gl.READ_FRAMEBUFFER, c.fbo);
      gl.readPixels(0, 0, c.w, c.h, gl.RGBA, gl.UNSIGNED_BYTE, buf);
      gl.bindFramebuffer(gl.READ_FRAMEBUFFER, prev);
      let s = "";
      for (let i = 0; i < buf.length; i += 8192) s += String.fromCharCode.apply(null, buf.subarray(i, i + 8192));
      return { b64: btoa(s), tK: o.roomState.temperatureK, shades: c.fullShades };
    }, t);

  // THE PAGE'S TEMPERATURE MUST BE THE KERNEL'S, AT THE SAME t.
  //
  // A mutant proved this was missing: replacing `room_temperature_k(t)` with a straight line
  // (346.8 - 0.004 t) passed every assertion here, because a linear ramp still moves the cache, still
  // re-shades, and is still a function of time. What distinguishes the kernel's trajectory from a ramp is
  // not that it moves, it is that it DECELERATES by a specific ratio -- so the page is compared against the
  // module at the same seeked times, in kelvin, to a tolerance far below the ramp's error.
  //
  // Measured on a correct page below; the ramp mutant misses by ~4 K at 3600 s.
  const pageVsKernel = [];
  // Seek, then WAIT UNTIL THE PAGE AGREES IT HAS SEEKED. A fixed sleep here was flaky on the first run --
  // two of five seeks read the previous frame's temperature and reported a 7 K disagreement on a correct
  // page. A check that fails at random is worse than one that is absent, because it trains the reader to
  // re-run rather than to look. So the seek is confirmed by the page's own `elapsedS` before its
  // temperature is believed.
  for (const t of [0, 600, 1800, 3600, 4800]) {
    const r = await page.evaluate(async (T) => {
      const o = window.__odderon;
      o.roomState.clockScale = 1e-9;
      o.roomState.curtainChangedAt = performance.now() - T * 1e9 * 1000;
      for (let i = 0; i < 40; i++) {
        o.requestFrame();
        await new Promise((res) => setTimeout(res, 50));
        if (Math.abs(o.roomState.elapsedS - T) < 1) break;   // the page has taken the seek
      }
      return { tK: o.roomState.temperatureK, elapsed: o.roomState.elapsedS };
    }, t);
    pageVsKernel.push({ t, page: r.tK, kernel: ex.room_temperature_k(t), elapsed: r.elapsed });
  }
  const worst = pageVsKernel.reduce((m, p) => Math.max(m, Math.abs(p.page - p.kernel)), 0);
  check(
    "the page's temperature IS the kernel's answer at the same t (not a ramp with the right endpoints)",
    worst < 0.05,
    `worst disagreement ${worst.toFixed(4)} K over ${pageVsKernel.length} seeked times ` +
      `[${pageVsKernel.map((p) => `${p.t}:${p.page.toFixed(1)}`).join(" ")}]`,
  );

  const f0 = await readAt(0);
  const f600 = await readAt(600);
  const fEnd = await readAt(7200);
  const d600 = cacheDiff(f0, f600);
  const dAll = cacheDiff(f0, fEnd);

  check(
    "the picture changes while the room cools (measured on the cache, not the panel)",
    d600.moved > 400000 && d600.maxd >= 2,
    `${d600.moved} of 840000 pixels differ over 0..600 s, up to ${d600.maxd} levels (${f0.tK.toFixed(1)} -> ${f600.tK.toFixed(1)} K)`,
  );
  check(
    "the whole trajectory is worth more than its first ten minutes",
    dAll.maxd > d600.maxd,
    `up to ${dAll.maxd} levels over the full 29.3 K against ${d600.maxd} over the first 5.9 K`,
  );

  // THE CACHE MUST BE INVALIDATED BY THE TEMPERATURE, which is the defect this gate was written blind to.
  // It was invisible from outside: the clock advanced, the panel counted, and every frame re-presented the
  // original image. So this asserts the thing that was actually broken -- a re-shade happened -- rather
  // than a consequence that could be produced by other means.
  const stale = await page.evaluate(async () => {
    const o = window.__odderon;
    o.roomState.clockScale = 240;
    o.roomState.curtainChangedAt = performance.now();
    o.cache.valid = false;
    const before = o.cache.fullShades;
    o.requestFrame();
    await new Promise((r) => setTimeout(r, 2500));        // ~600 simulated s at 240x
    return { before, after: o.cache.fullShades, elapsed: o.roomState.elapsedS, tK: o.roomState.temperatureK };
  });
  check(
    "the clock running invalidates the cache (the picture is re-shaded, not re-presented)",
    stale.after > stale.before + 1,
    `${stale.after - stale.before} full shades over ${Math.round(stale.elapsed)} simulated s, ending at ${stale.tK.toFixed(1)} K`,
  );

  // ...AND MUST STOP WHEN IT SETTLES. This is R2, and the clock is the one mechanism in this page that
  // could break it.
  //
  // THE RUN IS MEASURED IN SIMULATED TIME, NOT IN A GUESSED NUMBER OF SECONDS. The first version of this
  // waited 12 s and then measured -- correct when the run was 600 s at 60x, because that is ten seconds
  // of wall time. The run is now 4800 s at 240x, which is twenty, so it measured "idle" four hundred
  // simulated seconds before idle, counted 27 frames, and failed a page behaving exactly as designed.
  //
  // So: start a clean run at the SHIPPED rate, then poll until the page's own clock reports the end of
  // the window. The page owns that number (`__odderon.roomClock.settleS`); this check does not.
  await page.evaluate(() => {
    const o = window.__odderon;
    o.roomState.clockScale = 240;
    o.setCurtain(false);          // reset to a settled open room...
    o.setCurtain(true);           // ...then close it, which starts the clock at t = 0
    o.requestFrame();
  });
  const runStart = Date.now();
  let atRest = null;
  for (let i = 0; i < 120; i++) {
    await new Promise((r) => setTimeout(r, 500));
    atRest = await page.evaluate(() => ({
      elapsed: window.__odderon.roomState.elapsedS,
      tK: window.__odderon.roomState.temperatureK,
      settleS: window.__odderon.roomClock.settleS,
      row: document.getElementById("m-temp").textContent,
    }));
    if (atRest.elapsed >= atRest.settleS) break;
  }
  const runWall = (Date.now() - runStart) / 1000;
  check(
    "the room actually reached the end of its run before the idle measurement",
    atRest.elapsed >= atRest.settleS,
    `${Math.round(atRest.elapsed)} of ${atRest.settleS} simulated s in ${runWall.toFixed(0)} s of wall time, ` +
      `${atRest.tK.toFixed(1)} K, row "${atRest.row}"`,
  );
  // The observed rate, against the label, over the WHOLE run rather than over one second. This is a
  // weaker claim than the earlier measurement and a wider one: it catches a clock that drifts over
  // twenty seconds, which a 1.2 s sample cannot.
  check(
    "the clock's average rate over the whole run matches its label",
    Math.abs(atRest.elapsed / runWall - 240) / 240 < 0.15,
    `${(atRest.elapsed / runWall).toFixed(0)}x average against a stated 240x over ${runWall.toFixed(1)} s`,
  );

  // Now it is genuinely at rest, so idle must be free.
  const settledA = await page.evaluate(() => window.__odderon.bench.framesDrawn);
  await new Promise((r) => setTimeout(r, 2500));
  const settledB = await page.evaluate(() => window.__odderon.bench.framesDrawn);
  check(
    "the frame stops once the room has settled (idle is nothing scheduled)",
    settledB === settledA,
    `${settledB - settledA} frames drawn in 2.5 idle seconds after the full run`,
  );

  // THE ROOM HAS A MEMORY, and this is the bug that proved it did not.
  //
  // Before this, reopening the curtain teleported the room to the settled OPEN temperature in one frame:
  // measured 329.48 K -> 346.82 K, a **17.34 K** step, where a 120 kJ/K zone with UA 45 W/K needs about
  // forty minutes. The old code was not merely a jump at the toggle -- while the curtain was open the page
  // read `room_settled_k(0)` directly, so the room sat at its resting temperature forever and every
  // reopening discarded the cooling the user had just watched.
  const reopened = await page.evaluate(async () => {
    const o = window.__odderon;
    const before = o.roomState.temperatureK;
    o.setCurtain(false);
    o.requestFrame();
    await new Promise((r) => setTimeout(r, 150));
    return { before, after: o.roomState.temperatureK, from: o.roomState.fromK };
  });
  check(
    "reopening the curtain does not teleport the room to its open resting temperature",
    Math.abs(reopened.after - reopened.before) < 3.0,
    `${reopened.before.toFixed(1)} -> ${reopened.after.toFixed(1)} K (a 17.3 K jump is the old defect)`,
  );

  // ...and the warm-up must actually run, in the direction physics says: toward the OPEN resting
  // temperature, and it must still be short of it two seconds later. A page that parked at the settled
  // value would pass the assertion above and fail this one.
  const warmed = await page.evaluate(async () => {
    const o = window.__odderon;
    const a = o.roomState.temperatureK;
    await new Promise((r) => setTimeout(r, 6000));       // ~1440 simulated s at 240x
    return { a, b: o.roomState.temperatureK, settled: o.roomState.settledK, el: o.roomState.elapsedS };
  });
  check(
    "an open curtain now WARMS the room, toward the open resting temperature but short of it",
    warmed.b > warmed.a && warmed.b < warmed.settled - 0.5,
    `${warmed.a.toFixed(1)} -> ${warmed.b.toFixed(1)} K over ${Math.round(warmed.el)} simulated s, ` +
      `target ${warmed.settled.toFixed(1)} K`,
  );

  // ...and only once it has finished warming does the page go quiet again. This is R2 with the new law:
  // idle is nothing scheduled, and a warming room is not idle.
  const warmEnd = await page.evaluate(async () => {
    const o = window.__odderon;
    for (let i = 0; i < 200; i++) {
      if (!o.roomMoving()) break;
      await new Promise((r) => setTimeout(r, 250));
    }
    return { tK: o.roomState.temperatureK, settled: o.roomState.settledK };
  });
  const openA = await page.evaluate(() => window.__odderon.bench.framesDrawn);
  await new Promise((r) => setTimeout(r, 2500));
  const openB = await page.evaluate(() => window.__odderon.bench.framesDrawn);
  check(
    "once the room has finished warming, an open curtain schedules nothing",
    openB === openA,
    `${openB - openA} frames in 2.5 idle s at ${warmEnd.tK.toFixed(1)} K against ${warmEnd.settled.toFixed(1)} K`,
  );

  await page.close();
} catch (e) {
  fail.push(`the browser half threw: ${e && e.message ? e.message : e}`);
} finally {
  if (browser) await browser.close().catch(() => {});
  if (server) server.close();
}

report();
