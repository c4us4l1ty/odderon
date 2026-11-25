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
    };
  });

  check("the kernel really is absent for this run", /unavailable/.test(seen.kernel), seen.kernel);
  check("the banner is shown", seen.hidden === false);
  check("the banner occupies the frame", seen.inViewport && seen.visiblePx > 20000, `${Math.round(seen.visiblePx)} px²`);
  check("the banner says the room is not in the picture", /no room in this picture/i.test(seen.text));
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
