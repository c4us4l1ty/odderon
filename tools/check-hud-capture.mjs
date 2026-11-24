#!/usr/bin/env node
/**
 * The saved picture's instrument panel must be the panel a person sees.
 *
 * ## Why this exists
 *
 * `capturePicture()` rasterises the HTML panel into the saved PNG through an SVG `<foreignObject>`. For
 * as long as that block has existed it serialised `#hud` **without the document's stylesheet**, so the
 * panel was rasterised with no CSS at all: the flex rows collapsed into one unstyled run of text and
 * values printed on top of their own labels —
 *
 *     R2 idle < 1.5 m5 ms gpu blit < 1.5
 *
 * Every check in this repository passed, because every check looks at the **page**, and the page was
 * always correct. The artifact a person actually judges — the PNG that claim 5 is argued from — was the
 * broken one. That is the failure this file is arranged against: not "the code is wrong", but "the thing
 * you look at disagrees with the thing the code did".
 *
 * ## What it measures
 *
 * Two viewports — desktop at dpr 1 and **a phone at dpr 2** — because the second defect below only
 * appears above dpr 1, and the person who judges claim 5 judges it from a phone. At each, it captures:
 *
 *   1. `page.screenshot()` of the live DOM  — what a person sees on screen
 *   2. the panel composited into the saved PNG — what a person sees in the artifact
 *
 * and compares the **text** in the same region (the render-scale button block: static labels, so a
 * changing number cannot confuse it).
 *
 * ## Two defects, and why the metric took three attempts
 *
 * There were two independent faults, and the second was only found by running the check on a phone
 * viewport after the first was fixed:
 *
 *   a. **No stylesheet.** The SVG contained `#hud` but not the document's `<style>`, so the panel was
 *      rasterised unstyled and rows collapsed into one line of text.
 *   b. **No dpr scaling.** The panel was composited using CSS-pixel coordinates into a drawing buffer
 *      that is 729 px wide for a 390-px viewport, so on a phone it appeared at ~half size in the corner.
 *
 * The metric that finally works — resize both crops to one canonical width, then compare bright-pixel
 * masks — is the third, and the first two attempts each failed for a reason worth keeping:
 *
 *   * **Raw pixel overlay** scored 0.059 at dpr 2 on a capture that was visually identical to the page.
 *     The screenshot is in device pixels (x2.0) while the drawing buffer is capped by `MAX_PIXELS` and
 *     composited at x1.869; across a 314-px block that is ~20 px of drift, enough to miss every glyph.
 *   * **Ink bounding box** certified the pre-fix page as correct at dpr 1 (`width 102%`), because a panel
 *     rasterised with no CSS at all still fills its box with text by coincidence. Geometry cannot see a
 *     layout collapse.
 *
 * ## The threshold
 *
 * `MIN_IOU = 0.45`. Measured, at both viewports:
 *
 * | page state | IoU |
 * | --- | --- |
 * | correct | **0.728 – 1.000** |
 * | no stylesheet, no dpr scaling (pre-fix) | **0.000 – 0.025** |
 * | stylesheet fixed, dpr scaling missing | **0.007** at dpr 2 |
 *
 * The threshold sits between the populations rather than near either, so a font or hinting change cannot
 * trip it and neither defect can pass it.
 *
 * Usage: node tools/check-hud-capture.mjs [--port N] [--url URL]
 * Needs the Chromium unpacked and LD_LIBRARY_PATH=/tmp/al2023/lib, like the other browser checks.
 */

import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { extname } from "node:path";
import puppeteer from "puppeteer-core";
import chromium from "@sparticuz/chromium";
import { resolveStaticPath } from "./static-path.mjs";

const MIME = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript",
  ".mjs": "text/javascript",
  ".wasm": "application/wasm",
};

function flag(name, dflt) {
  const i = process.argv.indexOf(`--${name}`);
  return i >= 0 && process.argv[i + 1] ? process.argv[i + 1] : dflt;
}

// Text overlap between the panel on the page and the panel in the saved picture, both resized to one
// canonical width first. Measured: correct 0.725-0.829; a panel rasterised without its stylesheet 0.020;
// a panel drawn at CSS size instead of buffer size 0.005. The threshold sits between the populations
// rather than near either, so a font or hinting change cannot trip it, and neither defect can pass it.
const MIN_IOU = 0.45;

const server = createServer(async (req, res) => {
  try {
    const f = resolveStaticPath(req.url);
    if (f === null) throw new Error("forbidden");
    const body = await readFile(f);
    res.writeHead(200, { "content-type": MIME[extname(f)] ?? "application/octet-stream" }).end(body);
  } catch {
    res.writeHead(404).end("not found");
  }
});

const urlArg = flag("url", null);
const port = Number(flag("port", 8091));
let server_ = null;
if (!urlArg) {
  await new Promise((res) => server.listen(port, "127.0.0.1", res));
  server_ = server;
}
const target = urlArg ?? `http://127.0.0.1:${port}/`;

const browser = await puppeteer.launch({
  executablePath: await chromium.executablePath(),
  args: [...chromium.args, "--enable-unsafe-swiftshader", "--use-gl=angle"],
  defaultViewport: { width: 900, height: 700, deviceScaleFactor: 1 },
  headless: true,
});

const checks = [];
function fail(msg) {
  checks.push(["FAIL", msg]);
}

/**
 * Two viewports, because the defect is dpr-dependent.
 *
 * The half-size panel only appeared above `devicePixelRatio` 1, and every check in the repository ran at
 * dpr 1 on a desktop-sized window. The person who judges claim 5 judges it from a **phone**, which is
 * dpr 2 or 3. A check that only runs at dpr 1 would have certified the broken phone capture as good.
 */
const VIEWPORTS = [
  { width: 900, height: 700, deviceScaleFactor: 1, label: "desktop, dpr 1" },
  { width: 390, height: 844, deviceScaleFactor: 2, label: "phone, dpr 2" },
];

for (const vp of VIEWPORTS) {
  const page = await browser.newPage();
  await page.setViewport({ width: vp.width, height: vp.height, deviceScaleFactor: vp.deviceScaleFactor });
  await page.goto(target, { waitUntil: "networkidle0", timeout: 90000 });
  // Let the first frame land and the panel settle to its final height.
  await new Promise((r) => setTimeout(r, 1500));

  const rect = await page.evaluate(() => {
    const el = document.querySelector("#scales");
    if (!el) return null;
    const r = el.getBoundingClientRect();
    return { x: Math.round(r.x), y: Math.round(r.y), width: Math.round(r.width), height: Math.round(r.height) };
  });

  // The capture and the live screenshot are in DIFFERENT coordinate spaces above dpr 1: the screenshot
  // is in device pixels (CSS x dpr, or scaled by the pixel cap), the rect is in CSS pixels. The ratio is
  // taken from the actual capture, so this works whether the drawing buffer is larger than the viewport
  // or capped below it.
  if (!rect || rect.width < 40 || rect.height < 20) {
    fail(`${vp.label}: could not locate a render-scale button block (rect: ${JSON.stringify(rect)})`);
    await page.close();
    continue;
  }

  const live = await page.screenshot({ clip: rect, encoding: "base64" });
  const captured = await page.evaluate(() =>
    window.__odderon.capturePicture({ measure: false }).then((r) => r.png.split(",")[1]),
  );

  const result = await page.evaluate(
    async (liveB64, capB64, r) => {
      const load = (d) =>
        new Promise((s, j) => {
          const i = new Image();
          i.onload = () => s(i);
          i.onerror = j;
          i.src = "data:image/png;base64," + d;
        });
      const [liveImg, capImg] = await Promise.all([load(liveB64), load(capB64)]);

      // BOTH IMAGES ARE RESIZED TO ONE CANONICAL PANEL SIZE BEFORE COMPARING.
      //
      // This is the third metric tried, and the two that failed are worth recording because each failure
      // was a real fact about the page rather than sloppiness:
      //
      //  1. RAW PIXEL OVERLAY. Scored 0.059 at dpr 2 on a capture that was visually identical to the page.
      //     The page screenshot is in device pixels (CSS x dpr = x2) while the drawing buffer is capped by
      //     MAX_PIXELS, so the canvas is composited at a different scale (x1.869). Two images of the same
      //     panel at 1.869 and 2.000 do not line up across 314 px -- ~20 px of drift, which misses nearly
      //     every glyph.
      //  2. INK BOUNDING BOX. Certified the pre-fix page as correct at dpr 1 (`width 102%`), because a
      //     panel rasterised with no CSS at all still, by coincidence, fills its box with text. Geometry
      //     alone cannot see a layout collapse.
      //
      // Resizing both crops to the same 340-px-wide bitmap removes the scale problem, and the mask then
      // asks the question that matters -- is the text in the same place -- while staying blind to the one
      // deliberate difference between picture and page: the panel is drawn opaque so the provenance stamp
      // cannot bleed through it.
      //
      // Measured: correct 0.725-0.829; pre-fix 0.020; stylesheet-fixed-but-not-scaled 0.005. The threshold
      // is 0.45 -- between the populations, not near either.
      const TW = 340;
      const TH = Math.round((TW * r.height) / r.width);

      const mask = (img, sx, sy, sw, sh) => {
        const c = document.createElement("canvas");
        c.width = TW;
        c.height = TH;
        const cx = c.getContext("2d", { willReadFrequently: true });
        cx.drawImage(img, sx, sy, sw, sh, 0, 0, TW, TH);
        const px = cx.getImageData(0, 0, TW, TH).data;
        const m = new Uint8Array(TW * TH);
        let ink = 0;
        for (let i = 0, q = 0; i < px.length; i += 4, q++) {
          if (0.299 * px[i] + 0.587 * px[i + 1] + 0.114 * px[i + 2] > 120) {
            m[q] = 1;
            ink++;
          }
        }
        return { m, ink };
      };

      // The live screenshot is already cropped to the panel. The capture is the whole frame, so its crop
      // is the panel rect in BUFFER pixels: `k` is derived from the captured frame rather than from
      // devicePixelRatio, because `state.scale` and the pixel cap both move the buffer independently.
      const k = capImg.width / window.innerWidth;
      const A = mask(liveImg, 0, 0, liveImg.width, liveImg.height);
      const B = mask(capImg, r.x * k, r.y * k, r.width * k, r.height * k);

      let inter = 0;
      let union = 0;
      for (let i = 0; i < A.m.length; i++) {
        if (A.m[i] || B.m[i]) union++;
        if (A.m[i] && B.m[i]) inter++;
      }
      return { iou: union ? inter / union : 0, inkLive: A.ink, inkPicture: B.ink, k: +k.toFixed(3) };
    },
    live,
    captured,
    rect,
  );

  // Guard the guard: an empty mask on either side makes IoU meaningless, and "0 of 0 pixels agree" is
  // not a pass. This is the exact shape of failure that let the original bug through.
  if (result.inkLive < 50) {
    fail(`${vp.label}: the live panel has almost no text in this region (${result.inkLive} px)`);
  } else if (result.inkPicture < 50) {
    fail(`${vp.label}: the panel composited into the picture has almost no text (${result.inkPicture} px)`);
  } else if (result.iou < MIN_IOU) {
    fail(
      `${vp.label}: the picture's panel does not match the page's: text IoU ${result.iou.toFixed(3)} < ` +
        `${MIN_IOU} (live ${result.inkLive} px, picture ${result.inkPicture} px, buffer scale ` +
        `${result.k}). The saved PNG is the artifact claim 5 is judged from; if it does not match the ` +
        `page, the numbers in it cannot be argued from.`,
    );
  } else {
    checks.push([
      "ok  ",
      `${vp.label}: panel matches the page  IoU ${result.iou.toFixed(3)} (>= ${MIN_IOU}, scale ${result.k})`,
    ]);
  }

  await page.close();
}

await browser.close();
if (server_) server.close();

for (const [tag, msg] of checks) console.log(`  ${tag} ${msg}`);
console.log("");
if (checks.some(([t]) => t === "FAIL")) {
  console.log("HUD capture check FAILED");
  process.exit(1);
}
console.log("HUD capture check passed");
