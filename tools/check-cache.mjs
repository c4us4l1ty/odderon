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
import { dirname, extname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import puppeteer from "puppeteer-core";
import chromium from "@sparticuz/chromium";

const here = dirname(fileURLToPath(import.meta.url));
const webRoot = resolve(here, "..", "web");
const MIME = { ".html": "text/html; charset=utf-8", ".js": "text/javascript", ".mjs": "text/javascript" };

const server = createServer(async (req, res) => {
  try {
    const p = req.url === "/" ? "/index.html" : req.url.split("?")[0];
    const b = await readFile(join(webRoot, p));
    res.writeHead(200, { "content-type": MIME[extname(p)] ?? "text/plain" }).end(b);
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
