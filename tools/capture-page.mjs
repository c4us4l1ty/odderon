#!/usr/bin/env node
/**
 * Turn a PNG a person sent back into a file this repository can talk about.
 *
 * Why this exists: `web/index.html` can now capture its own frame (`window.__odderon.capturePicture()`),
 * which is the only way to see the view on a device that has no debugging cable attached. The person
 * saves that PNG and attaches it. This tool is the other half — it takes what came back and reports the
 * facts that matter for judging it, without asking anyone to trust a description of the image.
 *
 * It does two things and refuses to do a third:
 *
 *   1. Reads the dimensions out of the PNG header (IHDR, bytes 16..24). No image library, no dependency:
 *      the header is a fixed, specified layout and parsing it is four lines.
 *   2. Prints the provenance stamp that `capturePicture()` burned into the bottom of the image, if the
 *      caller also pasted it (--stamp). The stamp is text, so it can be quoted in a doc; the image
 *      cannot be committed at all.
 *
 * What it refuses to do is judge the picture. Claim 5 is "the window view is the best-looking thing on
 * your laptop", and that is a human verdict. A tool that scored it would be inventing a number to stand
 * in for a judgement, which is the failure mode this whole repository is organised against.
 *
 * ## Why the PNG is never committed
 *
 * `ci/kernel-policy.sh` R5 forbids binary assets in `git ls-files`, and `GRAPHICS.md` R1 gives the reason:
 * every surface must be a function S(u,v,t), not a stored bitmap. A screenshot is a stored bitmap. The
 * rule has no documentation exception, so the image goes in `.artifacts/` (gitignored) and only its
 * measurements and its stamp are quotable.
 *
 * Usage:
 *   node tools/capture-page.mjs photo.png
 *   node tools/capture-page.mjs photo.png --stamp "canvas 378 x 691 ..." --stamp "scale 0.50 ..."
 *   node tools/capture-page.mjs photo.png --json      # machine-readable, for pasting into an issue
 */

import { readFileSync, existsSync } from "node:fs";

const args = process.argv.slice(2);
const file = args.find((a) => !a.startsWith("--"));
const wantJson = args.includes("--json");

const stamps = [];
for (let i = 0; i < args.length; i++) {
  if (args[i] === "--stamp" && args[i + 1]) stamps.push(args[i + 1]);
}

if (!file) {
  console.error("usage: node tools/capture-page.mjs <image.png> [--stamp \"line\"] [--json]");
  process.exit(2);
}
if (!existsSync(file)) {
  console.error(`no such file: ${file}`);
  process.exit(2);
}

const buf = readFileSync(file);

// The signature is the only thing that distinguishes "a PNG" from "a file with a .png name". Checking it
// matters here because the failure mode is silent: a JPEG renamed to .png would otherwise be reported as
// a 0x0 image by the IHDR read below, which looks like a broken capture rather than a wrong format.
const PNG_SIG = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
const isPng = PNG_SIG.every((b, i) => buf[i] === b);
if (!isPng) {
  console.error(`${file} is not a PNG (bad signature). Re-save it as PNG and try again.`);
  process.exit(1);
}

// IHDR is always the first chunk, so width and height are at fixed offsets: 8 bytes signature, 4 length,
// 4 type, then width and height as big-endian uint32.
const width = buf.readUInt32BE(16);
const height = buf.readUInt32BE(20);
const megapixels = (width * height) / 1e6;

// The scale factor is not in the pixels -- a 0.5x render upscaled to the panel and a 1.0x render look
// identical at the same output size. It is only in the stamp, which is exactly why the stamp is burned
// into the image: without it, a downscaled frame is indistinguishable from a fast one.
let scale = null;
for (const s of stamps) {
  const m = /\bscale\s+([0-9.]+)/i.exec(s);
  if (m) scale = Number(m[1]);
}

const report = {
  file,
  width,
  height,
  megapixels: Number(megapixels.toFixed(3)),
  scale,
  // At 0.5x the panel is fed 1/4 of the pixels; that is the single number most likely to change how a
  // picture should be read, so it is computed rather than left to the reader.
  shadedMegapixels: scale ? Number((megapixels * scale * scale).toFixed(3)) : null,
  stamps,
};

if (wantJson) {
  console.log(JSON.stringify(report, null, 2));
} else {
  console.log(`file          ${file}`);
  console.log(`size          ${width} x ${height}  (${report.megapixels} Mpx)`);
  if (scale !== null) {
    console.log(`render scale  ${scale.toFixed(2)}  -> ${report.shadedMegapixels} Mpx actually shaded`);
  } else {
    console.log(`render scale  not stated — pass --stamp "scale 0.50 ..." if the image shows one`);
  }
  if (stamps.length) {
    console.log(`\nprovenance stamp from the image:`);
    for (const s of stamps) console.log(`  ${s}`);
  } else {
    console.log(`\nno --stamp given. The image has a stamp burned into its bottom edge; quote it here so`);
    console.log(`the numbers are quotable in a doc, because the image itself cannot be committed.`);
  }
}
