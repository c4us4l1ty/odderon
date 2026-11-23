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
  const start = html.indexOf("function summarise(rows, compileMs) {");
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
function healthy() {
  return [
    row("baseline", 2.00),
    row("control", 2.02),
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

console.log("");
if (failures) {
  console.log(`${failures} failure(s)`);
  process.exit(1);
}
console.log("all ablation report checks passed");
