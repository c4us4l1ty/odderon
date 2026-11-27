#!/usr/bin/env node
/**
 * The gate that makes the orbital rendezvous a real kernel prediction rather than a page-drawn
 * transfer plan.
 *
 * `web/index.html` answers "what does it cost to meet two circular orbits, and when can the
 * transfer begin?" by calling the kernel's `rendezvous_*` exports -- the seam over
 * `odderon_astro::rendezvous`, which computes the Hohmann transfer Δv, the transfer time, the
 * phase angle, and the rendezvous window. The same "no substitute" contract the orbit, the rocket,
 * the pad, the pass and the comms follow: the page computes no transfer Δv, holds no Hohmann
 * constant, and shows no window when the kernel is absent.
 *
 * The checks:
 *
 *   1. NO LOCAL TRANSFER MATH. The page may read the kernel's rendezvous readings but must not
 *      compute a transfer itself: no hard-coded Hohmann formula, no local `sqrt(mu/r)`, no local
 *      phase-angle computation. The failure mode is "someone re-adds a local transfer plan", which
 *      no runtime probe can see.
 *
 *   2. THE PAGE REQUIRES EVERY RENDEZVOUS EXPORT. The page must reference all five kernel
 *      accessors: `rendezvous_transfer_dv`, `rendezvous_transfer_time_s`, `rendezvous_phase_angle`,
 *      `rendezvous_window_s`, and `rendezvous_design_hash`. A partial build fails at load.
 *
 *   3. NO SILENT FALLBACK. `rendezvousReadings` must return a named source when the kernel is
 *      absent, and the panel must show `—` rather than a fabricated transfer plan.
 *
 *   4. THE RENDEZVOUS READINGS COME FROM THE KERNEL. `rendezvousReadings` must call the four
 *      `rendezvous_*` accessors, not compute a transfer from a local formula.
 *
 *   5. THE WASM BOUNDARY IS PHYSICAL. When a build is available, the exports must reproduce the
 *      invariants of the Hohmann transfer: the LEO-to-GEO Δv matches the textbook value; the
 *      transfer time is half the ellipse period; the phase angle is a pure function of the radius
 *      ratio (independent of `mu`); the window at the required lead is zero; a same-orbit transfer
 *      is refused with NaN for time and phase, and zero Δv; NaN inputs are refused.
 *
 * Usage: node tools/check-rendezvous.mjs            (static: no browser, no wasm needed)
 *        node tools/check-rendezvous.mjs --selftest  (verify the verifier accepts good exports)
 *        node tools/check-rendezvous.mjs --wasm dist/odderon_dmath_wasm.wasm
 *                                                     (also run the exports, if a build is available)
 */

import { readFile } from "node:fs/promises";

const PAGE = new URL("../web/index.html", import.meta.url);

/** Strip comments so documentation about rendezvous does not trip the checks that hunt for a solver. */
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

check("no local transfer math in the page", (src) => {
  // A local solver would carry the Hohmann formulas, a sqrt(mu/r) term, or a hand-rolled phase
  // angle. Each is the giveaway that the page is computing the transfer itself.
  const patterns = [
    [/rendezvous_transfer_dv\s*\([^)]*\)\s*=\s*[^;]*Math\./, "a hand-computed transfer Δv"],
    [/rendezvous_transfer_time\s*\([^)]*\)\s*=\s*[^;]*Math\./, "a hand-computed transfer time"],
    [/rendezvous_phase\s*\([^)]*\)\s*=\s*[^;]*Math\./, "a hand-computed phase angle"],
    [/rendezvous_window\s*\([^)]*\)\s*=\s*[^;]*Math\./, "a hand-computed window"],
    [/Math\.PI\s*\*\s*Math\.sqrt\s*\([^)]*mu[^)]*\)/i, "a local Hohmann sqrt(mu/r) term"],
  ];
  const hits = patterns.filter(([re]) => re.test(src)).map(([, why]) => why);
  if (hits.length) throw new Error("page still computes the rendezvous itself: " + hits.join("; "));
});

check("the page requires every rendezvous export", (src) => {
  const needed = [
    "rendezvous_transfer_dv",
    "rendezvous_transfer_time_s",
    "rendezvous_phase_angle",
    "rendezvous_window_s",
    "rendezvous_design_hash",
  ];
  const missing = needed.filter((n) => !src.includes(n));
  if (missing.length) {
    throw new Error("page does not reference: " + missing.join(", "));
  }
});

check("a missing kernel hides the rendezvous rather than substituting a plan", (src) => {
  // The contract: when the kernel is absent, `rendezvousReadings` returns a named `source` that is
  // not the kernel, and the panel shows `—` rather than a fabricated transfer plan.
  if (!/function rendezvousReadings\s*\([^)]*\)\s*\{[\s\S]{0,200}?if \(!k\) return \{ source: 'absent' \};/.test(src)) {
    throw new Error("rendezvousReadings does not return a named absent source when the kernel is missing");
  }
});

check("the rendezvous readings come from the kernel, not a local formula", (src) => {
  const m = src.match(/function rendezvousReadings\s*\([^)]*\)\s*\{([\s\S]*?)\n\}/);
  if (!m) throw new Error("rendezvousReadings not found");
  const body = m[1];
  for (const accessor of [
    "rendezvous_transfer_dv",
    "rendezvous_transfer_time_s",
    "rendezvous_phase_angle",
    "rendezvous_window_s",
  ]) {
    if (!body.includes(accessor)) {
      throw new Error(`rendezvousReadings does not call ${accessor}`);
    }
  }
});

/**
 * The physics the rendezvous exports must satisfy, as a function of the exports alone.
 *
 * Separated from the file loading because the wasm build only exists in CI: if the verification lived
 * inside `runWasm`, none of it could be executed in this sandbox, and "checked in CI" would be the
 * whole of its testing. `--selftest` drives this with a deliberate shim so both the accept and reject
 * paths run here.
 */
function verifyExports(ex) {
  const need = [
    "rendezvous_transfer_dv",
    "rendezvous_transfer_time_s",
    "rendezvous_phase_angle",
    "rendezvous_window_s",
    "rendezvous_design_hash",
    "orbit_mu",
  ];
  const missing = need.filter((n) => typeof ex[n] !== "function");
  if (missing.length) throw new Error("module is missing: " + missing.join(", "));

  const mu = ex.orbit_mu();
  const R_LEO = 6778.137;
  const R_GEO = 42164.0;
  const problems = [];

  // The LEO-to-GEO Hohmann transfer Δv. Reference: 3.853957 km/s (textbook formulas, independent).
  const dv = ex.rendezvous_transfer_dv(R_LEO, R_GEO, mu);
  if (!Number.isFinite(dv)) problems.push("transfer Δv is not finite");
  else if (Math.abs(dv - 3.853957) > 1e-4) problems.push(`LEO→GEO Δv = ${dv}, want 3.853957`);

  // Transfer time: half the ellipse period. Reference: 19048.5 s.
  const t = ex.rendezvous_transfer_time_s(R_LEO, R_GEO, mu);
  if (!Number.isFinite(t)) problems.push("transfer time is not finite");
  else if (Math.abs(t - 19048.5) > 1.0) problems.push(`transfer time = ${t}, want 19048.5`);

  // Phase angle: a pure function of the radius ratio. Reference: 1.752547 rad.
  const phi = ex.rendezvous_phase_angle(R_LEO, R_GEO, mu);
  if (!Number.isFinite(phi)) problems.push("phase angle is not finite");
  else if (Math.abs(phi - 1.752547) > 1e-4) problems.push(`phase angle = ${phi}, want 1.752547`);

  // Phase angle is independent of mu: doubling mu must not change it.
  const phi2 = ex.rendezvous_phase_angle(R_LEO, R_GEO, mu * 2);
  if (Math.abs(phi - phi2) > 1e-12) problems.push("phase angle depends on mu");

  // Window at the required lead is zero.
  const w0 = ex.rendezvous_window_s(R_LEO, R_GEO, phi, mu);
  if (!Number.isFinite(w0)) problems.push("window at required lead is not finite");
  else if (Math.abs(w0) > 1e-6) problems.push(`window at required lead = ${w0}, want 0`);

  // Window from zero lead: reference 4280.5 s.
  const w0lead = ex.rendezvous_window_s(R_LEO, R_GEO, 0.0, mu);
  if (!Number.isFinite(w0lead)) problems.push("window from zero lead is not finite");
  else if (Math.abs(w0lead - 4280.5) > 1.0) problems.push(`window from zero lead = ${w0lead}, want 4280.5`);

  // Same-orbit transfer: Δv = 0, time = NaN, phase = NaN.
  const dvSame = ex.rendezvous_transfer_dv(R_LEO, R_LEO, mu);
  if (dvSame !== 0.0) problems.push(`same-orbit Δv = ${dvSame}, want 0`);
  const tSame = ex.rendezvous_transfer_time_s(R_LEO, R_LEO, mu);
  if (Number.isFinite(tSame)) problems.push(`same-orbit transfer time = ${tSame}, want NaN`);
  const phiSame = ex.rendezvous_phase_angle(R_LEO, R_LEO, mu);
  if (Number.isFinite(phiSame)) problems.push(`same-orbit phase angle = ${phiSame}, want NaN`);

  // NaN inputs are refused.
  const dvNaN = ex.rendezvous_transfer_dv(NaN, R_GEO, mu);
  if (Number.isFinite(dvNaN)) problems.push(`NaN radius returned ${dvNaN}, want NaN`);

  // Inward transfer has the same Δv as outward (burn magnitudes are symmetric).
  const dvIn = ex.rendezvous_transfer_dv(R_GEO, R_LEO, mu);
  if (Math.abs(dv - dvIn) > 1e-9) problems.push(`outward ${dv} != inward ${dvIn}`);

  // Inward phase angle is negative.
  const phiIn = ex.rendezvous_phase_angle(R_GEO, R_LEO, mu);
  if (phiIn >= 0) problems.push(`inward phase angle = ${phiIn}, want negative`);

  // Design hash is deterministic.
  const h1 = ex.rendezvous_design_hash();
  const h2 = ex.rendezvous_design_hash();
  if (h1 !== h2) problems.push("design hash is not deterministic");
  if (h1 === 0) problems.push("design hash is zero");

  if (problems.length) throw new Error("boundary disagrees with physics: " + problems.join("; "));
  return `dv=${dv.toFixed(6)}km/s t=${t.toFixed(1)}s phi=${phi.toFixed(6)}rad window=${w0lead.toFixed(1)}s`;
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
  const MU = 398600.4418;
  const R_LEO = 6778.137;
  const R_GEO = 42164.0;

  // A shim that returns the textbook Hohmann values. Honest shim behaviour: the formulas are
  // implemented here in the fixture, which is the point -- the verifier checks the SHAPE of the
  // contract, not a particular implementation's internals.
  const good = {
    orbit_mu: () => MU,
    rendezvous_transfer_dv: (r1, r2, mu) => {
      if (!Number.isFinite(r1) || !Number.isFinite(r2) || r1 <= 0 || r2 <= 0) return NaN;
      if (r1 === r2) return 0;
      const rs = r1 + r2;
      return Math.sqrt(mu / r1) * Math.abs(Math.sqrt(2 * r2 / rs) - 1) +
        Math.sqrt(mu / r2) * Math.abs(Math.sqrt(2 * r1 / rs) - 1);
    },
    rendezvous_transfer_time_s: (r1, r2, mu) => {
      if (!Number.isFinite(r1) || !Number.isFinite(r2) || r1 <= 0 || r2 <= 0 || r1 === r2) return NaN;
      const rs = r1 + r2;
      return Math.PI * Math.sqrt(rs * rs * rs / (8 * mu));
    },
    rendezvous_phase_angle: (r1, r2, mu) => {
      if (!Number.isFinite(r1) || !Number.isFinite(r2) || r1 <= 0 || r2 <= 0 || r1 === r2) return NaN;
      const ratio = r1 / r2;
      return Math.PI * (1 - Math.sqrt(Math.pow(1 + ratio, 3) / 8));
    },
    rendezvous_window_s: (r1, r2, theta, mu) => {
      if (!Number.isFinite(r1) || !Number.isFinite(r2) || !Number.isFinite(theta) || r1 <= 0 || r2 <= 0 || r1 === r2) return NaN;
      const dphi = Math.PI * (1 - Math.sqrt(Math.pow(1 + r1 / r2, 3) / 8));
      const req = dphi >= 0 ? dphi : dphi + 2 * Math.PI;
      const n1 = Math.sqrt(mu / Math.pow(r1, 3));
      const n2 = Math.sqrt(mu / Math.pow(r2, 3));
      const omega = n2 - n1;
      if (Math.abs(theta - req) < 1e-4) return 0;
      if (omega > 0) {
        return theta < req ? (req - theta) / omega : (2 * Math.PI - theta + req) / omega;
      }
      const aw = -omega;
      return theta > req ? (theta - req) / aw : (theta + 2 * Math.PI - req) / aw;
    },
    rendezvous_design_hash: () => 0xcbf29ce484222325,
  };

  const results = [];
  try {
    verifyExports(good);
    results.push(["a correct boundary is accepted", true]);
  } catch (err) {
    results.push([`a correct boundary is accepted (${err.message})`, false]);
  }

  // The exact defect this check removes: a transfer Δv that does not match the textbook value.
  const wrongDv = { ...good, rendezvous_transfer_dv: () => 999.0 };
  try {
    verifyExports(wrongDv);
    results.push(["a wrong transfer Δv is rejected", false]);
  } catch {
    results.push(["a wrong transfer Δv is rejected", true]);
  }

  // A phase angle that depends on mu is not a pure function of the radius ratio.
  const wrongPhase = {
    ...good,
    rendezvous_phase_angle: (r1, r2, mu) => Math.PI * (1 - Math.sqrt(Math.pow(1 + r1 / r2, 3) / 8)) * (mu / MU),
  };
  try {
    verifyExports(wrongPhase);
    results.push(["a phase angle that depends on mu is rejected", false]);
  } catch {
    results.push(["a phase angle that depends on mu is rejected", true]);
  }

  // A window at the required lead that is not zero is not the kernel's prediction.
  const wrongWindow = { ...good, rendezvous_window_s: () => 999.0 };
  try {
    verifyExports(wrongWindow);
    results.push(["a non-zero window at the required lead is rejected", false]);
  } catch {
    results.push(["a non-zero window at the required lead is rejected", true]);
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
  console.log(`${failed} rendezvous check(s) FAILED`);
  process.exit(1);
}
console.log(`all ${checks.length} rendezvous checks passed`);
