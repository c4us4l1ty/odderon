#!/usr/bin/env node
/**
 * When the kernel is absent, the page must SAY SO — visibly, specifically, and with a way to fix it.
 *
 * ## The failure this exists for
 *
 * `dist/` is git-ignored, so a page served straight from a clone has no `odderon_dmath_wasm.wasm`. That
 * is not a bug; it is the build-output rule. The bug was the report: the panel said `— no kernel`, the
 * room rendered as a hole onto a flat horizon, and a reader reasonably concluded that THIS WAS THE ROOM.
 *
 * That is the worst kind of finding, because it is unfalsifiable from the picture: a hole onto a bright
 * horizon looks exactly like a room with no information in it. The claim "the panel's numbers come from
 * the thermal network" was true, gated, and completely invisible to the person looking at the frame.
 *
 * Two things were wrong and both are assertions now:
 *
 *   1. The absence must be attached to the PICTURE, not only to a panel row. A banner, visible in the
 *      frame, that says the room is not there and why.
 *   2. The absence must be ACTIONABLE. "no kernel" does not tell a reader whether the module crashed, was
 *      stale, or was never built. It must name the artifact, the reason it is missing, and the command.
 *
 * Measured before the fix, with `dist/odderon_dmath_wasm.wasm` removed and the page served from a clone:
 * no banner existed at all, and the room rows read `— no kernel` / `unknown (no kernel)` — which names
 * neither the artifact nor a remedy.
 *
 * Usage: node tools/check-missing-kernel.mjs [--port N]
 * Needs a built wasm at dist/ so it can hide it; skips with a clear note if there is none.
 */

import { readFile, rename, stat } from "node:fs/promises";
import { createServer } from "node:http";
import { extname, join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { resolveStaticPath } from "./static-path.mjs";

const HERE = dirname(fileURLToPath(import.meta.url));
const ROOT = join(HERE, "..");
const WASM = join(ROOT, "dist", "odderon_dmath_wasm.wasm");

const args = process.argv.slice(2);
const portArg = args.indexOf("--port");
const port = portArg >= 0 ? Number(args[portArg + 1]) : 8751;

const fail = [];
const pass = [];
const check = (name, ok, detail = "") => (ok ? pass : fail).push(detail ? `${name}  ${detail}` : name);

// --- the source contract, no browser needed ----------------------------------------------------
const src = await readFile(join(ROOT, "web", "index.html"), "utf8");

check("the page has an element that reports a missing kernel", /id="nokernel"/.test(src));
check(
  "the banner names the artifact",
  /odderon_dmath_wasm\.wasm/.test(src),
  "a reader has to know WHICH file is missing",
);
check(
  "the banner names the reason it is absent from a clone",
  /git-ignored|build output/.test(src),
  "'no kernel' reads as a defect; 'a git-ignored build output' reads as a build step",
);
check(
  "the banner names a remedy",
  /fetch:wasm/.test(src) && /cargo build/.test(src),
  "at least one command, ideally both routes",
);
check(
  "the room rows distinguish 'not built' from 'waiting'",
  /kernel not built/.test(src) && /waiting for kernel/.test(src),
  "one state is a checkout fact, the other is a transient",
);

// --- the live behaviour: hide the module and look ----------------------------------------------
let hidden = null;
try {
  await stat(WASM);
} catch {
  console.log("  skip: no dist/odderon_dmath_wasm.wasm to hide — the live half cannot run");
  console.log("        (this is the same absence the check is about, so it is not a pass)");
  for (const m of fail) console.log(`  FAIL ${m}`);
  for (const m of pass) console.log(`  ok   ${m}`);
  console.log("\nmissing-kernel check INCOMPLETE");
  process.exit(1);
}

let browser = null;
let server = null;
try {
  await rename(WASM, WASM + ".hidden");
  hidden = WASM + ".hidden";

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
    defaultViewport: { width: 1400, height: 800 },
    headless: true,
  });
  const page = await browser.newPage();
  await page.goto(`http://127.0.0.1:${port}/`, { waitUntil: "networkidle0", timeout: 90000 });
  await new Promise((r) => setTimeout(r, 2200));

  const seen = await page.evaluate(() => {
    const nk = document.getElementById("nokernel");
    const box = nk ? nk.getBoundingClientRect() : null;
    return {
      hidden: nk ? nk.hidden : true,
      visiblePx: box ? Math.max(0, box.width) * Math.max(0, box.height) : 0,
      text: nk ? nk.textContent : "",
      solar: document.getElementById("m-solar")?.textContent ?? "",
      curtain: document.getElementById("m-curtain")?.textContent ?? "",
      kernel: window.__odderon?.kernel?.status ?? "",
      // Is the banner actually IN the frame the user is looking at? A fixed element can be laid out
      // off-screen, or under the canvas, and still pass a `hidden === false` test.
      inViewport: box ? box.top < window.innerHeight && box.bottom > 0 && box.width > 200 : false,
      // The two rectangles whose RELATIONSHIP was the bug: the panel's opaque box and the banner's left
      // edge. A banner that starts inside the panel is a banner whose first words nobody can read.
      bannerLeft: box ? box.left : NaN,
      hudRight: document.getElementById("hud")?.getBoundingClientRect().right ?? NaN,
    };
  });

  check("the kernel really is absent for this run", /unavailable/.test(seen.kernel), seen.kernel);
  check("the banner is shown", seen.hidden === false);
  check("the banner occupies the frame", seen.inViewport && seen.visiblePx > 20000, `${Math.round(seen.visiblePx)} px²`);
  // THE BANNER MUST NOT SAY THE ROOM IS ABSENT, because the room is not absent: the geometry is drawn by
  // the same shader either way, and claiming otherwise is a second false statement beside the true one.
  // It said "there is no room in this picture" until a screenshot showed the room plainly in it. What is
  // missing is the MEASUREMENT, so that is what the banner has to name.
  // AND THE ROOM MUST STILL BE DRAWN. This is the check that would have caught the worst bug of the clock
  // work: the shader's new `(T/T0)^4` term is multiplied into the ambient, and with no kernel the host
  // sends 0.0 for the temperature, which made the ambient zero. Measured: mean scene luma **123.1 with the
  // kernel, 27.5 without** -- a black room with a banner over it, asserting this room is at absolute zero.
  // Nothing here noticed, because every assertion above reads the DOM. The panel was correct throughout.
  const noKernelLuma = await page.evaluate(async () => {
    const o = window.__odderon, gl = o.gl, c = o.cache;
    o.requestFrame();
    await new Promise((r) => setTimeout(r, 500));
    const buf = new Uint8Array(c.w * c.h * 4);
    const prev = gl.getParameter(gl.READ_FRAMEBUFFER_BINDING);
    gl.bindFramebuffer(gl.READ_FRAMEBUFFER, c.fbo);
    gl.readPixels(0, 0, c.w, c.h, gl.RGBA, gl.UNSIGNED_BYTE, buf);
    gl.bindFramebuffer(gl.READ_FRAMEBUFFER, prev);
    let L = 0, n = 0;
    for (let y = 0; y < c.h; y++) for (let x = Math.round(c.w * 0.30); x < c.w; x++) {
      const i = (y * c.w + x) * 4;
      L += (buf[i] + buf[i + 1] + buf[i + 2]) / 3;
      n++;
    }
    return L / n;
  });
  // The band is wide and low on purpose. This is not a judgement about how a room should look without a
  // kernel; it is the difference between "the room is drawn" and "the room is black", and the measured
  // values are 114.3 and 27.5. Anything between those two is a bug this check cannot rank.
  check(
    "the room is still drawn without a kernel (no absolute-zero ambient)",
    noKernelLuma > 80,
    `mean scene luma ${noKernelLuma.toFixed(1)} (drawn ~114, black-room bug ~27)`,
  );

  check(
    "the banner names the measurement as what is missing, not the room",
    /no numbers|not.*measured|— kernel not built/i.test(seen.text) && !/no room in this picture/i.test(seen.text),
    seen.text.slice(0, 80),
  );
  check("the banner does not claim the geometry is missing", !/no room/i.test(seen.text));

  // THE BANNER MUST NOT BE PAINTED OVER BY THE PANEL. It spans the full width at the bottom of the window
  // and the panel is a fixed opaque box at the left, so its first 348 px sat UNDERNEATH the panel -- and
  // the headline rendered as "s no room in this picture.", the opening words simply painted out. A
  // screenshot is what showed it; a DOM query would not have, because the text node was intact and
  // correctly laid out. So this compares the two RECTANGLES, which is the property that was violated.
  check(
    "the banner starts where the panel ends, so no line is painted over",
    seen.bannerLeft >= seen.hudRight - 0.5,
    `banner left ${Math.round(seen.bannerLeft)} px, panel right ${Math.round(seen.hudRight)} px`,
  );
  check("the banner names the artifact", /odderon_dmath_wasm\.wasm/.test(seen.text), "");
  check("the banner names a remedy", /fetch:wasm/.test(seen.text), "");
  check("the room row says the kernel was not built", /kernel not built/.test(seen.solar), seen.solar);
  check("the curtain row does not claim a state", /unknown/.test(seen.curtain), seen.curtain);

  await page.close();
} catch (e) {
  fail.push(`the browser half threw: ${e && e.message ? e.message : e}`);
} finally {
  if (browser) await browser.close().catch(() => {});
  if (server) server.close();
  if (hidden) await rename(hidden, WASM).catch(() => {});
}

for (const m of pass) console.log(`  ok   ${m}`);
for (const m of fail) console.log(`  FAIL ${m}`);
console.log("");
if (fail.length) {
  console.log("missing-kernel check FAILED");
  process.exit(1);
}
console.log("missing-kernel check passed");
