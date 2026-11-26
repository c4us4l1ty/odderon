#!/usr/bin/env node
/**
 * The gate that makes the sounding rocket a real kernel flight rather than a page-drawn arc.
 *
 * `web/index.html` renders the rocket's trajectory by sampling the kernel's `rocket_state_*` exports
 * and uploading the points to the shader. That is the same "no substitute" contract the orbit body
 * follows, and the P3 gate -- bit-identical replay in a second browser -- is a claim about the
 * trajectory being a pure function of the authored launch parameters and `t`. This check asserts the
 * page actually calls the kernel for every rocket number, and that the kernel's exports satisfy the
 * physics invariants of a suborbital flight.
 *
 * The checks:
 *
 *   1. NO LOCAL ROCKET SOLVER. The page may adapt the kernel's position (a tilt and a scale) but must
 *      not solve the trajectory itself: no Kepler iteration, no `sqrt(mu/r)`, no local `mu`, no
 *      hand-written apogee. The failure mode is "someone re-adds a local trajectory", which no runtime
 *      probe can see.
 *
 *   2. THE PAGE REQUIRES EVERY ROCKET EXPORT. The `needed` array must name all nine: the three
 *      position accessors, the four reading accessors, the verdict, and the design hash. A partial
 *      build (say, position but no readings) fails at load, not at the first draw.
 *
 *   3. NO SILENT FALLBACK. `rocketPosition` must return `null` when the kernel is absent, and the
 *      trajectory must not be drawn from a substitute. A page that quietly substitutes a JS arc is a
 *      page that lies about which code produced the picture.
 *
 *   4. THE ROCKET IS DRAWN FROM THE KERNEL'S POSITION. `rocketPosition` must call the three
 *      `rocket_state_*` accessors, not compute a position from a local formula.
 *
 *   5. THE WASM BOUNDARY IS PHYSICAL. When a build is available, the exports must reproduce the
 *      invariants of a suborbital flight: the inherited launch is suborbital and short of the Kármán
 *      line; the impact time returns the rocket to the surface; a vertical launch is bound (not
 *      escape); the verdict discriminates by energy; the design hash is stable.
 *
 * Usage: node tools/check-rocket.mjs            (static: no browser, no wasm needed)
 *        node tools/check-rocket.mjs --wasm dist/odderon_dmath_wasm.wasm
 *                                             (also run the exports, if a build is available)
 */

import { readFile } from "node:fs/promises";

const PAGE = new URL("../web/index.html", import.meta.url);

/** Strip comments so documentation about the rocket does not trip the checks that hunt for a solver. */
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

/** The Kármán line, km. Named in the check so a change to the kernel constant is a visible failure. */
const KARMAN = 100.0;
/** Earth's surface radius, km, as the kernel defines it. */
const R_EARTH = 6378.137;

check("no local rocket trajectory solver in the page", (src) => {
  // A local solver would carry a gravitational parameter, a Kepler iteration, or a hand-written apogee.
  // Each is the giveaway that the page is computing the flight instead of asking the kernel for it.
  const patterns = [
    [/398600/, "a hard-coded gravitational parameter"],
    [/6378/, "a hard-coded Earth radius"],
    [/eccentric\s*anomaly/i, "a variable named eccentric anomaly"],
    [/Math\.sqrt\s*\(\s*(mu|MU)\s*\//, "a sqrt(mu/r) circular speed"],
    [/apogee\s*=\s*[^;]*Math\./, "a hand-computed apogee"],
    [/specific\s*energy/i, "a specific-energy computation"],
  ];
  const hits = patterns.filter(([re]) => re.test(src)).map(([, why]) => why);
  if (hits.length) throw new Error("page still solves the rocket itself: " + hits.join("; "));
});

check("the page requires every rocket export", (src) => {
  const needed = [
    "rocket_state_x",
    "rocket_state_y",
    "rocket_state_z",
    "rocket_apogee_km",
    "rocket_time_to_apogee_s",
    "rocket_impact_time_s",
    "rocket_range_km",
    "rocket_verdict",
    "rocket_design_hash",
  ];
  const missing = needed.filter((n) => !src.includes(n));
  if (missing.length) {
    throw new Error("page does not reference: " + missing.join(", "));
  }
  // Naming them in the required-exports array is what makes a partial build fail loudly at load time.
  if (!/needed\s*=\s*\[/.test(src) && !/required\s*=\s*\[/.test(src)) {
    throw new Error("page does not check the module's export list before drawing");
  }
});

check("a missing kernel hides the rocket rather than substituting a trajectory", (src) => {
  // The contract, the same as the orbit body's: when the kernel is absent the rocket position is
  // `null`, the path is `null`, and the uniforms turn the craft off. What is forbidden is arithmetic
  // that produces a plausible arc when no kernel is present.
  if (!/function rocketPosition\s*\([^)]*\)\s*\{[\s\S]{0,300}?if \(!k\) return null;/.test(src)) {
    throw new Error("rocketPosition does not return null when the kernel is missing");
  }
  if (!/function rocketTrajectory\s*\([^)]*\)\s*\{[\s\S]{0,300}?if \(!k\) return null;/.test(src)) {
    throw new Error("rocketTrajectory does not return null when the kernel is missing");
  }
  // The draw path must turn the uniforms off when the position is null, not draw a substitute.
  if (!/rocketOn,\s*rocketPos \? 1\.0 : 0\.0/.test(src)) {
    throw new Error("drawFrame does not turn the rocket off when the kernel gave no position");
  }
});

check("the rocket position comes from the kernel, not a local formula", (src) => {
  // `rocketPosition` must call the three state accessors. A local formula (even one that looks right)
  // would be the stub this check exists to make impossible.
  const m = src.match(/function rocketPosition\s*\([^)]*\)\s*\{([\s\S]*?)\n\}/);
  if (!m) throw new Error("rocketPosition not found");
  const body = m[1];
  for (const accessor of ["rocket_state_x", "rocket_state_y", "rocket_state_z"]) {
    if (!body.includes(accessor)) {
      throw new Error(`rocketPosition does not call ${accessor}`);
    }
  }
});

check("the rocket reads are a pure function of the authored parameters", (src) => {
  // `rocketReadings` must call the four reading accessors. If the page ever computed a reading itself,
  // the HUD would disagree with the rendered trajectory -- the two must come from the same kernel.
  const m = src.match(/function rocketReadings\s*\([^)]*\)\s*\{([\s\S]*?)\n\}/);
  if (!m) throw new Error("rocketReadings not found");
  const body = m[1];
  for (const accessor of [
    "rocket_apogee_km",
    "rocket_time_to_apogee_s",
    "rocket_impact_time_s",
    "rocket_range_km",
    "rocket_verdict",
  ]) {
    if (!body.includes(accessor)) {
      throw new Error(`rocketReadings does not call ${accessor}`);
    }
  }
});

/**
 * The physics the rocket exports must satisfy, as a function of the exports alone.
 *
 * Separated from the file loading because the wasm build only exists in CI: if the verification lived
 * inside `runWasm`, none of it could be executed in this sandbox, and "checked in CI" would be the
 * whole of its testing. `--selftest` drives this with a deliberate shim so both the accept and reject
 * paths run here.
 */
function verifyExports(ex) {
  const need = [
    "rocket_state_x",
    "rocket_state_y",
    "rocket_state_z",
    "rocket_apogee_km",
    "rocket_time_to_apogee_s",
    "rocket_impact_time_s",
    "rocket_range_km",
    "rocket_verdict",
    "rocket_design_hash",
  ];
  const missing = need.filter((n) => typeof ex[n] !== "function");
  if (missing.length) throw new Error("module is missing: " + missing.join(", "));

  // The inherited launch: 45 deg elevation, 1.5 km/s, north, at the equator. Radians at the boundary.
  const lat = 0, az = 0, el = Math.PI / 4, v = 1.5;
  const problems = [];

  const verdict = ex.rocket_verdict(lat, az, el, v);
  if (verdict !== 0) problems.push(`inherited verdict = ${verdict}, want 0 (suborbital)`);

  const apogee = ex.rocket_apogee_km(lat, az, el, v);
  if (!Number.isFinite(apogee)) problems.push("inherited apogee is not finite");
  if (apogee >= KARMAN) problems.push(`inherited apogee ${apogee} km reached space; the default must fall short`);

  const impact = ex.rocket_impact_time_s(lat, az, el, v);
  if (!Number.isFinite(impact) || impact <= 0) problems.push(`inherited impact time = ${impact}`);

  const range = ex.rocket_range_km(lat, az, el, v);
  if (!Number.isFinite(range) || range <= 0) problems.push(`inherited range = ${range}`);

  // The impact time must return the rocket to the surface: propagate to impact and the radius is R.
  const rImpact = Math.hypot(
    ex.rocket_state_x(lat, az, el, v, impact),
    ex.rocket_state_y(lat, az, el, v, impact),
    ex.rocket_state_z(lat, az, el, v, impact),
  );
  if (Math.abs(rImpact - R_EARTH) > 1.0) problems.push(`r at impact = ${rImpact}, want ~${R_EARTH}`);

  // A vertical launch is bound, not escape: the energy discriminator, not eccentricity, decides.
  const vVert = ex.rocket_verdict(0, 0, Math.PI / 2, 3.0);
  if (vVert !== 0) problems.push(`vertical 3 km/s verdict = ${vVert}, want 0 (bound suborbital)`);

  // A high-velocity vertical launch below escape is still bound and recoverable.
  const vVertFast = ex.rocket_verdict(0, 0, Math.PI / 2, 5.0);
  if (vVertFast !== 0) problems.push(`vertical 5 km/s verdict = ${vVertFast}, want 0 (bound suborbital)`);

  // Escape is decided by energy: 12 km/s vertical escapes.
  const vEsc = ex.rocket_verdict(0, 0, Math.PI / 2, 12.0);
  if (vEsc !== 2) problems.push(`vertical 12 km/s verdict = ${vEsc}, want 2 (escape)`);

  // A horizontal launch above circular velocity is orbital: no impact.
  const vOrb = ex.rocket_verdict(0, Math.PI / 2, 0, 8.0);
  if (vOrb !== 1) problems.push(`horizontal 8 km/s verdict = ${vOrb}, want 1 (orbital)`);

  // Reproducibility: the same parameters, twice, give the same readings. This is the P3 gate.
  const a1 = ex.rocket_apogee_km(lat, az, el, v);
  const a2 = ex.rocket_apogee_km(lat, az, el, v);
  if (a1 !== a2) problems.push(`apogee not reproducible: ${a1} vs ${a2}`);

  // The design hash is stable and non-zero.
  const h1 = ex.rocket_design_hash();
  const h2 = ex.rocket_design_hash();
  if (h1 !== h2 || h1 === 0) problems.push(`design hash not stable/non-zero: ${h1}`);

  if (problems.length) throw new Error("boundary disagrees with physics: " + problems.join("; "));
  return `inherited apogee=${apogee.toFixed(2)}km impact=${impact.toFixed(2)}s range=${range.toFixed(2)}km hash=${h1}`;
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
  const R = R_EARTH;
  // A shim, not a propagator: it returns plausible readings for the inherited launch and sensible
  // verdicts. That is enough to satisfy `verifyExports`, and it is honest shim behaviour rather than a
  // pretend trajectory -- the distinction that matters in a test fixture.
  const good = {
    rocket_state_x: (lat, az, el, v, t) => R * Math.cos(0),
    rocket_state_y: () => 0,
    rocket_state_z: () => R * Math.sin(0),
    rocket_apogee_km: () => 59.0,
    rocket_time_to_apogee_s: () => 111.6,
    rocket_impact_time_s: () => 223.2,
    rocket_range_km: () => 233.8,
    rocket_verdict: (lat, az, el, v) => {
      if (v >= 11.2) return 2;
      if (v >= 7.9) return 1;
      return 0;
    },
    rocket_design_hash: () => 123456789,
  };

  const results = [];
  try {
    verifyExports(good);
    results.push(["a correct boundary is accepted", true]);
  } catch (err) {
    results.push([`a correct boundary is accepted (${err.message})`, false]);
  }

  // The exact defect this check removes: a local solver that reaches space on the default launch.
  const reachesSpace = { ...good, rocket_apogee_km: () => 150.0 };
  try {
    verifyExports(reachesSpace);
    results.push(["a default that reaches space is rejected", false]);
  } catch {
    results.push(["a default that reaches space is rejected", true]);
  }

  // A vertical launch misclassified as escape: the eccentricity-only discriminator's failure.
  const misclassifiesRadial = { ...good, rocket_verdict: () => 2 };
  try {
    verifyExports(misclassifiesRadial);
    results.push(["a radial launch misclassified as escape is rejected", false]);
  } catch {
    results.push(["a radial launch misclassified as escape is rejected", true]);
  }

  // A non-reproducible reading: the replay gate's failure.
  const nonReproducible = { ...good, rocket_apogee_km: () => Math.random() * 100 };
  try {
    verifyExports(nonReproducible);
    results.push(["a non-reproducible reading is rejected", false]);
  } catch {
    results.push(["a non-reproducible reading is rejected", true]);
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
  console.log(`${failed} rocket check(s) FAILED`);
  process.exit(1);
}
console.log(`all ${checks.length} rocket checks passed`);
