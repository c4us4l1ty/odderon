#!/usr/bin/env node
/**
 * The gate that makes the survey seam a real kernel boundary rather than a page-side star field.
 *
 * `web/index.html` answers "what is in this patch of sky?" by calling the kernel's `survey_*`
 * exports — the seam over `odderon_astro::survey`, which validates the cone request and the
 * backend's reply. The page owns the star data (a backend, not a table); the kernel validates
 * the request, checks the reply, and returns only the stars inside the cone. The same
 * "no substitute" contract the orbit, the pad, the rocket, the museum and the light-cone follow:
 * the page computes no cone-search logic, holds no separation formula, and shows no star field
 * when the kernel is absent.
 *
 * The checks:
 *
 *   1. NO LOCAL CONE MATH. The page may read the kernel's survey exports but must not compute
 *      a cone search itself: no local separation check, no local direction normalisation, no
 *      local star filtering. The failure mode is "someone re-adds a page-side search", which
 *      no runtime probe can see.
 *
 *   2. THE PAGE REQUIRES EVERY SURVEY EXPORT. The page must reference all kernel accessors:
 *      `survey_cone_search`, `survey_admits`, and `survey_design_hash`. A partial build fails
 *      at load.
 *
 *   3. NO SILENT FALLBACK. `surveyConeSearch` must return a named error when the kernel is
 *      absent, and the panel must show a refusal rather than a fabricated star field.
 *
 *   4. THE SEARCH READINGS COME FROM THE KERNEL. `surveyConeSearch` must call
 *      `survey_cone_search`, not filter stars with a local formula.
 *
 *   5. THE WASM BOUNDARY IS PHYSICAL. When a build is available, the exports must reproduce
 *      the invariants of the cone search: a valid request returns only stars inside the cone,
 *      a bad radius is refused (negative return), a non-finite coordinate is refused, a
 *      non-redistributable backend is refused in a release build, and the design hash is
 *      deterministic and non-zero.
 *
 * Usage: node tools/check-survey.mjs            (static: no browser, no wasm needed)
 *        node tools/check-survey.mjs --selftest  (verify the verifier accepts good exports)
 *        node tools/check-survey.mjs --wasm dist/odderon_dmath_wasm.wasm
 *                                                    (also run the exports, if a build exists)
 */

import { readFile } from "node:fs/promises";

const PAGE = new URL("../web/index.html", import.meta.url);

/** Strip comments so documentation about the survey does not trip the checks that hunt for a search. */
function liveSource(html) {
  const script = html.match(/<script type="module">([\s\S]*?)<\/script>/);
  if (!script) throw new Error("no module script found in web/index.html");
  return script[1]
    .replace(/\/\*[\s\S]*?\*/g, "")
    .replace(/(^|[^:])\/\/.*$/gm, "$1");
}

const checks = [];
function check(name, fn) {
  checks.push({ name, fn });
}

check("no local cone math in the page", (src) => {
  // A local search would carry a separation formula, a direction normalisation, or a star
  // filter. Each is the giveaway that the page is computing the search itself. The check is
  // scoped to the survey functions (surveyConeSearch and renderSurvey) because the page's
  // shader code legitimately uses direction math for rendering, which is a different concern.
  const searchBody = src.match(/function surveyConeSearch\s*\([^)]*\)\s*\{[\s\S]*?\n\}/)?.[0] || "";
  const renderBody = src.match(/function renderSurvey\s*\([^)]*\)\s*\{[\s\S]*?\n\}/)?.[0] || "";
  const scoped = searchBody + "\n" + renderBody;
  const patterns = [
    [/Math\.acos\s*\([^)]*dot[^)]*\)/i, "a local acos-based separation"],
    [/Math\.atan2\s*\([^)]*cross[^)]*\)/i, "a local cross-product separation"],
    [/separation\s*\(\s*\w+\s*,\s*\w+\s*\)/i, "a local separation call"],
    [/stars\.filter\s*\(/i, "a local star filter"],
    [/\.filter\s*\([^)]*separation/i, "a local filter on separation"],
    [/from_ra_dec|fromRaDec/i, "a local direction construction"],
    [/SURVEY_STARS\.filter/i, "filtering the star array in the page"],
  ];
  const hits = patterns.filter(([re]) => re.test(scoped)).map(([, why]) => why);
  if (hits.length) throw new Error("page still computes the cone search itself: " + hits.join("; "));
});

check("the page requires every survey export", (src) => {
  const needed = [
    "survey_cone_search",
    "survey_admits",
    "survey_design_hash",
  ];
  const missing = needed.filter((n) => !src.includes(n));
  if (missing.length) {
    throw new Error("page does not reference: " + missing.join(", "));
  }
});

check("a missing kernel hides the survey rather than substituting a search", (src) => {
  // The contract: when the kernel is absent, `surveyConeSearch` returns a named error, and
  // the panel shows a refusal rather than a fabricated star field.
  if (!/function surveyConeSearch\s*\([^)]*\)\s*\{[\s\S]{0,200}?if\s*\(!k\s*\)\s*return\s*\{[^}]*error:\s*'kernel not loaded'/.test(src)) {
    throw new Error("surveyConeSearch does not return a named error when the kernel is missing");
  }
});

check("the search readings come from the kernel, not a local formula", (src) => {
  const m = src.match(/function surveyConeSearch\s*\([^)]*\)\s*\{([\s\S]*?)\n\}/);
  if (!m) throw new Error("surveyConeSearch not found");
  const body = m[1];
  if (!body.includes("survey_cone_search")) {
    throw new Error("surveyConeSearch does not call survey_cone_search");
  }
  if (/k\.survey_cone_search\s*\([^)]*\)\s*=[^;]*Math\./.test(body)) {
    throw new Error("surveyConeSearch assigns a hand-computed value to survey_cone_search's result");
  }
});

/**
 * The cone-search invariants the survey exports must satisfy, as a function of the exports alone.
 *
 * Separated from the file loading because the wasm build only exists in CI: if the verification
 * lived inside `runWasm`, none of it could be executed in this sandbox, and "checked in CI" would
 * be the whole of its testing. `--selftest` drives this with a deliberate shim so both the accept
 * and reject paths run here.
 */
function verifyExports(ex) {
  const need = [
    "survey_cone_search",
    "survey_admits",
    "survey_design_hash",
  ];
  const missing = need.filter((n) => typeof ex[n] !== "function");
  if (missing.length) throw new Error("module is missing: " + missing.join(", "));

  const problems = [];

  const memory = ex.memory;
  if (!memory) throw new Error("module has no memory export");
  const NEED = 1 << 20;
  while (memory.buffer.byteLength < NEED + (1 << 21)) {
    try { memory.grow(256); } catch (e) { throw new Error("cannot grow memory: " + e.message); }
  }
  const dv = new DataView(memory.buffer);
  const STAR_FLOATS = 10;
  const STAR_OFF = NEED;
  const OUT_OFF = STAR_OFF + 8 * STAR_FLOATS * 8;

  // Write 4 star records: id, x, y, z, dist, dist_sig, mag, mag_sig, pm, pm_sig.
  // Star 0: at (1, 0, 0), dist 10, sigma 0.5 — 0 rad from centre
  // Star 1: at (cos(0.5), sin(0.5), 0), dist 20, sigma 1.0 — 0.5 rad from centre
  // Star 2: at (cos(0.8), sin(0.8), 0), dist 30, sigma 2.0 — 0.8 rad from centre
  // Star 3: at (-1, 0, 0), dist 40, sigma 3.0 — pi rad from centre (opposite side)
  const stars = [
    [1, 1.0, 0.0, 0.0, 10.0, 0.5, 4.0, 0.02, NaN, NaN],
    [2, 0.8775825618903728, 0.479425538604203, 0.0, 20.0, 1.0, 5.0, 0.03, NaN, NaN],
    [3, 0.6967067093471654, 0.7173560908995228, 0.0, 30.0, 2.0, 6.0, 0.04, NaN, NaN],
    [4, -1.0, 0.0, 0.0, 40.0, 3.0, 7.0, 0.05, NaN, NaN],
  ];
  for (let i = 0; i < stars.length; i++) {
    const base = STAR_OFF + i * STAR_FLOATS * 8;
    for (let j = 0; j < STAR_FLOATS; j++) {
      dv.setFloat64(base + j * 8, stars[i][j], true);
    }
  }

  // Search a cone centred on star 0 (RA=0, dec=0), radius 0.5 rad (~28.6 deg).
  // Star 0 is at 0 rad, star 1 at 0.5 rad (on the boundary), star 2 at 0.8 rad (outside).
  // The kernel uses <= radius, so star 1 at exactly 0.5 should be included.
  const count = ex.survey_cone_search(STAR_OFF, 4, 0.0, 0.0, 0.5, OUT_OFF, 256);
  if (count < 0) {
    problems.push(`survey_cone_search returned ${count}, want a non-negative count`);
  } else {
    // Read back the first star's id.
    const id0 = dv.getFloat64(OUT_OFF + 0, true);
    if (id0 !== 1) problems.push(`first match id = ${id0}, want 1`);
  }

  // A wider cone (1.0 rad, ~57.3 deg) should catch stars 0, 1, 2 but not star 3 (at -1,0,0,
  // which is pi rad away).
  const countWide = ex.survey_cone_search(STAR_OFF, 4, 0.0, 0.0, 1.0, OUT_OFF, 256);
  if (countWide < 0) {
    problems.push(`wide cone returned ${countWide}, want a non-negative count`);
  } else if (countWide !== 3) {
    problems.push(`wide cone returned ${countWide} stars, want 3`);
  }

  // A bad radius (zero) is refused with a negative code.
  const badRadius = ex.survey_cone_search(STAR_OFF, 4, 0.0, 0.0, 0.0, OUT_OFF, 256);
  if (badRadius >= 0) problems.push(`zero radius returned ${badRadius}, want a negative refusal`);

  // A non-finite coordinate is refused.
  const badCoord = ex.survey_cone_search(STAR_OFF, 4, NaN, 0.0, 0.5, OUT_OFF, 256);
  if (badCoord >= 0) problems.push(`NaN coordinate returned ${badCoord}, want a negative refusal`);

  // The release rule: a redistributable backend is admitted in both profiles; a
  // non-redistributable backend is admitted only in development.
  const devRedistributable = ex.survey_admits(1, 0);
  const relRedistributable = ex.survey_admits(1, 1);
  const devNonRedistributable = ex.survey_admits(0, 0);
  const relNonRedistributable = ex.survey_admits(0, 1);
  if (devRedistributable !== 1) problems.push(`survey_admits(redistributable, dev) = ${devRedistributable}, want 1`);
  if (relRedistributable !== 1) problems.push(`survey_admits(redistributable, release) = ${relRedistributable}, want 1`);
  if (devNonRedistributable !== 1) problems.push(`survey_admits(non-redistributable, dev) = ${devNonRedistributable}, want 1`);
  if (relNonRedistributable !== 0) problems.push(`survey_admits(non-redistributable, release) = ${relNonRedistributable}, want 0`);

  // Design hash is deterministic and non-zero.
  const h1 = ex.survey_design_hash();
  const h2 = ex.survey_design_hash();
  if (h1 !== h2) problems.push("design hash is not deterministic");
  if (h1 === 0) problems.push("design hash is zero");

  if (problems.length) throw new Error("boundary disagrees with the survey seam: " + problems.join("; "));
  return `stars=${count} wide=${countWide} badRadius=${badRadius} badCoord=${badCoord}`;
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
  // A shim that implements the cone search correctly. The formulas are implemented here in the
  // fixture, which is the point -- the verifier checks the SHAPE of the contract, not a particular
  // implementation's internals.
  const STAR_FLOATS = 10;
  const memory = (() => {
    const buf = new ArrayBuffer(1 << 22);
    return { buffer: buf, grow: () => {} };
  })();

  function separation(ax, ay, az, bx, by, bz) {
    const dot = ax * bx + ay * by + az * bz;
    const crossX = ay * bz - az * by;
    const crossY = az * bx - ax * bz;
    const crossZ = ax * by - ay * bx;
    const crossNorm = Math.sqrt(crossX * crossX + crossY * crossY + crossZ * crossZ);
    return Math.atan2(crossNorm, dot);
  }

  const STAR_DATA = [
    [1, 1.0, 0.0, 0.0, 10.0, 0.5, 4.0, 0.02, NaN, NaN],
    [2, 0.8775825618903728, 0.479425538604203, 0.0, 20.0, 1.0, 5.0, 0.03, NaN, NaN],
    [3, 0.6967067093471654, 0.7173560908995228, 0.0, 30.0, 2.0, 6.0, 0.04, NaN, NaN],
    [4, -1.0, 0.0, 0.0, 40.0, 3.0, 7.0, 0.05, NaN, NaN],
  ];

  const good = {
    memory,
    survey_cone_search: (starsOff, n, ra, dec, radius, outOff, outCap) => {
      if (!Number.isFinite(ra) || !Number.isFinite(dec)) return -2;
      if (!(radius > 0) || radius > Math.PI / 2) return -3;
      const dv = new DataView(memory.buffer);
      // Centre direction from RA/dec.
      const cx = Math.cos(dec) * Math.cos(ra);
      const cy = Math.cos(dec) * Math.sin(ra);
      const cz = Math.sin(dec);
      let count = 0;
      for (let i = 0; i < n; i++) {
        const base = starsOff + i * STAR_FLOATS * 8;
        const x = dv.getFloat64(base + 8, true);
        const y = dv.getFloat64(base + 16, true);
        const z = dv.getFloat64(base + 24, true);
        const sep = separation(cx, cy, cz, x, y, z);
        if (sep <= radius) {
          if (count >= outCap) return -1; // truncated (shim returns error for simplicity)
          for (let j = 0; j < STAR_FLOATS; j++) {
            dv.setFloat64(outOff + count * STAR_FLOATS * 8 + j * 8, dv.getFloat64(base + j * 8, true), true);
          }
          count++;
        }
      }
      return count;
    },
    survey_admits: (redistributable, isRelease) => {
      if (isRelease !== 0 && redistributable === 0) return 0;
      return 1;
    },
    survey_design_hash: () => 0xcbf29ce484222325,
  };

  const results = [];
  try {
    verifyExports(good);
    results.push(["a correct boundary is accepted", true]);
  } catch (err) {
    results.push([`a correct boundary is accepted (${err.message})`, false]);
  }

  // A search that ignores the radius (returns all stars) is not the kernel's.
  const wrongRadius = { ...good, survey_cone_search: (starsOff, n, ra, dec, radius, outOff, outCap) => {
    if (!Number.isFinite(ra) || !Number.isFinite(dec)) return -2;
    const dv = new DataView(memory.buffer);
    let count = 0;
    for (let i = 0; i < n; i++) {
      const base = starsOff + i * STAR_FLOATS * 8;
      for (let j = 0; j < STAR_FLOATS; j++) {
        dv.setFloat64(outOff + count * STAR_FLOATS * 8 + j * 8, dv.getFloat64(base + j * 8, true), true);
      }
      count++;
    }
    return count;
  } };
  try {
    verifyExports(wrongRadius);
    results.push(["a search that ignores the radius is rejected", false]);
  } catch {
    results.push(["a search that ignores the radius is rejected", true]);
  }

  // A release rule that admits a non-redistributable backend is not the kernel's.
  const wrongAdmits = { ...good, survey_admits: () => 1 };
  try {
    verifyExports(wrongAdmits);
    results.push(["a release rule that admits everything is rejected", false]);
  } catch {
    results.push(["a release rule that admits everything is rejected", true]);
  }

  // A non-deterministic design hash is not the kernel's.
  const wrongHash = { ...good, survey_design_hash: () => Math.random() };
  try {
    verifyExports(wrongHash);
    results.push(["a non-deterministic design hash is rejected", false]);
  } catch {
    results.push(["a non-deterministic design hash is rejected", true]);
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
  console.log(`${failed} survey check(s) FAILED`);
  process.exit(1);
}
console.log(`all ${checks.length} survey checks passed`);
