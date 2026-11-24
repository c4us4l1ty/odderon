#!/usr/bin/env node
/**
 * Regression tests for the ablation report's *judgement*, not its measurement.
 *
 * ## Why this exists
 *
 * `web/ablate.html` measured a shader and printed a verdict. It was wrong, and nothing caught it,
 * because a page that produces plausible numbers has no failure mode to observe. On Reference A it
 * reported the full shader at 3.00 ms and the same shader at native resolution at 1.71 ms - a 43%
 * self-disagreement - and then went on to print "R2 IS reachable at 1.38x", which is a render scale
 * greater than native, i.e. not a render scale at all.
 *
 * The measurement loop is now interleaved with a control row. This file tests the part that reads the
 * numbers, by feeding `summarise()` known matrices and asserting on what it says. Four cases:
 *
 *   1. A clean run reports a verdict and a small error bar.
 *   2. A run whose control disagrees by 43% reports **no verdict at all**.
 *   3. A matrix where the deltas add up to more frame than exists is **condemned by name**.
 *   4. A render scale above 1.0 is never printed as a scale.
 *
 * Case 3 is the one that matters: it is the exact shape of the real Reference A failure, and the
 * assertion is that the report refuses to call those deltas findings.
 *
 * Usage: node tools/check-ablation.mjs
 */

import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, "..");

// ---------------------------------------------------------------------------------------------
// Load `summarise()` out of the page.
//
// The page is a single module with no build step and no exports, which is the correct shape for a
// self-contained instrument. Rather than restructure it for testability, the function is extracted by
// brace matching. That is slightly ugly and it is *honest*: if the page is refactored such that this
// extraction breaks, the test fails loudly instead of passing against a stale copy.
// ---------------------------------------------------------------------------------------------

function loadSummarise() {
  const html = readFileSync(resolve(root, "web/ablate.html"), "utf8");
  // Match the NAME, not the argument list. Pinning the full signature meant adding a default parameter
  // broke the extractor with "did it get renamed?", which is a test failing for a reason that has
  // nothing to do with the behaviour it tests.
  const start = html.indexOf("function summarise(");
  if (start < 0) throw new Error("summarise() not found in web/ablate.html - did it get renamed?");

  let depth = 0;
  const open = html.indexOf("{", start);
  let end = -1;
  for (let i = open; i < html.length; i++) {
    if (html[i] === "{") depth++;
    else if (html[i] === "}") {
      depth--;
      if (depth === 0) {
        end = i + 1;
        break;
      }
    }
  }
  if (end < 0) throw new Error("could not find the end of summarise()");

  const src = html.slice(start, end);
  const fn = new Function("return " + src)();
  if (typeof fn !== "function") throw new Error("extracted summarise is not a function");
  return fn;
}

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

/** A row in the shape `run()` produces. */
const row = (id, ms, extra = {}) => ({
  id,
  label: extra.label ?? id,
  ok: true,
  amortised: ms,
  min: ms,
  max: ms,
  spread: 0,
  w: 1400,
  h: 900,
  ...extra,
});

/**
 * The matrix Reference A actually produced, transcribed.
 *
 * This is not a hypothetical. `full shader (as shipped)` = 3.00 ms and `resolution 1.00x (native)` =
 * 1.71 ms are two rows of the same work; and the five feature ablations land at 1.50-1.58 ms, all
 * within 5% of each other, which is the signature of a shared floor rather than five real costs.
 */
function referenceA(controlScale = 1.01) {
  return [
    row("baseline", 3.00),
    row("control", 3.00 * controlScale),
    row("floor", 0.17),
    row("floor_tex", 0.29),
    row("floor_tex_sample", 0.29),
    row("floor_blit", 0.29),
    row("no_shadow", 0.83),
    row("shadow_4tap", 1.33),
    row("no_wall_ribs", 1.58),
    row("no_aerial", 1.50),
    row("no_sphere", 1.50),
    row("no_tonemap", 1.50),
    row("no_sky_glow", 1.54),
    row("r100", 1.71, { resScale: 1.0 }),
    row("r071", 0.88, { resScale: 0.71 }),
    row("r050", 0.50, { resScale: 0.5 }),
    row("r035", 0.21, { resScale: 0.35 }),
    row("r025", 0.17, { resScale: 0.25 }),
  ];
}

/**
 * A *healthy* matrix: every feature ablated once, one real cost, everything else near zero. This is
 * what the page should look like when the instrument is working and the shader is simple. It exists so
 * the tests can prove the condemnation logic does not fire on good data - a detector that always
 * screams is as useless as one that never does.
 */
/**
 * The first real-hardware run of the REWRITTEN instrument — a laptop, 2026-11-24, transcribed from the
 * page's own output. Not hypothetical, and not a clean run either.
 *
 * `frames` is the field that matters here. Each row is a batch total divided by its frame count, so the
 * timer's quantisation is `tick / frames` for THAT row, and `tick` on this device is 1 ms (Chrome
 * coarsens `performance.now()`). The page printed a 63 us floor and then judged deltas of 50 us against
 * a 37.5 us error bar, i.e. it reported a finding smaller than its own resolution.
 */
function laptop2026_11_24() {
  // 8 is MIN_FRAMES in the page, so these are rows that took the clamp's low branch — the case that
  // produces the coarsest floor and the one the real run hit.
  const r = (id, ms, frames = 8, extra = {}) => row(id, ms, { frames, ...extra });
  return [
    r("baseline", 0.75, 16),
    r("control", 0.771, 16),
    r("floor", 0.13, 400),
    r("floor_tex", 0.25, 240),
    r("floor_blit", 0.24, 240),
    r("no_shadow", 0.60),
    r("shadow_4tap", 0.75),
    r("no_wall_ribs", 0.73),
    r("no_aerial", 0.72),
    r("no_sphere", 0.70),
    r("no_tonemap", 0.74),
    r("no_skyglow", 0.75),
    r("r100", 0.76, 80, { resScale: 1.0 }),
    r("r050", 0.19, 300, { resScale: 0.5 }),
    r("r025", 0.06, 400, { resScale: 0.25 }),
  ];
}

function healthy() {
  return [
    row("baseline", 2.00, { frames: 100 }),
    row("control", 2.02, { frames: 100 }),
    row("floor", 0.20),
    row("floor_tex", 0.30),
    row("floor_blit", 0.30),
    row("no_shadow", 0.90), // a real 55% cost
    row("shadow_4tap", 1.60), // taps really are most of the shadow
    row("no_wall_ribs", 1.94), // ~3%
    row("no_sphere", 1.97), // ~1.5%
    row("no_tonemap", 1.98), // ~1%
    row("r100", 1.15, { resScale: 1.0 }),
    row("r050", 0.50, { resScale: 0.5 }),
    row("r025", 0.20, { resScale: 0.25 }),
  ];
}

// ---------------------------------------------------------------------------------------------
// Assertions
// ---------------------------------------------------------------------------------------------

let failures = 0;

function check(name, cond, detail) {
  if (cond) {
    console.log(`  ok    ${name}`);
  } else {
    failures++;
    console.log(`  FAIL  ${name}`);
    if (detail) console.log(`        ${String(detail).split("\n").join("\n        ")}`);
  }
}

const summarise = loadSummarise();

console.log("case 1: a healthy matrix reports a verdict");
{
  const out = summarise(healthy());
  check("names the error bar", out.includes("INSTRUMENT ERROR BAR"), out);
  check("says the instrument agrees with itself", /agrees with itself/.test(out), out);
  check("prints a verdict", /VERDICT:/.test(out), out);
  check("does not condemn its own deltas", !out.includes("DO NOT ADD UP"), out);
}

console.log("case 2: a 43% control disagreement suppresses the verdict");
{
  const out = summarise(referenceA(1.43));
  check("flags the disagreement", /disagreement 43\.0%/.test(out), out);
  check("says the run is not trustworthy", /NOT trustworthy/.test(out), out);
  check("prints NO VERDICT", out.includes("NO VERDICT"), out);
  check("prints no contrary verdict", !/VERDICT: R2/.test(out), out);
}

console.log("case 3: deltas that add up to more frame than exists are condemned");
{
  // The real Reference A shape, with a *well-behaved* control - i.e. the ordering bias is gone, so the
  // error bar cannot catch this and the arithmetic has to.
  const out = summarise(referenceA(1.01));
  check("detects the impossibility", out.includes("DO NOT ADD UP"), out);
  check("states the total exceeds the frame", /more frame than/.test(out), out);
  check("names the five clustered rows", /no_wall_ribs/.test(out) && /no_tonemap/.test(out), out);
  check("names the largest as the only candidate", /If one is real it is the largest/.test(out), out);

  // and it must not fire on data that does add up
  const ok = summarise(healthy());
  check("does not fire on a healthy matrix", !ok.includes("DO NOT ADD UP"), ok);
}

console.log("case 4: rows that read exactly 0.00 ms are not reported as savings");
{
  // Shape of the real post-fix phone run: baseline 0.750 ms with a 183% spread, five ablations landing on
  // 0.00 ms of saving, and a 1 in 16 frames timer floor of 62 us. At 0.75 ms a 62 us floor is 8% of the
  // frame, and five features cannot all be worth 0.00 ms. The report must not call those measurements.
  const rows = [
    row("baseline", 0.750), row("control", 0.750),
    row("floor", 0.13), row("floor_tex", 0.29), row("floor_blit", 0.29),
    row("no_shadow", 0.63), row("shadow_4tap", 0.79),
    row("no_wall_ribs", 0.75), row("no_aerial", 0.75), row("no_sphere", 0.75),
    row("no_tonemap", 0.75), row("no_sky_glow", 0.75),
    row("r100", 0.79, { resScale: 1.0 }), row("r050", 0.21, { resScale: 0.5 }),
    row("r025", 0.08, { resScale: 0.25 }),
  ];
  // spread 1.83 on the baseline is itself a red flag: it means the median is not representative
  rows[0] = { ...rows[0], spread: 1.83 };
  rows[1] = { ...rows[1], spread: 1.83 };
  const out = summarise(rows);
  check("0.00 ms deltas are not listed as measured", !/measured:.*no_aerial/.test(out), out);
  check("names them as at-or-below threshold", /NOT findings/.test(out), out);
}

console.log("case 5: a render scale is never printed above 1.0");
{
  // A screen small enough that the cost model says native already fits. The old code printed
  // "R2 IS reachable at 1.38x", which is a pixel ratio being mistaken for a render scale.
  const rows = referenceA(1.01).map((r) =>
    r.resScale ? { ...r, w: 900, h: 560 } : r
  );
  const out = summarise(rows);
  // No "reachable at Nx" line at all, and no scale figure that exceeds 1.
  check("prints no scale figure", !/scale that fits is/.test(out), out);
  check("says native already fits", /native already fits|MET AT NATIVE/i.test(out), out);
  check("does not report a scale above 1.0", !/reachable at 1\.[1-9]/.test(out), out);
}

console.log("case 6: a delta under the timer floor is not a finding, however it compares to the error bar");
{
  // The real laptop run. `no_sphere` reads 0.70 against a 0.75 baseline: 50 us, which clears the 5%
  // error bar (37.5 us) and was therefore reported as a finding. It is 0.8 of one 63 us timer tick.
  //
  // Both the floor AND the row's frame count matter here, and getting either wrong makes the test
  // vacuous. The floor has to be coarse enough (minFrames 8 -> 125 us) for the delta to fall under it,
  // and the row has to be one that actually hit that clamp (frames = 8) rather than a fast row with
  // hundreds of frames and a fine floor. The first version of this case passed for neither reason: it
  // gave every row `frames: 8`, and its assertion regex carried an escaped pipe (`no_sphere\|sphere`)
  // which can never match, so it was a tautology that reported `ok` regardless of the output.
  // tickMs = 1 reproduces Chrome's coarsened clock, which is what the laptop actually had. Without
  // it this case runs against Node's fine timer and cannot fail however wrong the page is.
  const out = summarise(laptop2026_11_24(), 1000, 8, 1);
  const findings = /larger than the threshold:[^\n]*/.exec(out);
  check("prints a feature-delta line", !!findings, out);
  check("counts the shadow as a finding", /no_shadow|solar/.test(findings ? findings[0] : ""), out);
  check("does NOT count the sphere test as a finding", !/no_sphere|sphere/.test(findings ? findings[0] : "_"), out);
  check("gives the timer floor as the reason", /timer floor|us timer/.test(out) || /NOT findings/.test(out), out);
}

console.log("case 7: the verdict names the blit, not the full-shade extrapolation");
{
  // On this run the full shader cost 0.41 ms/Mpx and the screen was 1.86 Mpx, so the sweep said "native
  // fits with room to spare" — while the FBO blit, which is what a settled frame actually pays, cost
  // 0.24 ms. Both are true and only the second is what R2 asks about. The page used to print both in
  // the same breath without saying they answered different questions.
  const out = summarise(laptop2026_11_24());
  check("says R2 is about the drawn frame, not the shade",
    /R2 ASKS ABOUT THE DRAWN FRAME, NOT THE SHADE/.test(out), out);
  check("quotes the cached frame against 1.5 ms", /cached static frame: 0\.24 ms/.test(out), out);
  check("labels the sweep as the full shader", /VERDICT \(full shade\)/.test(out), out);
  check("does not print two unqualified verdicts", (out.match(/VERDICT:/g) || []).length <= 1, out);
}

console.log("case 8: the floor rows measure different OPERATIONS, not the same shader twice");
{
  // The defect this replaces: `floor_blit` and `floor_tex` compiled byte-identical GLSL and differed
  // only in which texture object they sampled, so the page reported a 0.01 ms "difference in mechanism"
  // between two runs of the same code. On real hardware that gap was 10 us against a 63 us tick.
  //
  // The check is structural because the bug was structural: if two floor variants produce the same
  // program source, the report must not attribute a cost difference to a mechanism. The page now
  // dispatches a real `blitFramebuffer` for the blit row, which runs no fragment shader at all.
  const src = readFileSync(new URL("../web/ablate.html", import.meta.url), "utf8");
  check("the blit row performs a real blitFramebuffer", /gl\.blitFramebuffer\(/.test(src),
    "no blitFramebuffer call in the page — the row is a texture read wearing a blit label");
  check("the blit row is dispatched to its own mode", /'blitfb'/.test(src),
    "no distinct dispatch mode — blit and texture-read share a code path");
  // Extracted and inspected rather than regex-matched across a window: a character budget is a magic
  // number, and the first version of this assertion failed on correct code because the comment was
  // longer than the budget. The property is "the blitfb branch returns before any draw call".
  {
    const fnStart = src.indexOf("function drawOnce(");
    const braceAt = src.indexOf("{", fnStart);
    let d = 0, end = -1;
    for (let k = braceAt; k < src.length; k++) {
      if (src[k] === "{") d++;
      else if (src[k] === "}") { d--; if (!d) { end = k + 1; break; } }
    }
    const body = src.slice(fnStart, end);
    const blitAt = body.indexOf("mode === 'blitfb'");
    const drawAt = body.indexOf("drawArrays");
    check("drawOnce has a blitfb branch", blitAt >= 0, body.slice(0, 80));
    check("the blitfb branch returns before the drawArrays", blitAt >= 0 && blitAt < drawAt,
      "the blit mode falls through to a drawArrays");
    const branch = blitAt >= 0 ? body.slice(blitAt, body.indexOf("return;", blitAt) + 7) : "";
    check("the blitfb branch contains no draw call", !/drawArrays|drawElements/.test(branch), branch);
  }
  check("summary says when the blit could not be measured", /not measured, not zero/.test(src),
    "a refused blitFramebuffer would be silently absent from the floors paragraph");
  // And the doc must not still claim the renderer blits.
  const doc = readFileSync(new URL("../docs/GRAPHICS.md", import.meta.url), "utf8");
  check("GRAPHICS.md does not claim the cached path is a blit",
    !/The blit is in fact \*cheaper\*/.test(doc), "the retracted mechanism is still in the document");
  check("GRAPHICS.md withdraws the 11 B/px figure", /withdrawn/.test(doc),
    "the ceiling is still quoted without qualification");
}

console.log("");
if (failures) {
  console.log(`${failures} failure(s)`);
  process.exit(1);
}
console.log("all ablation report checks passed");
