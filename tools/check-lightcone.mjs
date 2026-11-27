#!/usr/bin/env node
/**
 * The gate that makes the deeper light-cone comms a real kernel prediction rather than a page-drawn
 * causal structure.
 *
 * `web/index.html` answers "what is the causal structure of the sky — the light-cone envelope, which
 * peers are reachable, the Doppler shift, and the signal attenuation?" by calling the kernel's
 * `lightcone_*` exports -- the seam over `odderon_astro::lightcone`, which computes the light-cone
 * radius, the reachability predicate, the relativistic Doppler factor, and the inverse-square
 * attenuation. The same "no substitute" contract the orbit, the rocket, the pad, the pass, the
 * rendezvous, the arc and the comms follow: the page computes no causal structure, holds no
 * speed-of-light or inverse-square constant, and shows no causal reading when the kernel is absent.
 *
 * The checks:
 *
 *   1. NO LOCAL CAUSAL MATH. The page may read the kernel's light-cone readings but must not compute
 *      a causal structure itself: no hard-coded speed of light, no local `c * dt`, no local Doppler
 *      formula, no local inverse-square. The failure mode is "someone re-adds a local causal model",
 *      which no runtime probe can see.
 *
 *   2. THE PAGE REQUIRES EVERY LIGHT-CONE EXPORT. The page must reference all five kernel accessors:
 *      `light_cone_radius_km`, `causal_reachable`, `doppler_factor`, `signal_power_ratio`, and
 *      `lightcone_design_hash`. A partial build fails at load.
 *
 *   3. NO SILENT FALLBACK. `lightconeReadings` must return a named source when the kernel is absent,
 *      and the panel must show `—` rather than a fabricated causal structure.
 *
 *   4. THE LIGHT-CONE READINGS COME FROM THE KERNEL. `lightconeReadings` must call the four
 *      `lightcone_*` accessors, not compute a causal structure from a local formula.
 *
 *   5. THE WASM BOUNDARY IS PHYSICAL. When a build is available, the exports must reproduce the
 *      invariants of the light-cone structure: the radius at 1 s is exactly the speed of light in km;
 *      the radius scales linearly with time; reachability is the radius comparison (inside, at, and
 *      beyond the boundary); the Doppler factor matches the three classic cases (approaching =
 *      blueshift, receding = redshift, transverse = relativistic redshift); the inverse-square law
 *      gives one at the reference, a quarter at twice the distance, and one-hundredth at ten times;
 *      and the design hash is deterministic and non-zero.
 *
 * Usage: node tools/check-lightcone.mjs            (static: no browser, no wasm needed)
 *        node tools/check-lightcone.mjs --selftest  (verify the verifier accepts good exports)
 *        node tools/check-lightcone.mjs --wasm dist/odderon_dmath_wasm.wasm
 *                                                    (also run the exports, if a build is available)
 */

import { readFile } from "node:fs/promises";

const PAGE = new URL("../web/index.html", import.meta.url);

/** Strip comments so documentation about the light cone does not trip the checks that hunt for a solver. */
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

check("no local causal math in the page", (src) => {
  // A local solver would carry the speed of light, a c*dt term, a Doppler formula, or an
  // inverse-square term. Each is the giveaway that the page is computing the structure itself.
  const patterns = [
    [/light_cone_radius\s*\([^)]*\)\s*=[^;]*Math\./, "a hand-computed light-cone radius"],
    [/causal_reachable\s*\([^)]*\)\s*=[^;]*Math\./, "a hand-computed reachability"],
    [/doppler_factor\s*\([^)]*\)\s*=[^;]*Math\./, "a hand-computed Doppler factor"],
    [/signal_power_ratio\s*\([^)]*\)\s*=[^;]*Math\./, "a hand-computed signal power"],
    [/299_?792/, "a hard-coded speed of light"],
    [/SPEED_OF_LIGHT/i, "a speed-of-light constant"],
    [/Math\.pow\s*\([^)]*2\s*\)/, "a local inverse-square"],
  ];
  const hits = patterns.filter(([re]) => re.test(src)).map(([, why]) => why);
  if (hits.length) throw new Error("page still computes the causal structure itself: " + hits.join("; "));
});

check("the page requires every light-cone export", (src) => {
  const needed = [
    "light_cone_radius_km",
    "causal_reachable",
    "doppler_factor",
    "signal_power_ratio",
    "lightcone_design_hash",
  ];
  const missing = needed.filter((n) => !src.includes(n));
  if (missing.length) {
    throw new Error("page does not reference: " + missing.join(", "));
  }
});

check("a missing kernel hides the light cone rather than substituting a structure", (src) => {
  // The contract: when the kernel is absent, `lightconeReadings` returns a named `source` that is
  // not the kernel, and the panel shows `—` rather than a fabricated causal structure.
  if (!/function lightconeReadings\s*\([^)]*\)\s*\{[\s\S]{0,200}?if \(!k\) return \{ source: 'absent' \};/.test(src)) {
    throw new Error("lightconeReadings does not return a named absent source when the kernel is missing");
  }
});

check("the light-cone readings come from the kernel, not a local formula", (src) => {
  const m = src.match(/function lightconeReadings\s*\([^)]*\)\s*\{([\s\S]*?)\n\}/);
  if (!m) throw new Error("lightconeReadings not found");
  const body = m[1];
  for (const accessor of ["light_cone_radius_km", "causal_reachable", "doppler_factor", "signal_power_ratio"]) {
    if (!body.includes(accessor)) {
      throw new Error(`lightconeReadings does not call ${accessor}`);
    }
  }
});

/**
 * The physics the light-cone exports must satisfy, as a function of the exports alone.
 *
 * Separated from the file loading because the wasm build only exists in CI: if the verification lived
 * inside `runWasm`, none of it could be executed in this sandbox, and "checked in CI" would be the
 * whole of its testing. `--selftest` drives this with a deliberate shim so both the accept and reject
 * paths run here.
 */
function verifyExports(ex) {
  const need = [
    "light_cone_radius_km",
    "causal_reachable",
    "doppler_factor",
    "signal_power_ratio",
    "lightcone_design_hash",
  ];
  const missing = need.filter((n) => typeof ex[n] !== "function");
  if (missing.length) throw new Error("module is missing: " + missing.join(", "));

  const C = 299792.458;
  const problems = [];

  // The radius at 1 s is exactly the speed of light in km.
  const r1 = ex.light_cone_radius_km(1.0);
  if (!Number.isFinite(r1)) problems.push("radius at 1 s is not finite");
  else if (Math.abs(r1 - C) > 1e-6) problems.push(`radius at 1 s = ${r1}, want ${C}`);

  // The radius scales linearly with time.
  const r5 = ex.light_cone_radius_km(5.0);
  if (Number.isFinite(r1) && Number.isFinite(r5)) {
    if (Math.abs(r5 - 5.0 * r1) > 1e-6) problems.push(`radius not linear: ${r5} vs ${5.0 * r1}`);
  }

  // Reachability: inside, at, and beyond the boundary.
  if (ex.causal_reachable(0.5 * C, 1.0) !== 1) problems.push("a position inside the cone is not reachable");
  if (ex.causal_reachable(C, 1.0) !== 1) problems.push("a position at the cone boundary is not reachable");
  if (ex.causal_reachable(2.0 * C, 1.0) !== 0) problems.push("a position beyond the cone is reachable");
  if (ex.causal_reachable(1.0, 0.0) !== 0) problems.push("a position is reachable at zero time");

  // Doppler: approaching is a blueshift (factor > 1).
  const beta = 0.01;
  const v = beta * C;
  const fApproach = ex.doppler_factor(v, 0.0);
  const fApproachExpected = Math.sqrt((1.0 + beta) / (1.0 - beta));
  if (!Number.isFinite(fApproach)) problems.push("approaching Doppler is not finite");
  else if (Math.abs(fApproach - fApproachExpected) > 1e-12) problems.push(`approaching Doppler = ${fApproach}, want ${fApproachExpected}`);
  else if (fApproach <= 1.0) problems.push("approaching is not a blueshift");

  // Doppler: receding is a redshift (factor < 1).
  const fRecede = ex.doppler_factor(v, Math.PI);
  const fRecedeExpected = Math.sqrt((1.0 - beta) / (1.0 + beta));
  if (!Number.isFinite(fRecede)) problems.push("receding Doppler is not finite");
  else if (Math.abs(fRecede - fRecedeExpected) > 1e-12) problems.push(`receding Doppler = ${fRecede}, want ${fRecedeExpected}`);
  else if (fRecede >= 1.0) problems.push("receding is not a redshift");

  // Doppler: transverse is the relativistic redshift sqrt(1 - beta^2).
  const fTrans = ex.doppler_factor(v, Math.PI / 2.0);
  const fTransExpected = Math.sqrt(1.0 - beta * beta);
  if (!Number.isFinite(fTrans)) problems.push("transverse Doppler is not finite");
  else if (Math.abs(fTrans - fTransExpected) > 1e-12) problems.push(`transverse Doppler = ${fTrans}, want ${fTransExpected}`);

  // Doppler: zero velocity is no shift.
  const fZero = ex.doppler_factor(0.0, 0.5);
  if (!Number.isFinite(fZero)) problems.push("zero-velocity Doppler is not finite");
  else if (Math.abs(fZero - 1.0) > 1e-12) problems.push(`zero-velocity Doppler = ${fZero}, want 1.0`);

  // Doppler: a velocity at c is refused.
  if (Number.isFinite(ex.doppler_factor(C, 0.0))) problems.push("velocity at c is not refused");

  // Inverse-square: one at the reference, a quarter at twice, one-hundredth at ten times.
  const p1 = ex.signal_power_ratio(100.0, 100.0);
  const p2 = ex.signal_power_ratio(200.0, 100.0);
  const p10 = ex.signal_power_ratio(1000.0, 100.0);
  if (!Number.isFinite(p1) || Math.abs(p1 - 1.0) > 1e-12) problems.push(`power at reference = ${p1}, want 1.0`);
  if (!Number.isFinite(p2) || Math.abs(p2 - 0.25) > 1e-12) problems.push(`power at 2x = ${p2}, want 0.25`);
  if (!Number.isFinite(p10) || Math.abs(p10 - 0.01) > 1e-12) problems.push(`power at 10x = ${p10}, want 0.01`);

  // Inverse-square: zero distance is refused.
  if (Number.isFinite(ex.signal_power_ratio(0.0, 100.0))) problems.push("zero distance is not refused");

  // Design hash is deterministic and non-zero.
  const h1 = ex.lightcone_design_hash();
  const h2 = ex.lightcone_design_hash();
  if (h1 !== h2) problems.push("design hash is not deterministic");
  if (h1 === 0) problems.push("design hash is zero");

  if (problems.length) throw new Error("boundary disagrees with the light-cone structure: " + problems.join("; "));
  return `radius=${r1.toFixed(1)}km reach=${ex.causal_reachable(C, 1.0)} doppler=${fApproach.toFixed(6)} power=${p2.toFixed(4)}`;
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
  const C = 299792.458;

  // A shim that implements the light-cone physics correctly. The formulas are implemented here in the
  // fixture, which is the point -- the verifier checks the SHAPE of the contract, not a particular
  // implementation's internals.
  const good = {
    light_cone_radius_km: (dt) => (dt > 0 && Number.isFinite(dt) ? C * dt : NaN),
    causal_reachable: (r, dt) => (Number.isFinite(r) && Number.isFinite(dt) && dt > 0 ? (r <= C * dt ? 1 : 0) : 0),
    doppler_factor: (v, theta) => {
      if (!Number.isFinite(v) || !Number.isFinite(theta)) return NaN;
      const beta = v / C;
      if (beta >= 1) return NaN;
      return Math.sqrt(1 - beta * beta) / (1 - beta * Math.cos(theta));
    },
    signal_power_ratio: (d, dr) => (d > 0 && dr > 0 && Number.isFinite(d) && Number.isFinite(dr) ? Math.pow(dr / d, 2) : NaN),
    lightcone_design_hash: () => 0xcbf29ce484222325,
  };

  const results = [];
  try {
    verifyExports(good);
    results.push(["a correct boundary is accepted", true]);
  } catch (err) {
    results.push([`a correct boundary is accepted (${err.message})`, false]);
  }

  // The exact defect this check removes: a light-cone radius that is not c * dt.
  const wrongRadius = { ...good, light_cone_radius_km: () => 999.0 };
  try {
    verifyExports(wrongRadius);
    results.push(["a wrong light-cone radius is rejected", false]);
  } catch {
    results.push(["a wrong light-cone radius is rejected", true]);
  }

  // A reachability that says everything is reachable is not the kernel's prediction.
  const wrongReach = { ...good, causal_reachable: () => 1 };
  try {
    verifyExports(wrongReach);
    results.push(["a reachability that accepts everything is rejected", false]);
  } catch {
    results.push(["a reachability that accepts everything is rejected", true]);
  }

  // A Doppler factor that is not a blueshift for an approaching source is not the kernel's.
  const wrongDoppler = { ...good, doppler_factor: () => 0.5 };
  try {
    verifyExports(wrongDoppler);
    results.push(["a Doppler factor that is not physical is rejected", false]);
  } catch {
    results.push(["a Doppler factor that is not physical is rejected", true]);
  }

  // An inverse-square that is not 1/d^2 is not the kernel's prediction.
  const wrongPower = { ...good, signal_power_ratio: () => 1.0 };
  try {
    verifyExports(wrongPower);
    results.push(["an inverse-square that is not 1/d^2 is rejected", false]);
  } catch {
    results.push(["an inverse-square that is not 1/d^2 is rejected", true]);
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
  console.log(`${failed} light-cone check(s) FAILED`);
  process.exit(1);
}
console.log(`all ${checks.length} light-cone checks passed`);
