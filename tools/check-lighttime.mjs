#!/usr/bin/env node
/**
 * The gate that makes the light-time comms a real kernel prediction rather than a page-drawn delay.
 *
 * `web/index.html` answers "when does a message sent to the orbiting body land?" by calling the
 * kernel's `comms_*` and `light_time_s` exports -- the seam over `odderon_astro::lighttime`, which
 * divides a distance by the one defined speed of light. The same "no substitute" contract the orbit
 * body, the rocket, the pad and the pass follow: the page computes no light-time, holds no
 * speed-of-light constant, and shows no delay when the kernel is absent.
 *
 * The checks:
 *
 *   1. NO LOCAL LIGHT-TIME. The page may read the kernel's light-time but must not compute one: no
 *      hard-coded speed of light, no local distance/c, no local `Math.sqrt` distance between two
 *      positions. The failure mode is "someone re-adds a local delay", which no runtime probe can see.
 *
 *   2. THE PAGE REQUIRES EVERY COMMS EXPORT. The `needed` array must name all five: the primitive
 *      `light_time_s`, the gate `comms_link_status`, and the three readings
 *      `station_light_time_s`/`comms_round_trip_s`/`comms_arrival_s`. A partial build fails at load.
 *
 *   3. NO SILENT FALLBACK. `commsReadings` must return a named source when the kernel is absent, and
 *      the body flash must be gated on a sent message whose arrival the kernel predicted -- never a
 *      constant delay or a page-computed arrival.
 *
 *   4. THE COMMS READINGS COME FROM THE KERNEL. `commsReadings` must call the five `comms_*`/`light_time_s`
 *      accessors, not compute a delay from a local formula.
 *
 *   5. THE WASM BOUNDARY IS PHYSICAL. When a build is available, the exports must reproduce the
 *      invariants of light-time: a one-light-second distance gives exactly 1.0 s; coincident positions
 *      are refused with NaN; a station with a computable link gives status 1 with a finite one-way
 *      delay, round-trip = 2 x one-way, arrival = send + one-way, all reproducible; a refused site is
 *      status 2. The primitive `light_time_s` is checked against the defined constant directly.
 *
 * Usage: node tools/check-lighttime.mjs            (static: no browser, no wasm needed)
 *        node tools/check-lighttime.mjs --wasm dist/odderon_dmath_wasm.wasm
 *                                             (also run the exports, if a build is available)
 */

import { readFile } from "node:fs/promises";

const PAGE = new URL("../web/index.html", import.meta.url);

/** Strip comments so documentation about light-time does not trip the checks that hunt for a solver. */
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

/** The defined speed of light, km/s. The kernel's single copy of it. */
const SPEED_OF_LIGHT_KM_S = 299792.458;

check("no local light-time in the page", (src) => {
  // A local solver would carry the speed of light, a distance/c, or a hand-rolled distance between two
  // positions. Each is the giveaway that the page is computing the delay itself.
  const patterns = [
    [/299792/, "a hard-coded speed of light"],
    [/299_792/, "a hard-coded speed of light"],
    [/SPEED_OF_LIGHT/i, "a speed-of-light constant"],
    [/Math\.sqrt\s*\([^)]*\)\s*\/\s*[0-9]/, "a local distance/c"],
    [/lightTime\s*=\s*[^;]*Math\./, "a hand-computed light-time"],
    [/arrival\s*=\s*[^;]*\+\s*[^;]*Math\./, "a hand-computed arrival"],
  ];
  const hits = patterns.filter(([re]) => re.test(src)).map(([, why]) => why);
  if (hits.length) throw new Error("page still computes the light-time itself: " + hits.join("; "));
});

check("the page requires every comms export", (src) => {
  const needed = [
    "light_time_s",
    "comms_link_status",
    "station_light_time_s",
    "comms_round_trip_s",
    "comms_arrival_s",
  ];
  const missing = needed.filter((n) => !src.includes(n));
  if (missing.length) {
    throw new Error("page does not reference: " + missing.join(", "));
  }
  if (!/needed\s*=\s*\[/.test(src) && !/required\s*=\s*\[/.test(src)) {
    throw new Error("page does not check the module's export list before drawing");
  }
});

check("a missing kernel hides the comms rather than substituting a delay", (src) => {
  // The contract: when the kernel is absent, `commsReadings` returns a named `source` that is not the
  // kernel, and the body flash is gated on a sent message with a kernel-predicted arrival -- never a
  // constant delay or a page-computed arrival.
  if (!/function commsReadings\s*\([^)]*\)\s*\{[\s\S]{0,200}?if \(!k\) return \{ source: 'absent' \};/.test(src)) {
    throw new Error("commsReadings does not return a named absent source when the kernel is missing");
  }
  // The body flash is a function of a sent message whose arrival the kernel predicted.
  if (!/const bodyReceive = \(commsState\.sent && commsState\.arrived\)/.test(src)) {
    throw new Error("the body flash is not gated on a sent, kernel-arrived message");
  }
});

check("the comms readings come from the kernel, not a local formula", (src) => {
  const m = src.match(/function commsReadings\s*\([^)]*\)\s*\{([\s\S]*?)\n\}/);
  if (!m) throw new Error("commsReadings not found");
  const body = m[1];
  for (const accessor of [
    "comms_link_status",
    "station_light_time_s",
    "comms_round_trip_s",
    "comms_arrival_s",
  ]) {
    if (!body.includes(accessor)) {
      throw new Error(`commsReadings does not call ${accessor}`);
    }
  }
});

/**
 * The physics the comms exports must satisfy, as a function of the exports alone.
 *
 * Separated from the file loading because the wasm build only exists in CI: if the verification lived
 * inside `runWasm`, none of it could be executed in this sandbox, and "checked in CI" would be the
 * whole of its testing. `--selftest` drives this with a deliberate shim so both the accept and reject
 * paths run here.
 */
function verifyExports(ex) {
  const need = [
    "light_time_s",
    "comms_link_status",
    "station_light_time_s",
    "comms_round_trip_s",
    "comms_arrival_s",
  ];
  const missing = need.filter((n) => typeof ex[n] !== "function");
  if (missing.length) throw new Error("module is missing: " + missing.join(", "));

  const problems = [];

  // The primitive: a one-light-second distance is exactly 1.0 s by definition of the constant.
  const oneLightSecond = ex.light_time_s(0, 0, 0, SPEED_OF_LIGHT_KM_S, 0, 0);
  if (!Number.isFinite(oneLightSecond)) problems.push("one light-second did not return a finite time");
  else if (Math.abs(oneLightSecond - 1.0) > 1e-9) problems.push(`one light-second = ${oneLightSecond}, want 1.0`);

  // Coincident positions are refused, not a zero-second delay.
  const coincident = ex.light_time_s(1000, 2000, 3000, 1000, 2000, 3000);
  if (Number.isFinite(coincident)) problems.push(`coincident positions returned ${coincident}, want NaN`);

  // The composed link: a station inside the orbit's reach gives status 1 and finite readings.
  const lat = 0.0, lon = 0.0, from = 0.0;
  const status = ex.comms_link_status(lat, lon, from);
  if (status !== 1) problems.push(`a computable link verdict = ${status}, want 1`);

  const oneWay = ex.station_light_time_s(lat, lon, from);
  const round = ex.comms_round_trip_s(lat, lon, from);
  const arrival = ex.comms_arrival_s(lat, lon, from);
  if (!Number.isFinite(oneWay)) problems.push("one-way light-time is not finite");
  if (!Number.isFinite(round)) problems.push("round-trip is not finite");
  if (!Number.isFinite(arrival)) problems.push("arrival is not finite");

  if (Number.isFinite(oneWay) && Number.isFinite(round)) {
    if (Math.abs(round - 2.0 * oneWay) > 1e-9) problems.push(`round-trip ${round} != 2 x one-way ${oneWay}`);
  }
  if (Number.isFinite(oneWay) && Number.isFinite(arrival)) {
    if (Math.abs(arrival - (from + oneWay)) > 1e-9) problems.push(`arrival ${arrival} != send + one-way ${from + oneWay}`);
  }

  // Reproducibility: the same station and time give the same delay. The P3 replay claim.
  const oneWay2 = ex.station_light_time_s(lat, lon, from);
  if (oneWay !== oneWay2) problems.push(`one-way not reproducible: ${oneWay} vs ${oneWay2}`);

  // A refused site is status 2, never a fabricated delay.
  const refused = ex.comms_link_status(NaN, 0.0, 0.0);
  if (refused !== 2) problems.push(`a refused site verdict = ${refused}, want 2`);

  if (problems.length) throw new Error("boundary disagrees with physics: " + problems.join("; "));
  return `oneWay=${oneWay.toFixed(6)}s round=${round.toFixed(6)}s arrival=${arrival.toFixed(6)}s`;
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
  // A shim, not a propagator: it returns a plausible light-time for a valid station and an honest
  // refusal for a bad one. That is enough to satisfy `verifyExports`, and it is honest shim behaviour
  // rather than a pretend prediction -- the distinction that matters in a test fixture.
  const oneLight = SPEED_OF_LIGHT_KM_S / 299792.458; // 1.0 by construction
  const good = {
    light_time_s: (fx, fy, fz, tx, ty, tz) => {
      const dx = tx - fx, dy = ty - fy, dz = tz - fz;
      const d = Math.sqrt(dx * dx + dy * dy + dz * dz);
      return d === 0 ? NaN : d / SPEED_OF_LIGHT_KM_S;
    },
    comms_link_status: (lat, lon, from) => (Number.isFinite(lat) ? 1 : 2),
    station_light_time_s: () => 0.044,
    comms_round_trip_s: () => 0.088,
    comms_arrival_s: () => 0.044,
  };
  void oneLight;

  const results = [];
  try {
    verifyExports(good);
    results.push(["a correct boundary is accepted", true]);
  } catch (err) {
    results.push([`a correct boundary is accepted (${err.message})`, false]);
  }

  // The exact defect this check removes: a fabricated delay for a site the kernel refuses.
  const fabricatesEverywhere = { ...good, comms_link_status: () => 1 };
  try {
    verifyExports(fabricatesEverywhere);
    results.push(["a delay fabricated for a refused site is rejected", false]);
  } catch {
    results.push(["a delay fabricated for a refused site is rejected", true]);
  }

  // A round-trip that is not twice the one-way is not a light-time.
  const wrongRoundTrip = { ...good, comms_round_trip_s: () => 0.1 };
  try {
    verifyExports(wrongRoundTrip);
    results.push(["a round-trip not equal to 2 x one-way is rejected", false]);
  } catch {
    results.push(["a round-trip not equal to 2 x one-way is rejected", true]);
  }

  // An arrival that is not send + one-way is not the kernel's prediction.
  const wrongArrival = { ...good, comms_arrival_s: () => 999.0 };
  try {
    verifyExports(wrongArrival);
    results.push(["an arrival not equal to send + one-way is rejected", false]);
  } catch {
    results.push(["an arrival not equal to send + one-way is rejected", true]);
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
  console.log(`${failed} lighttime check(s) FAILED`);
  process.exit(1);
}
console.log(`all ${checks.length} lighttime checks passed`);
