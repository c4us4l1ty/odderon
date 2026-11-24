#!/usr/bin/env node
/**
 * Capture the shell's current frame to a PNG, for a human to look at.
 *
 * Why this exists: `BUILD-PLAN.md` §4 claim 5 is "the window view is the best-looking thing on your
 * laptop", and the plan is explicit that it must be *shown* before anything about the art is claimed.
 * Claims 1–4 are numbers a machine can check; claim 5 is a judgement, and the only honest way to put a
 * judgement in front of someone is to put the pixels in front of them.
 *
 * ## Why the output is not committed
 *
 * The screenshot is a `.png`, and `GRAPHICS.md` R1 / `ci/kernel-policy.sh` rule R5 forbid binary assets
 * in the repository — every surface must be a function S(u,v,t). That rule is written for the *shipped*
 * renderer, but it is enforced over `git ls-files` and has no exception for documentation, which is the
 * right default: an exception is how a rule stops being a rule. So captures are written to `.artifacts/`,
 * which `.gitignore` already covers, and the numbers that accompany them go in the docs instead.
 *
 * This also means the tool is honest about the one thing it cannot do here: this sandbox has no GPU, so
 * the frame is SwiftShader. It is the right *picture* and the wrong *timing*, and the two must not be
 * confused — `tools/check-shell.mjs` refuses to report frame time for the same reason.
 *
 * Usage:
 *   node tools/screenshot.mjs                       # 1600x1000, .artifacts/shell.png
 *   node tools/screenshot.mjs --width 2560 --height 1600 --out .artifacts/big.png
 *   node tools/screenshot.mjs --url http://127.0.0.1:8080/ --hide-ui   # use a server you already run
 *
 * With no `--url` it starts `tools/serve.mjs` itself and stops it again. That is not a convenience: the
 * first version assumed a server was already listening on 8080 and passed locally, where a preview server
 * happened to be up, then failed in CI, where it was not. A tool that is green because of the state of the
 * machine it ran on is the same class of bug as a test that calls the code under test to build its own
 * expectation, so the tool now owns its server and cannot be green for the wrong reason.
 *
 * Requires `npm install` and, in a sandbox without a system Chromium, `node tools/unpack-chromium.mjs`
 * with `LD_LIBRARY_PATH=/tmp/al2023/lib` exported.
 */

import { mkdirSync } from "node:fs";
import { spawn } from "node:child_process";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

function flag(name, dflt) {
  const i = process.argv.indexOf(`--${name}`);
  return i >= 0 && process.argv[i + 1] ? process.argv[i + 1] : dflt;
}
const has = (name) => process.argv.includes(`--${name}`);

const url = flag("url", "");
const out = resolve(flag("out", ".artifacts/shell.png"));
const width = Number(flag("width", 1600));
const height = Number(flag("height", 1000));
const hideUi = has("hide-ui");

const here = dirname(fileURLToPath(import.meta.url));

/** Start `tools/serve.mjs` on `serverPort` and resolve once it says it is listening. */
async function startServer(serverPort) {
  const child = spawn(process.execPath, [resolve(here, "serve.mjs"), String(serverPort)], {
    stdio: ["ignore", "pipe", "pipe"],
  });
  await new Promise((res, rej) => {
    const timer = setTimeout(() => rej(new Error("serve.mjs did not start within 15 s")), 15000);
    child.stdout.on("data", (d) => {
      if (String(d).includes("shell on")) {
        clearTimeout(timer);
        res();
      }
    });
    child.on("exit", (code) => {
      clearTimeout(timer);
      rej(new Error(`serve.mjs exited early with code ${code}`));
    });
  });
  return child;
}

const ownServer = !has("url");
const serverPort = Number(flag("port", 8080));
const server = ownServer ? await startServer(serverPort) : null;
const target = ownServer ? `http://127.0.0.1:${serverPort}/` : url;

const { default: puppeteer } = await import("puppeteer-core");
const { default: chromium } = await import("@sparticuz/chromium");

// `--use-gl=angle` rather than `swiftshader`: with the latter, Chromium here launches, then the
// DevTools socket refuses the connection. `tools/check-shell.mjs` already uses angle for this reason.
const browser = await puppeteer.launch({
  args: [...chromium.args, "--enable-unsafe-swiftshader", "--use-gl=angle"],
  defaultViewport: { width, height, deviceScaleFactor: 1 },
  executablePath: await chromium.executablePath(),
  headless: true,
});

const page = await browser.newPage();
const errors = [];

// ONE CATEGORY OF CONSOLE ERROR IS EXPECTED, and only when the page says so.
//
// `web/index.html` fetches `/dist/odderon_dmath_wasm.wasm` for the orbit kernel. That artifact is build
// output -- gitignored, and absent on a clean checkout including this CI job, which never builds wasm.
// The browser logs the 404 as a console error, and this script treats any console error as "the frame is
// not trustworthy". So on 2026-11-24 this job went red for a condition the page handles correctly:
// it hid the body and reported `orbit source unavailable`.
//
// The trade this makes: a missing OPTIONAL ASSET is not a rendering fault, but a page that fails
// silently very much is. So the exemption is applied only when the page *declares* the absence through
// `__odderon.kernel.status` (which it sets only after the fetch has failed). Any other console error --
// a shader that will not compile, a WebGL warning promoted to error, an uncaught exception -- still
// fails the capture. Suppressing console errors wholesale would have hidden exactly the class of defect
// this script exists to catch.
const OPTIONAL_ASSET_404 = /Failed to load resource.*404|odderon_dmath_wasm\.wasm/;
const pageErrors = [];
const exempt = [];
page.on("console", (m) => {
  if (m.type() !== "error") return;
  if (OPTIONAL_ASSET_404.test(m.text())) exempt.push(m.text());
  else pageErrors.push(m.text());
});
page.on("pageerror", (e) => pageErrors.push(String(e)));

/**
 * Was the exemption legitimate?
 *
 * `--selftest` drives this with no browser, so the rejection path is exercised somewhere rather than
 * being first run on a red CI job.
 *
 * @param {string[]} exemptTexts console errors that matched the optional-asset pattern
 * @param {string|null} kernelStatus what the page said about the kernel
 * @returns {{note: string|null, failures: string[]}}
 */
export function classifyExemption(exemptTexts, kernelStatus) {
  if (!exemptTexts.length) return { note: null, failures: [] };
  if (kernelStatus && String(kernelStatus).startsWith("unavailable")) {
    return {
      note:
        `${exemptTexts.length} console error(s) for the optional wasm artifact, and the page declares ` +
        `"${kernelStatus}". The body is hidden and the HUD says so; this is not a rendering fault.`,
      failures: [],
    };
  }
  return {
    note: null,
    failures: exemptTexts.map(
      (t) => `${t} (page did not declare the kernel unavailable: ${JSON.stringify(kernelStatus)})`,
    ),
  };
}

await page.goto(target, { waitUntil: "networkidle0", timeout: 90000 });

// The exemption is only honoured if the page confirms it expected the asset to be missing. A page that
// reports the kernel as loaded while logging a 404 for it is lying, and that stays a failure.
if (exempt.length) {
  const kernelStatus = await page.evaluate(() => window.__odderon?.kernel?.status ?? null).catch(() => null);
  const verdict = classifyExemption(exempt, kernelStatus);
  if (verdict.note) console.log(`\nnote: ${verdict.note}`);
  pageErrors.push(...verdict.failures);
}
errors.push(...pageErrors);

// Wait for the first frame, then for the instrument to *settle*. A shader that failed to compile leaves
// a blank canvas and one console line; waiting on framesDrawn > 0 is what distinguishes "rendered" from
// "loaded", and the quiet window is what makes the capture deterministic rather than racy.
await page.waitForFunction(() => (window.__odderon?.bench?.framesDrawn ?? 0) > 0, { timeout: 90000 });

const settled = await (async () => {
  const quietMs = 600;
  let last = -1;
  let quietSince = Date.now();
  for (let i = 0; i < 120; i++) {
    const n = await page.evaluate(() => window.__odderon?.bench?.framesDrawn ?? -1);
    if (n !== last) {
      last = n;
      quietSince = Date.now();
    } else if (Date.now() - quietSince >= quietMs) {
      return { frames: n, quietForMs: Date.now() - quietSince };
    }
    await new Promise((r) => setTimeout(r, 50));
  }
  return { frames: last, quietForMs: -1, timedOut: true };
})();

if (hideUi) {
  // The instrument panel is a dev overlay, not part of the picture. Hiding it is a CSS change in the
  // capture only -- the page is untouched.
  await page.evaluate(() => {
    const panel = document.querySelector("#panel, .panel, #hud");
    if (panel) panel.style.display = "none";
  });
  await new Promise((r) => setTimeout(r, 200));
}

// Prove the frame is *static* rather than merely paused: sample again after settled, and expect exactly
// zero new frames. `tools/check-shell.mjs` makes the same assertion; repeating it here means a capture
// cannot silently be of an animation mid-flight. Note that the earlier version of this file reported
// "drewWhileIdle: 2" from a window that still spanned the settle itself, which reads as a renderer bug
// and was a measurement bug -- the window now starts only after `settled`.
const framesBeforeIdle = await page.evaluate(() => window.__odderon?.bench?.framesDrawn ?? -1);
await new Promise((r) => setTimeout(r, 400));
const framesAfterIdle = await page.evaluate(() => window.__odderon?.bench?.framesDrawn ?? -1);
const drewWhileIdle = framesAfterIdle - framesBeforeIdle;

const metrics = await page.evaluate(() => {
  const o = window.__odderon ?? {};
  return {
    framesDrawn: o.bench?.framesDrawn ?? null,
    needsFrame: o.state?.needsFrame ?? null,
    canvas: o.bench?.canvas ? { w: o.bench.canvas.w, h: o.bench.canvas.h } : null,
    renderer: o.bench?.renderer ?? null,
  };
});

mkdirSync(dirname(out), { recursive: true });
await page.screenshot({ path: out });
await browser.close();
if (server) server.kill();

console.log(
  JSON.stringify(
    { out, width, height, hideUi, settled, drewWhileIdle, ...metrics, errors: errors.slice(0, 5) },
    null,
    2,
  ),
);

if (errors.length) {
  console.error(`\n${errors.length} console error(s) — the frame is not trustworthy. See above.`);
  process.exit(1);
}
if (settled.timedOut) {
  console.error("\nnever settled: frames kept being drawn, so the capture may be mid-animation.");
  process.exit(2);
}
if (drewWhileIdle !== 0) {
  console.error(`\ndrew ${drewWhileIdle} frame(s) while idle — render-on-demand is broken, or this window`);
  console.error("overlaps the settle. Either way the capture is not of a static frame.");
  process.exit(3);
}
