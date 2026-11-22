#!/usr/bin/env node
/**
 * Print the WebGL2 renderer string in this sandbox.
 *
 * Its purpose is to prevent a specific, expensive mistake: taking a screenshot here and believing it
 * says anything about performance. WebGL2 in this container runs on **SwiftShader**, a CPU
 * rasteriser. That means:
 *
 *   correct pixels  -> trustworthy
 *   frame time      -> meaningless (it is a software rasteriser pretending to be a GPU)
 *
 * Real performance numbers come only from Reference A / Reference B (docs/GRAPHICS.md §1), measured
 * on the user's hardware. See docs/BUILD-PLAN.md §0 fact 3.
 *
 * Usage: node tools/gl-info.mjs
 */

import { existsSync, readdirSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { execSync } from "node:child_process";

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, "..");

function findChromiumLibs() {
  // @sparticuz/chromium ships its shared objects as a brotli'd tar inside the package. Rather than
  // depend on undocumented internals, look for an already-extracted directory first and tell the
  // user exactly how to make one if it is absent.
  const candidates = ["/tmp/al2023/lib", "/tmp/chromium-libs/lib"];
  return candidates.find((p) => existsSync(p)) ?? null;
}

const libs = findChromiumLibs();
if (!libs) {
  console.error("Chromium shared libraries are not extracted yet. Run this once:");
  console.error("");
  console.error("  node tools/unpack-chromium.mjs");
  console.error("");
  process.exit(1);
}

const chromium = (await import("@sparticuz/chromium")).default;
const puppeteer = (await import("puppeteer-core")).default;

const executablePath = await chromium.executablePath();
const browser = await puppeteer.launch({
  args: [
    ...chromium.args,
    "--use-gl=angle",
    "--use-angle=swiftshader",
    "--enable-unsafe-swiftshader",
    "--no-sandbox",
  ],
  executablePath,
  headless: true,
  env: { ...process.env, LD_LIBRARY_PATH: libs },
});

try {
  const page = await browser.newPage();
  const info = await page.evaluate(() => {
    const gl = document.createElement("canvas").getContext("webgl2");
    if (!gl) return { webgl2: false };
    const dbg = gl.getExtension("WEBGL_debug_renderer_info");
    const maxTexture = gl.getParameter(gl.MAX_TEXTURE_SIZE);
    return {
      webgl2: true,
      vendor: dbg ? gl.getParameter(dbg.UNMASKED_VENDOR_WEBGL) : "unknown",
      renderer: dbg ? gl.getParameter(dbg.UNMASKED_RENDERER_WEBGL) : gl.getParameter(gl.RENDERER),
      version: gl.getParameter(gl.VERSION),
      glsl: gl.getParameter(gl.SHADING_LANGUAGE_VERSION),
      maxTexture,
      extensions: gl.getSupportedExtensions()?.length ?? 0,
    };
  });

  console.log(JSON.stringify(info, null, 2));

  if (info.webgl2 && /swiftshader/i.test(String(info.renderer))) {
    console.log(
      "\nNOTE: this is a software rasteriser. Correctness yes, performance no.\n" +
        "Frame-time claims must come from Reference A/B (docs/BUILD-PLAN.md §0)."
    );
  }
} finally {
  await browser.close();
}
