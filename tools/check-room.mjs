#!/usr/bin/env node
/**
 * Claim 2, in the page: the room's light must be the thermal network's output.
 *
 * ## Why this exists
 *
 * `odderon_astro::thermal` has measured the bible's §6.7 room for a long time -- 1440 W open, 120 W
 * with the curtain closed, and an AC draw falling 713 W -> 304 W. **No pixel ever read it.** The claim
 * was true, tested, and invisible: the page rendered a wall and a crate, and the game had no room in it.
 *
 * The risk in wiring it up is not that the numbers are wrong. It is that the picture quietly stops
 * being a consequence of them. A brightness constant that happens to be 0.083 when the curtain closes
 * produces the same image as a solver, and every visual review passes. So this check asks the question
 * the image cannot answer by itself: **is the light coming from the kernel, or from a constant?**
 *
 *  1. The kernel must export the four room functions, and the page must depend on all four.
 *  2. The page must hold no thermal constant of its own -- no SHGC, no glazing area, no COP. A literal
 *     like 0.60 in `web/index.html` means somebody re-derived the physics in the presenter.
 *  3. The numbers the panel shows must equal the kernel's numbers, computed here from the same module.
 *  4. The curtain must go both ways: closing must drop solar gain AND the draw, and the draw drop must
 *     be SMALLER than the load drop, because ASHRAE's cyclic degradation makes a part-loaded unit
 *     worse. A ratio that equals the load ratio has lost that term.
 *  5. Toggling the curtain must actually change the pixels, and must invalidate the cache -- without
 *     that the room is shaded once, before the kernel answers, and every later frame blits the cold
 *     image back. That failure was measured here: the panel read "1440 W 100.0%" beside a pane
 *     rendered at 0 %.
 *
 * Usage: node tools/check-room.mjs [--port N]
 * Needs --wasm <path> or a wasm at dist/odderon_dmath_wasm.wasm; the kernel numbers are read from the
 * real module rather than restated in JavaScript, for the same reason every other check does.
 */

import { readFile } from "node:fs/promises";
import { createServer } from "node:http";
import { extname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { dirname } from "node:path";
import { resolveStaticPath } from "./static-path.mjs";

const HERE = dirname(fileURLToPath(import.meta.url));
const ROOT = join(HERE, "..");
const WEB_ROOT = join(ROOT, "web");

const args = process.argv.slice(2);
const portArg = args.indexOf("--port");
const port = portArg >= 0 ? Number(args[portArg + 1]) : 8431;

const checks = [];
const fail = (m) => checks.push(["FAIL", m]);
const ok = (m) => checks.push(["ok  ", m]);

// --- 1 & 2: the source contract, which needs no browser ----------------------------------------
const src = await readFile(join(WEB_ROOT, "index.html"), "utf8");

for (const fn of ["room_solar_w", "room_light_fraction", "room_cooling_w", "room_draw_w"]) {
  if (!src.includes(fn)) fail(`web/index.html does not use the kernel export \`${fn}\``);
}
if (checks.every(([t]) => t === "ok  ")) ok("the page depends on all four room exports");

// The presenter must not own the physics. These are the numbers that would mean it does.
const forbidden = [
  [/\b0\.60\b/, "0.60", "SHGC"],
  [/\b0\.05\b(?!\s*\))/, "0.05", "the curtain's SHGC"],
  [/\b1440\b/, "1440", "the open-window solar gain"],
  [/\b800\b\s*\*\s*3/, "800 * 3", "irradiance x glazing area"],
  [/\bCOP\b/, "COP", "a COP"],
];
for (const [re, lit, what] of forbidden) {
  // Only the shipped page; comments are allowed to discuss the numbers they explain.
  const code = src.replace(/\/\/[^\n]*/g, "").replace(/\/\*[\s\S]*?\*\//g, "");
  if (re.test(code)) fail(`web/index.html computes ${what} itself (literal ${lit}) -- the kernel owns it`);
}
if (checks.every(([t]) => t === "ok  ")) ok("the page holds no thermal constant of its own");

// The invalidation, which is the defect this check found first.
if (!/function setCurtain[\s\S]{0,400}cache\.valid = false/.test(src)) {
  fail("setCurtain() does not invalidate the cache -- the room will render once, before the kernel answers");
} else {
  ok("setCurtain() invalidates the cache");
}

// --- 3 & 4: the real numbers, from the real module ---------------------------------------------
const wasmPath = join(ROOT, "dist", "odderon_dmath_wasm.wasm");
let k = null;
try {
  const bytes = await readFile(wasmPath);
  const { instance } = await WebAssembly.instantiate(bytes, {});
  k = instance.exports;
} catch (e) {
  fail(`could not load the kernel at dist/odderon_dmath_wasm.wasm: ${e.message}`);
}

if (k) {
  const solarOpen = k.room_solar_w(0);
  const solarShut = k.room_solar_w(1);
  const drawOpen = k.room_draw_w(0);
  const drawShut = k.room_draw_w(1);
  const coolOpen = k.room_cooling_w(0);
  const coolShut = k.room_cooling_w(1);
  const lightOpen = k.room_light_fraction(0);
  const lightShut = k.room_light_fraction(1);

  // The bible's arithmetic: 800 W/m2 x 3 m2 x SHGC.
  const near = (a, b, tol) => Math.abs(a - b) <= tol;
  if (!near(solarOpen, 1440, 1)) fail(`open solar gain is ${solarOpen.toFixed(1)} W, not 1440 W`);
  else ok(`open solar gain ${solarOpen.toFixed(1)} W (bible 1440)`);
  if (!near(solarShut, 120, 1)) fail(`closed solar gain is ${solarShut.toFixed(1)} W, not 120 W`);
  else ok(`closed solar gain ${solarShut.toFixed(1)} W (bible 120)`);

  // 713 / 304 are the numbers BUILD-PLAN ships. They are pinned because a drop that is right for the
  // wrong reason is the failure the thermal tests exist to catch, and the same applies here.
  if (!near(drawOpen, 713, 2)) fail(`AC draw open is ${drawOpen.toFixed(1)} W, not 713 W`);
  else ok(`AC draw open ${drawOpen.toFixed(1)} W (plan 713)`);
  if (!near(drawShut, 304, 2)) fail(`AC draw closed is ${drawShut.toFixed(1)} W, not 304 W`);
  else ok(`AC draw closed ${drawShut.toFixed(1)} W (plan 304)`);

  const drawDrop = 1 - drawShut / drawOpen;
  const loadDrop = 1 - coolShut / coolOpen;
  if (!(drawDrop > 0.5)) fail(`closing the curtain drops the draw by only ${(drawDrop * 100).toFixed(1)}%`);
  else ok(`closing the curtain drops the AC draw ${(drawDrop * 100).toFixed(2)}%`);

  // THE DEGRADATION TERM. A cycling unit is worse at part load, so the electrical drop must be
  // SMALLER than the cooling-load drop. If they are equal the cyclic degradation coefficient has been
  // lost and the "conservative direction" the plan claims no longer exists.
  if (!(drawDrop < loadDrop)) {
    fail(
      `the draw drop (${(drawDrop * 100).toFixed(2)}%) is not smaller than the load drop ` +
        `(${(loadDrop * 100).toFixed(2)}%), so ASHRAE's cyclic degradation term is missing`,
    );
  } else {
    ok(`the draw drop is smaller than the load drop (${(loadDrop * 100).toFixed(2)}%), degradation present`);
  }

  // The shader's input is a RATIO, and it must be the SHGC ratio and nothing else.
  if (!near(lightOpen, 1.0, 1e-9)) fail(`open light fraction is ${lightOpen}, not 1`);
  else ok(`open light fraction 1.0`);
  if (!near(lightShut, 0.05 / 0.60, 1e-6)) {
    fail(`closed light fraction is ${lightShut}, not the SHGC ratio ${(0.05 / 0.6).toFixed(6)}`);
  } else {
    ok(`closed light fraction ${lightShut.toFixed(6)} = SHGC ratio 0.05/0.60`);
  }
}

// --- 5: the picture, in a real page -------------------------------------------------------------
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
    defaultViewport: { width: 900, height: 700, deviceScaleFactor: 1 },
    headless: true,
  });
  const page = await browser.newPage();
  await page.goto(`http://127.0.0.1:${port}/`, { waitUntil: "networkidle0", timeout: 90000 });
  await new Promise((r) => setTimeout(r, 2500));

  const open = await page.evaluate(() => ({
    solar: document.getElementById("m-solar").textContent,
    draw: document.getElementById("m-draw").textContent,
    curtain: document.getElementById("m-curtain").textContent,
    light: window.__odderon.roomState.lightFraction,
    png: null,
  }));

  if (!/kernel|W/.test(open.solar)) fail(`the room row reads "${open.solar}" -- the kernel did not answer`);
  else ok(`the panel reports the room from the kernel: "${open.solar}"`);

  // THE VALUES, NOT JUST THE PRESENCE OF A NUMBER.
  //
  // An earlier version of this check stopped at "the row contains a W", and a page that hardcoded
  // `lightFraction = curtain ? 0.083 : 1.0` -- a decorative room with no kernel behind it -- passed
  // every assertion. Measured: it produced the same picture, the same text, and the same pixel change.
  // The whole point of this file is that a plausible picture is not evidence, so the panel's numbers
  // are compared against the module's, computed independently above.
  const wantOpenSolar = k ? k.room_solar_w(0) : 1440;
  const wantOpenDraw = k ? k.room_draw_w(0) : 713;
  const num = (t) => {
    const m = /([\d.]+)\s*W/.exec(t || "");
    return m ? Number(m[1]) : NaN;
  };
  if (Math.abs(num(open.solar) - wantOpenSolar) > 1) {
    fail(`the panel reports ${num(open.solar)} W of solar gain; the kernel says ${wantOpenSolar.toFixed(1)} W`);
  } else {
    ok(`the panel's solar gain equals the kernel's: ${num(open.solar)} W`);
  }
  if (Math.abs(num(open.draw) - wantOpenDraw) > 1) {
    fail(`the panel reports ${num(open.draw)} W of AC draw; the kernel says ${wantOpenDraw.toFixed(1)} W`);
  } else {
    ok(`the panel's AC draw equals the kernel's: ${num(open.draw)} W`);
  }
  // And the shader's own input, which is what actually colours the pixels.
  const wantLightOpen = k ? k.room_light_fraction(0) : 1;
  if (Math.abs(open.light - wantLightOpen) > 1e-6) {
    fail(`the shader input is ${open.light}; the kernel's ratio is ${wantLightOpen}`);
  } else {
    ok(`the shader input equals the kernel's ratio: ${open.light}`);
  }
  if (open.curtain !== "open") fail(`the curtain row reads "${open.curtain}" before anything was clicked`);

  // Toggling must change the picture, not just the text.
  const shot = () => page.screenshot({ encoding: "base64" });
  const before = await shot();
  await page.evaluate(() => document.getElementById("curtain").click());
  await new Promise((r) => setTimeout(r, 1200));
  const after = await shot();

  const closed = await page.evaluate(() => ({
    solar: document.getElementById("m-solar").textContent,
    draw: document.getElementById("m-draw").textContent,
    curtain: document.getElementById("m-curtain").textContent,
    light: window.__odderon.roomState.lightFraction,
  }));

  const differing = await page.evaluate(async (a, b) => {
    const load = (d) =>
      new Promise((s, j) => {
        const i = new Image();
        i.onload = () => s(i);
        i.onerror = j;
        i.src = "data:image/png;base64," + d;
      });
    const [ia, ib] = await Promise.all([load(a), load(b)]);
    const px = (img) => {
      const c = document.createElement("canvas");
      c.width = img.width;
      c.height = img.height;
      c.getContext("2d").drawImage(img, 0, 0);
      return c.getContext("2d").getImageData(0, 0, img.width, img.height).data;
    };
    const A = px(ia);
    const B = px(ib);
    let n = 0;
    for (let i = 0; i < A.length; i += 4) {
      if (Math.abs(A[i] - B[i]) > 4 || Math.abs(A[i + 1] - B[i + 1]) > 4) n++;
    }
    return n;
  }, before, after);

  // The window is 1.15 x 1.25 units at ~9 units from the camera, so it covers tens of thousands of
  // pixels; requiring 500 means "the pane changed", not "a few HUD glyphs reflowed".
  if (differing < 500) {
    fail(`toggling the curtain changed only ${differing} pixels -- the picture is not reading the room`);
  } else {
    ok(`toggling the curtain changed ${differing} pixels of the picture`);
  }

  if (closed.curtain !== "closed") fail(`the curtain row reads "${closed.curtain}" after closing it`);
  else ok(`the panel follows the curtain: "${closed.solar}" / "${closed.draw}"`);
  // EXACTLY the kernel's closed value, not merely "smaller". A decorative constant tuned to look
  // right lands within 0.5 % of 0.0833 by luck; it does not land within 1e-6.
  const wantLightShut = k ? k.room_light_fraction(1) : 0.05 / 0.6;
  if (Math.abs(closed.light - wantLightShut) > 1e-6) {
    fail(`closed shader input is ${closed.light}; the kernel's ratio is ${wantLightShut}`);
  } else {
    ok(`the shader's input is the kernel's ratio, both ways: ${open.light} -> ${closed.light}`);
  }

  await page.close();
} catch (e) {
  fail(`the browser half threw: ${e && e.message ? e.message : e}`);
} finally {
  if (browser) await browser.close().catch(() => {});
  if (server) server.close();
}

for (const [tag, msg] of checks) console.log(`  ${tag} ${msg}`);
console.log("");
if (checks.some(([t]) => t === "FAIL")) {
  console.log("room check FAILED");
  process.exit(1);
}
console.log("room check passed");
