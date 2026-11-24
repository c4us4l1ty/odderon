#!/usr/bin/env node
/**
 * The gate that makes the JavaScript orbit stub mechanically impossible to leave behind.
 *
 * `web/index.html` used to draw its satellite by solving Kepler's equation in JS and returning a
 * position. It was labelled as a stand-in, and the label was true — but a label is not a guard: the
 * stub was described in three places as "to be replaced" and survived every check in the repo, because
 * every check asked whether the page *rendered*, and the stub rendered beautifully.
 *
 * This asserts the replacement actually happened, and asserts it in the only way that cannot be talked
 * around: **the page must call the kernel's exports, and must not contain an orbit solver of its own.**
 *
 * Four checks, each one able to fail against the code as it was:
 *
 *   1. THE STUB IS GONE. The string `orbitPosition` is fine (it is the page's adapter) but the *solver*
 *      must not be there: no Newton iteration on Kepler's equation, no `sqrt(mu / r)` velocity, and no
 *      local definition of the gravitational parameter. These are positive/negative textual checks
 *      because the failure mode is "someone re-adds a local solution", which no runtime probe can see.
 *
 *   2. THE PAGE LOADS THE MODULE AND NAMES WHAT IT NEEDS. `WebAssembly.instantiate` with no imports,
 *      and the six state/velocity accessors in the required-exports list. A partially wired page (say,
 *      position only) fails here.
 *
 *   3. VELOCITY CROSSES THE BOUNDARY. The old stub returned a position and nothing else, which is the
 *      specific defect: a position-only state cannot be advanced across an idle gap. The page must
 *      call all three velocity accessors.
 *
 *   4. NO SILENT FALLBACK. There must be no numeric fallback when the kernel is missing — the function
 *      returns `null` and callers handle it. A page that quietly substitutes a JS orbit is a page that
 *      lies about which code produced the picture.
 *
 * Usage: node tools/check-orbit.mjs            (static: no browser, no wasm needed)
 *        node tools/check-orbit.mjs --wasm dist/odderon_dmath_wasm.wasm
 *                                             (also run the exports, if a build is available)
 */

import { readFile } from "node:fs/promises";

const PAGE = new URL("../web/index.html", import.meta.url);

/** Strip comments so that documentation *about* the old stub does not trip the checks that hunt it. */
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

/**
 * Kepler's equation has exactly one standard shape: `M = E - e sin E`, solved by Newton on `E`.
 * A local solver will contain an iteration that updates an eccentric anomaly. The kernel does that
 * too — but in wasm, not in this file.
 */
check("no local Kepler solver in the page", (src) => {
  const patterns = [
    [/eccentric\s*anomaly/i, "a variable named eccentric anomaly"],
    [/E\s*=\s*E\s*-/, "a Newton update of E"],
    [/E\s*-=\s*\(/, "a Newton update of E"],
    [/M\s*=\s*E\s*-/, "Kepler's equation written out"],
    [/Math\.sin\s*\(\s*E\s*\)/, "sin(E) in a Kepler iteration"],
  ];
  const hits = patterns.filter(([re]) => re.test(src)).map(([, why]) => why);
  if (hits.length) throw new Error("page still solves the orbit itself: " + hits.join("; "));
});

check("no local gravitational parameter", (src) => {
  // The page must read `mu` from the kernel (`orbit_mu()`), not carry its own copy that can drift from
  // the one the tests check. 398600.4418 is the giveaway.
  if (/398600/.test(src)) {
    throw new Error("page contains a hard-coded mu; it must call orbit_mu()");
  }
});

check("no circular-orbit speed written by hand", (src) => {
  // `Math.sqrt(mu / r)` was the old stub's velocity, and it is wrong by 15% at periapsis on this orbit.
  if (/Math\.sqrt\s*\(\s*(mu|MU)\s*\/\s*r\s*\)/.test(src)) {
    throw new Error("page computes a circular-orbit speed instead of asking the kernel for velocity");
  }
});

check("module is instantiated with no imports", (src) => {
  const m = src.match(/WebAssembly\.instantiate\(([^)]*)\)/);
  if (!m) throw new Error("page never instantiates the wasm module");
  if (!/\{\s*\}/.test(m[1])) {
    throw new Error(
      "instantiation passes an import object; the module is import-free by design and adding " +
        "imports would break tools/determinism.mjs, which instantiates with {}"
    );
  }
});

check("the page requires every state and velocity export", (src) => {
  const needed = [
    "orbit_state_x",
    "orbit_state_y",
    "orbit_state_z",
    "orbit_velocity_x",
    "orbit_velocity_y",
    "orbit_velocity_z",
  ];
  const missing = needed.filter((n) => !src.includes(n));
  if (missing.length) {
    throw new Error("page does not reference: " + missing.join(", "));
  }
  // Naming them in a required-exports array is what makes a partial build fail loudly at load time
  // rather than at the first draw, where the symptom would be a stale orbit rather than an error.
  if (!/needed\s*=\s*\[/.test(src) && !/required\s*=\s*\[/.test(src)) {
    throw new Error("page does not check the module's export list before drawing");
  }
});

check("a missing kernel hides the body rather than substituting an orbit", (src) => {
  // The FIRST version of this check demanded that `drawFrame` bail out when the kernel was missing.
  // That was wrong, and running the preview showed why: the instrument would stop rendering entirely on
  // every machine without a built artifact -- including the sandboxed preview, which has no network --
  // and the scene shading and R2 measurement have nothing to do with the orbit. So the contract is:
  //
  //   * the body is placed at a position that is out of frame, and
  //   * the placement is a NAMED constant, not a computation, and
  //   * the HUD says the orbit source is unavailable.
  //
  // What stays forbidden is the thing that actually lied: arithmetic that produces a plausible orbit
  // when no kernel is present.
  if (!/orbitPosition\s*\(\s*tSeconds\s*\)\s*\{[\s\S]{0,260}?if \(!k\) return NO_ORBIT_POSITION;/.test(src)) {
    throw new Error("orbitPosition does not return the named out-of-frame position when the kernel is missing");
  }
  if (!/const NO_ORBIT_POSITION = \[/.test(src)) {
    throw new Error("no named NO_ORBIT_POSITION constant: the absent-kernel path must be one obvious value");
  }
  // It must be out of frame. The camera is at y=1.6 looking along +z with a narrow cone, so a body a
  // long way below the ground plane is not visible; a body near the origin would be.
  const m = src.match(/const NO_ORBIT_POSITION = \[([^\]]+)\]/);
  const vals = m[1].split(",").map((v) => Number(v.trim()));
  if (vals.length !== 3 || vals.some((v) => !Number.isFinite(v))) {
    throw new Error("NO_ORBIT_POSITION is not three finite numbers");
  }
  if (Math.abs(vals[1]) < 100) {
    throw new Error(
      `NO_ORBIT_POSITION y=${vals[1]} is near the scene; a hidden body must be far outside it or it ` +
        "will be visible and read as a computed position"
    );
  }
  // And the absence must be reported, not silently rendered.
  if (!/m-orbit/.test(src)) {
    throw new Error("the HUD does not report the orbit source");
  }
});

check("the module is fetched from the served build directory", (src) => {
  // `/dist/` is what `tools/static-path.mjs` maps to the build output, and what `npm run fetch:wasm`
  // fills. Fetching from the page's own directory silently 404s, and the symptom is a missing body with
  // no error visible anywhere.
  if (!/\/dist\/odderon_dmath_wasm\.wasm/.test(src)) {
    throw new Error("page does not fetch /dist/odderon_dmath_wasm.wasm");
  }
});

/**
 * The physics a set of exports must satisfy, as a function of the exports alone.
 *
 * Separated from the file loading because the wasm build only exists in CI: if the verification lived
 * inside `runWasm`, none of it could be executed in this sandbox, and "checked in CI" would be the whole
 * of its testing. `--selftest` drives this with a deliberate shim so both the accept and reject paths
 * run here.
 */
function verifyExports(ex) {
  const need = [
    "orbit_state_x",
    "orbit_state_y",
    "orbit_state_z",
    "orbit_velocity_x",
    "orbit_velocity_y",
    "orbit_velocity_z",
    "orbit_iterations",
    "orbit_unconverged",
    "orbit_mu",
    "orbit_semi_major_axis",
    "orbit_eccentricity",
  ];
  const missing = need.filter((n) => typeof ex[n] !== "function");
  if (missing.length) throw new Error("module is missing: " + missing.join(", "));

  const mu = ex.orbit_mu();
  const a = ex.orbit_semi_major_axis();
  const e = ex.orbit_eccentricity();
  const r0 = Math.hypot(ex.orbit_state_x(0), ex.orbit_state_y(0), ex.orbit_state_z(0));
  const v0 = Math.hypot(ex.orbit_velocity_x(0), ex.orbit_velocity_y(0), ex.orbit_velocity_z(0));

  // The three independent facts from the native test, re-derived here so that a broken *boundary*
  // (right maths, wrong argument order, a lost sign) is caught as well as broken maths.
  const periapsis = (a * (1 - e * e)) / (1 + e);
  const visViva = Math.sqrt(mu * (2 / periapsis - 1 / a));
  const period = 2 * Math.PI * Math.sqrt((a * a * a) / mu);

  const problems = [];
  // Convergence first: every number below is meaningless if the solve that produced it gave up.
  for (const t of [0, 1, 600, period]) {
    if (ex.orbit_unconverged(t) !== 0) problems.push(`solve did not converge at dt=${t}`);
  }
  if (Math.abs(mu - 398600.4418) > 1e-6) problems.push(`mu = ${mu}`);
  if (Math.abs(a - 9800) > 1e-9) problems.push(`a = ${a}`);
  if (Math.abs(e - 0.3) > 1e-12) problems.push(`e = ${e}`);
  if (Math.abs(r0 - periapsis) > 1e-6) problems.push(`r(0) = ${r0}, want periapsis ${periapsis}`);
  if (Math.abs(v0 - visViva) > 1e-6) {
    problems.push(
      `v(0) = ${v0}, want vis-viva ${visViva} (a circular sqrt(mu/r) would be ${Math.sqrt(mu / r0)})`,
    );
  }

  // A full period must return to the start. This is the property the renderer needs, and it is the one
  // that a position-only boundary cannot express at all.
  const rT = Math.hypot(
    ex.orbit_state_x(period),
    ex.orbit_state_y(period),
    ex.orbit_state_z(period),
  );
  if (Math.abs(rT - r0) > 1e-3) problems.push(`r(period) = ${rT}, want ${r0}`);

  if (problems.length) throw new Error("boundary disagrees with physics: " + problems.join("; "));
  return `mu=${mu} a=${a} e=${e} r(0)=${r0.toFixed(3)} km v(0)=${v0.toFixed(6)} km/s`;
}

/** Load a built module and verify it. */
async function runWasm(path) {
  const bytes = await readFile(path);
  const { instance } = await WebAssembly.instantiate(bytes, {});
  return verifyExports(instance.exports);
}

/**
 * `--selftest`: prove `verifyExports` accepts a correct boundary and rejects the two ways it can be
 * wrong. Without this, the accept branch and both reject branches would first run in CI, and a checker
 * whose rejection path has never executed is not yet known to reject anything.
 */
function selftest() {
  const mu = 398600.4418;
  const a = 9800.0;
  const e = 0.3;
  const r0 = (a * (1 - e * e)) / (1 + e);
  const v0 = Math.sqrt(mu * (2 / r0 - 1 / a));
  // A shim, not a propagator: every time returns periapsis. That is enough to satisfy the three things
  // `verifyExports` checks -- r(0), v(0), r(period) -- and it is *honest shim behaviour* rather than a
  // pretend orbit, which is the distinction that matters in a test fixture.
  const good = {
    orbit_mu: () => mu,
    orbit_semi_major_axis: () => a,
    orbit_eccentricity: () => e,
    orbit_state_x: () => r0,
    orbit_state_y: () => 0,
    orbit_state_z: () => 0,
    orbit_velocity_x: () => 0,
    orbit_velocity_y: () => v0,
    orbit_velocity_z: () => 0,
    orbit_iterations: () => 4,
    orbit_unconverged: () => 0,
  };

  const results = [];
  try {
    verifyExports(good);
    results.push(["a correct boundary is accepted", true]);
  } catch (err) {
    results.push([`a correct boundary is accepted (${err.message})`, false]);
  }

  // The exact defect this whole change removes: a circular-orbit speed standing in for the real one.
  const circular = { ...good, orbit_velocity_y: () => Math.sqrt(mu / r0) };
  try {
    verifyExports(circular);
    results.push(["a circular-orbit speed is rejected", false]);
  } catch {
    results.push(["a circular-orbit speed is rejected", true]);
  }

  // The old stub's actual shape: a position and no velocity at all. `verifyExports` must fail on the
  // missing export rather than on a quieter signal.
  const positionOnly = { ...good };
  delete positionOnly.orbit_velocity_x;
  delete positionOnly.orbit_velocity_y;
  delete positionOnly.orbit_velocity_z;
  try {
    verifyExports(positionOnly);
    results.push(["a position-only boundary is rejected", false]);
  } catch {
    results.push(["a position-only boundary is rejected", true]);
  }

  // A solve that did not converge is not a position. The kernel reports it; the checker must refuse it.
  try {
    verifyExports({ ...good, orbit_unconverged: () => 1 });
    results.push(["an unconverged solve is rejected", false]);
  } catch {
    results.push(["an unconverged solve is rejected", true]);
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
  console.log(`${failed} orbit check(s) FAILED`);
  process.exit(1);
}
console.log(`all ${checks.length} orbit checks passed`);
