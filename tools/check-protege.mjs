#!/usr/bin/env node
/**
 * The gate that makes the protégé's teaching arc a real kernel prediction rather than a page-drawn
 * progression.
 *
 * `web/index.html` answers "what tier is the protégé in, how deep is their next question, and what
 * can they notice?" by calling the kernel's `protege_*` exports -- the seam over
 * `odderon_astro::protege`, which maps the reputation count and the kernel's error-entry readings
 * to a tier, a question depth, and a notice level. The same "no substitute" contract the orbit,
 * the rocket, the pad, the pass, the rendezvous and the comms follow: the page computes no tier
 * threshold, holds no arc constant, and shows no progression when the kernel is absent.
 *
 * The checks:
 *
 *   1. NO LOCAL ARCH MATHS. The page may read the kernel's protege readings but must not compute an
 *      arc itself: no hard-coded tier threshold, no local reputation comparison, no local depth
 *      formula. The failure mode is "someone re-adds a local progression", which no runtime probe
 *      can see.
 *
 *   2. THE PAGE REQUIRES EVERY PROTÉGÉ EXPORT. The page must reference all four kernel accessors:
 *      `protege_tier`, `protege_question_depth`, `protege_notice_level`, and
 *      `protege_design_hash`. A partial build fails at load.
 *
 *   3. NO SILENT FALLBACK. `protegeReadings` must return a named source when the kernel is absent,
 *      and the panel must show `—` rather than a fabricated progression.
 *
 *   4. THE PROTÉGÉ READINGS COME FROM THE KERNEL. `protegeReadings` must call the four
 *      `protege_*` accessors, not compute an arc from a local formula.
 *
 *   5. THE WASM BOUNDARY IS PHYSICAL. When a build is available, the exports must reproduce the
 *      invariants of the arc: the tier thresholds are exactly the kernel's (0–1 novice, 2–4
 *      junior, 5–9 analyst, 10+ peer); the question depth never exceeds the tier and is capped at 1
 *      for a refused launch or a non-finite apogee; the notice level never exceeds the tier; the
 *      design hash is deterministic and non-zero.
 *
 * Usage: node tools/check-protege.mjs            (static: no browser, no wasm needed)
 *        node tools/check-protege.mjs --selftest  (verify the verifier accepts good exports)
 *        node tools/check-protege.mjs --wasm dist/odderon_dmath_wasm.wasm
 *                                                   (also run the exports, if a build is available)
 */

import { readFile } from "node:fs/promises";

const PAGE = new URL("../web/index.html", import.meta.url);

/** Strip comments so documentation about the arc does not trip the checks that hunt for a solver. */
function liveSource(html) {
  const script = html.match(/<script type="module">([\s\S]*?)<\/script>/);
  if (!script) throw new Error("no module script found in web/index.html");
  return script[1]
    .replace(/\/\*[\s\S]*?\*\//g, "")
    .replace(/(^|[^:])\/\/.*$/gm, "$1");
}

const checks = [];
function check(name, fn) {
  checks.push({ name, fn });
}

check("no local arc maths in the page", (src) => {
  // A local solver would carry the tier thresholds, a reputation comparison, or a hand-rolled
  // depth formula. Each is the giveaway that the page is computing the arc itself.
  const patterns = [
    [/protege_tier\s*\([^)]*\)\s*=[^;]*Math\./, "a hand-computed tier"],
    [/protege_question_depth\s*\([^)]*\)\s*=[^;]*Math\./, "a hand-computed question depth"],
    [/protege_notice_level\s*\([^)]*\)\s*=[^;]*Math\./, "a hand-computed notice level"],
    [/protege_tier\s*\([^)]*\)\s*=[^;]*(reputation|correct)[^;]*>=\s*[0-9]/, "a local threshold comparison"],
    [/protege_tier\s*\([^)]*\)\s*=[^;]*(reputation|correct)[^;]*<\s*[0-9]/, "a local threshold comparison"],
  ];
  const hits = patterns.filter(([re]) => re.test(src)).map(([, why]) => why);
  if (hits.length) throw new Error("page still computes the arc itself: " + hits.join("; "));
});

check("the page requires every protégé export", (src) => {
  const needed = [
    "protege_tier",
    "protege_question_depth",
    "protege_notice_level",
    "protege_design_hash",
  ];
  const missing = needed.filter((n) => !src.includes(n));
  if (missing.length) {
    throw new Error("page does not reference: " + missing.join(", "));
  }
});

check("a missing kernel hides the arc rather than substituting a progression", (src) => {
  // The contract: when the kernel is absent, `protegeReadings` returns a named `source` that is
  // not the kernel, and the panel shows `—` rather than a fabricated progression.
  if (!/function protegeReadings\s*\([^)]*\)\s*\{[\s\S]{0,200}?if \(!k\) return \{ source: 'absent' \};/.test(src)) {
    throw new Error("protegeReadings does not return a named absent source when the kernel is missing");
  }
});

check("the protégé readings come from the kernel, not a local formula", (src) => {
  const m = src.match(/function protegeReadings\s*\([^)]*\)\s*\{([\s\S]*?)\n\}/);
  if (!m) throw new Error("protegeReadings not found");
  const body = m[1];
  for (const accessor of ["protege_tier", "protege_question_depth", "protege_notice_level"]) {
    if (!body.includes(accessor)) {
      throw new Error(`protegeReadings does not call ${accessor}`);
    }
  }
});

/**
 * The physics the protégé exports must satisfy, as a function of the exports alone.
 *
 * Separated from the file loading because the wasm build only exists in CI: if the verification lived
 * inside `runWasm`, none of it could be executed in this sandbox, and "checked in CI" would be the
 * whole of its testing. `--selftest` drives this with a deliberate shim so both the accept and reject
 * paths run here.
 */
function verifyExports(ex) {
  const need = [
    "protege_tier",
    "protege_question_depth",
    "protege_notice_level",
    "protege_design_hash",
  ];
  const missing = need.filter((n) => typeof ex[n] !== "function");
  if (missing.length) throw new Error("module is missing: " + missing.join(", "));

  const problems = [];

  // Tier thresholds: 0-1 novice, 2-4 junior, 5-9 analyst, 10+ peer.
  const tierCases = [
    [0, 0], [1, 0], [2, 1], [4, 1], [5, 2], [9, 2], [10, 3], [100, 3],
  ];
  for (const [rep, want] of tierCases) {
    const got = ex.protege_tier(rep);
    if (got !== want) problems.push(`tier(${rep}) = ${got}, want ${want}`);
  }

  // Question depth never exceeds the tier for a suborbital failure with a real apogee.
  for (let tier = 0; tier <= 3; tier++) {
    const depth = ex.protege_question_depth(0, 45.0, tier);
    if (depth !== tier) problems.push(`depth(suborbital, tier ${tier}) = ${depth}, want ${tier}`);
  }

  // A refused launch caps the depth at 1 at every tier >= 1.
  for (let tier = 1; tier <= 3; tier++) {
    const depth = ex.protege_question_depth(3, NaN, tier);
    if (depth !== 1) problems.push(`depth(refused, tier ${tier}) = ${depth}, want 1`);
  }

  // A non-finite apogee caps the depth at 1 at every tier >= 1.
  for (let tier = 1; tier <= 3; tier++) {
    const depth = ex.protege_question_depth(1, NaN, tier);
    if (depth !== 1) problems.push(`depth(non-finite apogee, tier ${tier}) = ${depth}, want 1`);
  }

  // A novice with a refused launch gets verdict depth (0).
  const noviceRefused = ex.protege_question_depth(3, NaN, 0);
  if (noviceRefused !== 0) problems.push(`depth(novice refused) = ${noviceRefused}, want 0`);

  // Notice level never exceeds the tier.
  for (let tier = 0; tier <= 3; tier++) {
    const notice = ex.protege_notice_level(tier);
    if (notice !== tier) problems.push(`notice(tier ${tier}) = ${notice}, want ${tier}`);
  }

  // Design hash is deterministic and non-zero.
  const h1 = ex.protege_design_hash();
  const h2 = ex.protege_design_hash();
  if (h1 !== h2) problems.push("design hash is not deterministic");
  if (h1 === 0) problems.push("design hash is zero");

  if (problems.length) throw new Error("boundary disagrees with the arc: " + problems.join("; "));
  return `tier=${ex.protege_tier(10)} depth=${ex.protege_question_depth(0, 45.0, 3)} notice=${ex.protege_notice_level(3)}`;
}

/** Load a built module and verify it. */
async function runWasm(path) {
  const bytes = await readFile(path);
  const { instance } = await WebAssembly.instantiate(bytes, {});
  return verifyExports(instance.exports);
}

/**
 * `--selftest`: prove `verifyExports` accepts a correct boundary and rejects the ways it can be wrong.
 * Without this, the accept branch and the reject branches would first run in CI, and a checker whose
 * rejection path has never executed is not yet known to reject anything.
 */
function selftest() {
  // A shim that implements the arc correctly. The thresholds and capping are implemented here in the
  // fixture, which is the point -- the verifier checks the SHAPE of the contract, not a particular
  // implementation's internals.
  const good = {
    protege_tier: (rep) => {
      if (rep >= 10) return 3;
      if (rep >= 5) return 2;
      if (rep >= 2) return 1;
      return 0;
    },
    protege_question_depth: (verdict, apogee, tier) => {
      if (verdict === 3 || Number.isNaN(apogee)) return tier >= 1 ? 1 : 0;
      if (tier >= 3) return 3;
      return tier;
    },
    protege_notice_level: (tier) => {
      if (tier >= 3) return 3;
      return tier;
    },
    protege_design_hash: () => 0xcbf29ce484222325,
  };

  const results = [];
  try {
    verifyExports(good);
    results.push(["a correct boundary is accepted", true]);
  } catch (err) {
    results.push([`a correct boundary is accepted (${err.message})`, false]);
  }

  // The exact defect this check removes: a tier threshold that is wrong.
  const wrongTier = {
    ...good,
    protege_tier: (rep) => (rep >= 1 ? 1 : 0),
  };
  try {
    verifyExports(wrongTier);
    results.push(["a wrong tier threshold is rejected", false]);
  } catch {
    results.push(["a wrong tier threshold is rejected", true]);
  }

  // A question depth that exceeds the tier is not the kernel's prediction.
  const wrongDepth = {
    ...good,
    protege_question_depth: (verdict, apogee, tier) => 3,
  };
  try {
    verifyExports(wrongDepth);
    results.push(["a question depth that exceeds the tier is rejected", false]);
  } catch {
    results.push(["a question depth that exceeds the tier is rejected", true]);
  }

  // A refused launch that does not cap the depth is not the kernel's prediction.
  const wrongRefused = {
    ...good,
    protege_question_depth: (verdict, apogee, tier) => tier,
  };
  try {
    verifyExports(wrongRefused);
    results.push(["a refused launch that does not cap the depth is rejected", false]);
  } catch {
    results.push(["a refused launch that does not cap the depth is rejected", true]);
  }

  // A notice level that exceeds the tier is not the kernel's prediction.
  const wrongNotice = {
    ...good,
    protege_notice_level: () => 3,
  };
  try {
    verifyExports(wrongNotice);
    results.push(["a notice level that exceeds the tier is rejected", false]);
  } catch {
    results.push(["a notice level that exceeds the tier is rejected", true]);
  }

  let bad = 0;
  for (const [name, ok] of results) {
    console.log(`  ${ok ? "ok  " : "FAIL"} ${name}`);
    if (!ok) bad++;
  }
  return bad;
}

const html = await readFile(PAGE, "utf8");
const src = liveSource(html);

let failed = 0;
for (const { name, fn } of checks) {
  try {
    fn(src);
    console.log(`  ok   ${name}`);
  } catch (e) {
    failed++;
    console.log(`  FAIL ${name}`);
    console.log(`       ${e.message}`);
  }
}

if (process.argv.includes("--selftest")) {
  const bad = selftest();
  console.log("");
  console.log(bad ? `${bad} self-test(s) FAILED` : "self-test passed: the verifier accepts good exports and rejects bad ones");
  process.exit(bad ? 1 : 0);
}

const wasmArg = process.argv.indexOf("--wasm");
if (wasmArg !== -1) {
  const path = process.argv[wasmArg + 1];
  try {
    const detail = await runWasm(path);
    console.log(`  ok   wasm boundary: ${detail}`);
  } catch (e) {
    failed++;
    console.log(`  FAIL wasm boundary`);
    console.log(`       ${e.message}`);
  }
} else {
  console.log("  ..   wasm boundary not run (pass --wasm <file> where a build exists)");
}

console.log("");
if (failed) {
  console.log(`${failed} protégé check(s) FAILED`);
  process.exit(1);
}
console.log(`all ${checks.length} protégé checks passed`);
