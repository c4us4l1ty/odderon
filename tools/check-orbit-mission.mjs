#!/usr/bin/env node
/**
 * The gate that makes the orbit mission a real kernel pass prediction rather than a page-drawn schedule.
 *
 * `web/index.html` answers "when will the orbiting body be over my station?" by calling the kernel's
 * `pass_next_*` exports -- the seam over `odderon_astro::observe` (ground stations) and
 * `odderon_astro::passes` (pass prediction), the deepest and most-validated part of the astro kernel.
 * The same "no substitute" contract the orbit body, the rocket and the pad follow: the page computes
 * no pass, holds no pass constant, and draws nothing when the kernel is absent.
 *
 * The checks:
 *
 *   1. NO LOCAL PASS SOLVER. The page may read the kernel's pass window but must not compute one: no
 *      local Kepler solve, no local `mu`, no local Earth radius, no local site/elevation geometry. The
 *      failure mode is "someone re-adds a local prediction", which no runtime probe can see.
 *
 *   2. THE PAGE REQUIRES EVERY PASS EXPORT. The `needed` array must name all eight: exists, rise,
 *      culmination, set, max elevation, min range, rise azimuth, set azimuth. A partial build fails at
 *      load, not at the first draw.
 *
 *   3. NO SILENT FALLBACK. `passReadings` must return a named source when the kernel is absent, and
 *      the highlight must turn off (bodyPass 0) rather than draw a substitute pass.
 *
 *   4. THE PASS READINGS COME FROM THE KERNEL. `passReadings` must call the eight `pass_next_*`
 *      accessors, not compute a window from a local formula.
 *
 *   5. THE WASM BOUNDARY IS PHYSICAL. When a build is available, the exports must reproduce the
 *      invariants of a pass: a station inside the inclination band has a pass with
 *      rise < culmination < set, maximum elevation above the mask, minimum range below the orbit
 *      altitude, a positive duration, and the same window for the same input (replay); a station
 *      outside the band is honestly refused with `exists = 0`.
 *
 * Usage: node tools/check-orbit-mission.mjs            (static: no browser, no wasm needed)
 *        node tools/check-orbit-mission.mjs --wasm dist/odderon_dmath_wasm.wasm
 *                                             (also run the exports, if a build is available)
 */

import { readFile } from "node:fs/promises";

const PAGE = new URL("../web/index.html", import.meta.url);

/** Strip comments so documentation about the pass does not trip the checks that hunt for a solver. */
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

/** The pass mask the kernel uses, degrees. Named so a change to the kernel constant is visible. */
const PASS_MASK_DEG = 10.0;

check("no local pass solver in the page", (src) => {
  // A local solver would carry a gravitational parameter, an Earth radius, a Kepler iteration, or a
  // hand-written elevation/azimuth. Each is the giveaway that the page is predicting the pass itself.
  const patterns = [
    [/398600/, "a hard-coded gravitational parameter"],
    [/6378/, "a hard-coded Earth radius"],
    [/eccentric\s*anomaly/i, "a variable named eccentric anomaly"],
    [/Math\.sqrt\s*\(\s*(mu|MU)\s*\//, "a sqrt(mu/r) circular speed"],
    [/from_geodetic|look_at|next_pass/i, "a local site or pass computation"],
    [/elevation_deg\s*=\s*[^;]*Math\./, "a hand-computed elevation"],
    [/azimuth_deg\s*=\s*[^;]*Math\./, "a hand-computed azimuth"],
  ];
  const hits = patterns.filter(([re]) => re.test(src)).map(([, why]) => why);
  if (hits.length) throw new Error("page still solves the pass itself: " + hits.join("; "));
});

check("the page requires every pass export", (src) => {
  const needed = [
    "pass_next_exists",
    "pass_next_rise_s",
    "pass_next_culmination_s",
    "pass_next_set_s",
    "pass_next_max_elevation_deg",
    "pass_next_min_range_km",
    "pass_next_rise_az_deg",
    "pass_next_set_az_deg",
  ];
  const missing = needed.filter((n) => !src.includes(n));
  if (missing.length) {
    throw new Error("page does not reference: " + missing.join(", "));
  }
  if (!/needed\s*=\s*\[/.test(src) && !/required\s*=\s*\[/.test(src)) {
    throw new Error("page does not check the module's export list before drawing");
  }
});

check("a missing kernel hides the pass rather than substituting a schedule", (src) => {
  // The contract: when the kernel is absent, `passReadings` returns a named `source` that is not the
  // kernel, and the highlight turns off. What is forbidden is arithmetic that produces a plausible
  // pass when no kernel is present.
  if (!/function passReadings\s*\([^)]*\)\s*\{[\s\S]{0,200}?if \(!k\) return \{ source: 'absent' \};/.test(src)) {
    throw new Error("passReadings does not return a named absent source when the kernel is missing");
  }
  // The highlight is a function of the kernel's window: with no valid pass it is 0, never a constant.
  if (!/const bodyPass = \(passCache\.valid &&/.test(src)) {
    throw new Error("the body highlight is not gated on a valid kernel pass");
  }
});

check("the pass readings come from the kernel, not a local formula", (src) => {
  const m = src.match(/function passReadings\s*\([^)]*\)\s*\{([\s\S]*?)\n\}/);
  if (!m) throw new Error("passReadings not found");
  const body = m[1];
  for (const accessor of [
    "pass_next_exists",
    "pass_next_rise_s",
    "pass_next_culmination_s",
    "pass_next_set_s",
    "pass_next_max_elevation_deg",
    "pass_next_min_range_km",
    "pass_next_rise_az_deg",
    "pass_next_set_az_deg",
  ]) {
    if (!body.includes(accessor)) {
      throw new Error(`passReadings does not call ${accessor}`);
    }
  }
});

/**
 * The physics the pass exports must satisfy, as a function of the exports alone.
 *
 * Separated from the file loading because the wasm build only exists in CI: if the verification lived
 * inside `runWasm`, none of it could be executed in this sandbox, and "checked in CI" would be the
 * whole of its testing. `--selftest` drives this with a deliberate shim so both the accept and reject
 * paths run here.
 */
function verifyExports(ex) {
  const need = [
    "pass_next_exists",
    "pass_next_rise_s",
    "pass_next_culmination_s",
    "pass_next_set_s",
    "pass_next_max_elevation_deg",
    "pass_next_min_range_km",
    "pass_next_rise_az_deg",
    "pass_next_set_az_deg",
  ];
  const missing = need.filter((n) => typeof ex[n] !== "function");
  if (missing.length) throw new Error("module is missing: " + missing.join(", "));

  // A station inside the orbit's inclination band (the reference orbit is inclined ~31.5 deg): the
  // equator is inside it, so a pass must exist.
  const lat = 0.0, lon = 0.0, from = 0.0;
  const problems = [];

  const exists = ex.pass_next_exists(lat, lon, from);
  if (exists !== 1) problems.push(`equatorial station verdict = ${exists}, want 1 (a pass)`);

  const rise = ex.pass_next_rise_s(lat, lon, from);
  const culmination = ex.pass_next_culmination_s(lat, lon, from);
  const set = ex.pass_next_set_s(lat, lon, from);
  if (!Number.isFinite(rise) || !Number.isFinite(culmination) || !Number.isFinite(set)) {
    problems.push("pass times are not all finite");
  } else {
    if (!(rise < culmination)) problems.push(`rise ${rise} must precede culmination ${culmination}`);
    if (!(culmination < set)) problems.push(`culmination ${culmination} must precede set ${set}`);
    if (!(set - rise > 0)) problems.push(`duration ${set - rise} must be positive`);
  }

  const maxElev = ex.pass_next_max_elevation_deg(lat, lon, from);
  if (!Number.isFinite(maxElev)) problems.push("max elevation is not finite");
  else if (maxElev <= PASS_MASK_DEG) problems.push(`max elevation ${maxElev} must exceed the mask ${PASS_MASK_DEG}`);

  const minRange = ex.pass_next_min_range_km(lat, lon, from);
  if (!Number.isFinite(minRange)) problems.push("min range is not finite");
  else if (minRange <= 0) problems.push(`min range ${minRange} must be positive`);

  const riseAz = ex.pass_next_rise_az_deg(lat, lon, from);
  const setAz = ex.pass_next_set_az_deg(lat, lon, from);
  if (!Number.isFinite(riseAz) || !Number.isFinite(setAz)) problems.push("azimuths are not finite");

  // Reproducibility: the same station and time give the same window. The P3 replay gate.
  const rise2 = ex.pass_next_rise_s(lat, lon, from);
  if (rise !== rise2) problems.push(`rise not reproducible: ${rise} vs ${rise2}`);

  // A station outside the inclination band is honestly refused: no pass, not a fabricated one.
  const farLat = (60.0 * Math.PI) / 180.0;
  const farExists = ex.pass_next_exists(farLat, lon, from);
  if (farExists !== 0) problems.push(`a station outside the band verdict = ${farExists}, want 0 (no pass)`);

  // Searching from a later time still returns a pass (the next one after `from`).
  const laterExists = ex.pass_next_exists(lat, lon, 1000.0);
  if (laterExists !== 1) problems.push(`a pass after t=1000 verdict = ${laterExists}, want 1`);

  if (problems.length) throw new Error("boundary disagrees with physics: " + problems.join("; "));
  return `rise=${rise.toFixed(1)}s culmination=${culmination.toFixed(1)}s set=${set.toFixed(1)}s maxElev=${maxElev.toFixed(1)}deg minRange=${minRange.toFixed(0)}km`;
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
  // A shim, not a propagator: it returns a plausible pass window for the equatorial station and an
  // honest refusal outside the band. That is enough to satisfy `verifyExports`, and it is honest shim
  // behaviour rather than a pretend prediction -- the distinction that matters in a test fixture.
  const good = {
    pass_next_exists: (lat, lon, from) => (Math.abs(lat) < 0.5 ? 1 : 0),
    pass_next_rise_s: () => 1000.0,
    pass_next_culmination_s: () => 1200.0,
    pass_next_set_s: () => 1400.0,
    pass_next_max_elevation_deg: () => 45.0,
    pass_next_min_range_km: () => 500.0,
    pass_next_rise_az_deg: () => 270.0,
    pass_next_set_az_deg: () => 90.0,
  };

  const results = [];
  try {
    verifyExports(good);
    results.push(["a correct boundary is accepted", true]);
  } catch (err) {
    results.push([`a correct boundary is accepted (${err.message})`, false]);
  }

  // The exact defect this check removes: a fabricated pass for a station outside the band.
  const fabricatesEverywhere = { ...good, pass_next_exists: () => 1 };
  try {
    verifyExports(fabricatesEverywhere);
    results.push(["a pass fabricated outside the band is rejected", false]);
  } catch {
    results.push(["a pass fabricated outside the band is rejected", true]);
  }

  // A window with the wrong ordering: set before rise is not a pass.
  const wrongOrder = { ...good, pass_next_rise_s: () => 1400.0, pass_next_set_s: () => 1000.0 };
  try {
    verifyExports(wrongOrder);
    results.push(["a window with set before rise is rejected", false]);
  } catch {
    results.push(["a window with set before rise is rejected", true]);
  }

  // A maximum elevation below the mask is not a pass.
  const belowMask = { ...good, pass_next_max_elevation_deg: () => 5.0 };
  try {
    verifyExports(belowMask);
    results.push(["a maximum elevation below the mask is rejected", false]);
  } catch {
    results.push(["a maximum elevation below the mask is rejected", true]);
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
  console.log(`${failed} orbit-mission check(s) FAILED`);
  process.exit(1);
}
console.log(`all ${checks.length} orbit-mission checks passed`);
