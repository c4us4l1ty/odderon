#!/usr/bin/env node
/**
 * The gate that makes the museum a real, kernel-verifiable archive rather than a page-drawn list.
 *
 * `web/index.html` records the player's career -- every vehicle launched, every report exported, every
 * message sent -- into an append-only ledger, each entry carrying the kernel's own readings and a
 * content hash. The P4 gate is "someone else can verify your work": a third party, given an entry's
 * params, must be able to re-derive its readings from the kernel and recompute the hash, and both
 * must agree. This check is that claim made mechanical.
 *
 * The checks:
 *
 *   1. NO LOCAL ARCHIVE READINGS. The archive's readings are captured from the kernel (rocket verdict,
 *      pad design hash, comms light-time) and verified against it. The page must not compute a
 *      verdict, a hash, or a light-time locally for the archive. The failure mode is "someone
 *      re-adds a local reading", which no runtime probe can see.
 *
 *   2. THE ARCHIVE IS RECORDED AT THE EVENTS. A vehicle is recorded on launch, a report on export, a
 *      message on send -- "a museum cannot be retro-fitted". The recording calls must be wired into
 *      the actions that produce the work.
 *
 *   3. VERIFICATION RE-DERIVES FROM THE KERNEL. `verifyArchiveEntry` must call the kernel's own
 *      exports (rocket_verdict, pad_design_hash, comms_*) to re-derive the readings, not recompute
 *      them from a local formula.
 *
 *   4. THE HASH IS A PURE FUNCTION OF PHYSICS. `archiveSerialize` must exclude the id and the epoch,
 *      so the hash is a fingerprint of the work, reproducible across time and platform -- the replay
 *      claim. A hash that moves with the timestamp is not a verifiable record.
 *
 *   5. THE WASM BOUNDARY IS PHYSICAL. The exports the archive's verification depends on must be present
 *      and physical: the rocket readings, the pad design hash, and the comms light-time, all
 *      re-derivable from the kernel.
 *
 * Usage: node tools/check-archive.mjs            (static: no browser, no wasm needed)
 *        node tools/check-archive.mjs --wasm dist/odderon_dmath_wasm.wasm
 *                                             (also check the exports the archive depends on)
 */

import { readFile } from "node:fs/promises";
import { readFileSync } from "node:fs";

const PAGE = new URL("../web/index.html", import.meta.url);

/** Strip comments so documentation about the archive does not trip the checks that hunt for local readings. */
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

check("no local archive readings in the page", (src) => {
  // The archive captures readings from the kernel and verifies against it. A local reading would be a
  // verdict, a hash, or a light-time computed in the page instead of called from the kernel.
  const patterns = [
    [/rocket_verdict\s*=\s*[^;]*(Math|Number)\./, "a local rocket verdict"],
    [/pad_design_hash\s*=\s*[^;]*(Math|Number)\./, "a local design hash"],
    [/oneWay\s*=\s*[^;]*Math\./, "a local light-time in the archive"],
    [/archiveHash\s*=\s*[^;]*Date\.now/, "a hash that moves with the clock"],
  ];
  const hits = patterns.filter(([re]) => re.test(src)).map(([, why]) => why);
  if (hits.length) throw new Error("page still computes an archive reading locally: " + hits.join("; "));
});

check("the archive is recorded at the events that produce the work", (src) => {
  // "A museum cannot be retro-fitted": the record is built as the work is done. Each recording call
  // must be wired into the action that produces the artifact.
  const wiring = [
    [/function relaunchRocket\s*\([^)]*\)\s*\{[\s\S]*?recordVehicle\(\);/, "recordVehicle on launch"],
    [/function exportAkashi\s*\([^)]*\)\s*\{[\s\S]*?recordReport\(\);/, "recordReport on export"],
    [/function sendComms\s*\([^)]*\)\s*\{[\s\S]*?recordComms\(\);/, "recordComms on send"],
  ];
  const missing = wiring.filter(([re]) => !re.test(src)).map(([, why]) => why);
  if (missing.length) throw new Error("archive is not recorded at: " + missing.join(", "));
});

check("verification re-derives the readings from the kernel", (src) => {
  const m = src.match(/function verifyArchiveEntry\s*\([^)]*\)\s*\{([\s\S]*?)\n\}/);
  if (!m) throw new Error("verifyArchiveEntry not found");
  const body = m[1];
  // The verification must call the kernel's own exports to re-derive each entry type's readings.
  for (const accessor of [
    "rocket_verdict",
    "rocket_apogee_km",
    "pad_design_hash",
    "station_light_time_s",
    "comms_round_trip_s",
    "comms_arrival_s",
  ]) {
    if (!body.includes(accessor)) {
      throw new Error(`verifyArchiveEntry does not re-derive via ${accessor}`);
    }
  }
});

check("the archive hash is a pure function of physics, not the clock", (src) => {
  const m = src.match(/function archiveSerialize\s*\([^)]*\)\s*\{([\s\S]*?)\n\}/);
  if (!m) throw new Error("archiveSerialize not found");
  const body = m[1];
  // The serialization must build the hash from the entry's physics (type, params, readings) and must
  // NOT include the id or the epoch -- a hash that moves with the timestamp is not a verifiable record.
  if (/entry\.id/.test(body)) throw new Error("archiveSerialize includes the id in the hash");
  if (/entry\.epoch/.test(body)) throw new Error("archiveSerialize includes the epoch in the hash");
  if (!/entry\.type/.test(body)) throw new Error("archiveSerialize does not include the entry type");
});

/**
 * The exports the archive's verification depends on. The archive reuses the existing kernel exports
 * (it adds no new ones): a vehicle's verdict and readings, the pad's design hash, and the comms
 * light-time. All must be present and physical.
 */
function verifyExports(ex) {
  const need = [
    "rocket_verdict",
    "rocket_apogee_km",
    "rocket_impact_time_s",
    "rocket_range_km",
    "rocket_design_hash",
    "pad_design_hash",
    "comms_link_status",
    "station_light_time_s",
    "comms_round_trip_s",
    "comms_arrival_s",
  ];
  const missing = need.filter((n) => typeof ex[n] !== "function");
  if (missing.length) throw new Error("module is missing: " + missing.join(", "));

  const problems = [];
  // A vehicle the archive might record: the inherited launch is suborbital and short of the Kármán
  // line, so the verdict is 0 and the readings are finite. Re-deriving them is what verification does.
  const v = ex.rocket_verdict(0, 0, Math.PI / 4, 1.5);
  if (v !== 0) problems.push(`inherited launch verdict = ${v}, want 0 (suborbital)`);
  const apogee = ex.rocket_apogee_km(0, 0, Math.PI / 4, 1.5);
  if (!Number.isFinite(apogee) || apogee <= 0) problems.push(`apogee ${apogee} is not a positive finite km`);
  // The design hashes are u64, which cross the wasm ABI as BigInt -- not a finite Number. Accept
  // either, but require a real value.
  const designHash = ex.rocket_design_hash();
  if (!(typeof designHash === 'bigint' || Number.isFinite(designHash))) problems.push("rocket design hash is not a value");
  const padHash = ex.pad_design_hash();
  if (!(typeof padHash === 'bigint' || Number.isFinite(padHash))) problems.push("pad design hash is not a value");

  // A comms entry the archive might record: a station with a computable link, re-derivable.
  const status = ex.comms_link_status(0, 0, 0);
  if (status !== 1) problems.push(`comms link verdict = ${status}, want 1`);
  const oneWay = ex.station_light_time_s(0, 0, 0);
  const round = ex.comms_round_trip_s(0, 0, 0);
  if (!Number.isFinite(oneWay) || !Number.isFinite(round)) problems.push("comms readings are not finite");
  else if (Math.abs(round - 2 * oneWay) > 1e-9) problems.push(`round-trip ${round} != 2 x one-way ${oneWay}`);

  if (problems.length) throw new Error("boundary disagrees with physics: " + problems.join("; "));
  return `rocket verdict=${v} apogee=${apogee.toFixed(1)}km, comms oneWay=${oneWay.toFixed(6)}s`;
}

/** Load a built module and verify the exports the archive depends on. */
async function runWasm(path) {
  const bytes = await readFile(path);
  const { instance } = await WebAssembly.instantiate(bytes, {});
  return verifyExports(instance.exports);
}

/**
 * `--selftest`: prove the archive's hash and verification are sound, by extracting the page's own
 * functions and running them against a shim kernel. Without this, the accept and reject paths would
 * first run in a browser, and a checker whose rejection path has never executed is not known to reject.
 */
function selftest() {
  const html = readFileSync(PAGE, 'utf8');
  const src = liveSource(html);

  // Extract the archive's pure functions from the page source and evaluate them together, so the
  // self-test runs the REAL functions rather than a copy.
  function extract(name) {
    const bodyRe = new RegExp("function " + name + "\\s*\\([^)]*\\)\\s*\\{([\\s\\S]*?)\\n\\}");
    const m = src.match(bodyRe);
    if (!m) throw new Error("could not extract " + name + " from the page");
    const paramsRe = new RegExp("function " + name + "\\s*\\(([^)]*)\\)");
    const pm = m[0].match(paramsRe);
    if (!pm) throw new Error("could not read " + name + " params");
    return "function " + name + "(" + pm[1] + ") {" + m[1] + "\n}";
  }
  const fnSource = [
    extract("archiveSerialize"),
    extract("archiveHash"),
    extract("close"),
    extract("verifyArchiveEntry"),
  ].join("\n");

  // A shim kernel that re-derives the readings from the params, the way the real one does. The
  // vehicle shim returns the inherited suborbital readings; the comms shim returns a fixed light-time.
  const shimKernel = {
    rocket_verdict: () => 0,
    rocket_apogee_km: () => 58.99616489732307,
    rocket_impact_time_s: () => 223.16693056629583,
    rocket_range_km: () => 233.81531963963184,
    rocket_design_hash: () => 12345678901234567890,
    pad_design_hash: () => 98765432109876543210,
    station_light_time_s: () => 0.044,
    comms_round_trip_s: () => 0.088,
    comms_arrival_s: (lat, lon, from) => from + 0.044,
  };
  const sandbox = {
    kernelState: { module: shimKernel },
    rocketParams: { latitudeDeg: 0, azimuthDeg: 0, elevationDeg: 45, burnoutVelocity: 1.5 },
    groundStation: { latitudeDeg: 0, longitudeDeg: 0 },
    padState: { akashiEpoch: 1 },
    commsState: { sendS: 100 },
    rocketReadings: () => ({ source: "wasm kernel", verdict: 0, apogeeKm: 58.99616489732307, impactTimeS: 223.16693056629583, rangeKm: 233.81531963963184 }),
    commsReadings: () => ({ source: "wasm kernel", valid: true, oneWay: 0.044, roundTrip: 0.088, arrival: 100.044 }),
  };
  const fn = new Function(...Object.keys(sandbox), fnSource + "\nreturn { archiveSerialize, archiveHash, close, verifyArchiveEntry };");
  const { archiveSerialize, archiveHash, verifyArchiveEntry } = fn(...Object.values(sandbox));

  const vehicleEntry = {
    type: "vehicle",
    params: { lat: 0, az: 0, el: 45, v: 1.5 },
    readings: { verdict: 0, apogee: 58.99616489732307, impact: 223.16693056629583, range: 233.81531963963184, hash: 12345678901234567890 },
  };
  const commsEntry = {
    type: "comms",
    params: { lat: 0, lon: 0, sendS: 100 },
    readings: { oneWay: 0.044, roundTrip: 0.088, arrival: 100.044 },
  };

  const results = [];
  // Hash determinism: the same physics hashes the same.
  const h1 = archiveHash(vehicleEntry);
  const h2 = archiveHash(vehicleEntry);
  results.push(["the hash is deterministic", h1 === h2]);
  // Physics-only: changing the id or epoch does not move the hash.
  const withId = { ...vehicleEntry, id: 999, epoch: 12345 };
  results.push(["the hash excludes the id and epoch", archiveHash(withId) === h1]);
  // Tamper-sensitivity: changing a reading moves the hash.
  const tampered = { ...vehicleEntry, readings: { ...vehicleEntry.readings, apogee: 999 } };
  results.push(["a tampered reading changes the hash", archiveHash(tampered) !== h1]);
  // Verification: a correct entry verifies against the shim kernel.
  const vOk = verifyArchiveEntry({ ...vehicleEntry, hash: h1 });
  results.push(["a correct entry verifies against the kernel", vOk.ok === true]);
  // Verification: a tampered reading is caught.
  const vBad = verifyArchiveEntry({ ...vehicleEntry, readings: { ...vehicleEntry.readings, apogee: 999 }, hash: archiveHash({ ...vehicleEntry, readings: { ...vehicleEntry.readings, apogee: 999 } }) });
  results.push(["a tampered reading fails verification", vBad.ok === false]);
  // Verification: a comms entry verifies.
  const cOk = verifyArchiveEntry({ ...commsEntry, hash: archiveHash(commsEntry) });
  results.push(["a comms entry verifies against the kernel", cOk.ok === true]);

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
  console.log(bad ? `${bad} self-test(s) FAILED` : "self-test passed: the archive hash is sound and verification catches tampering");
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
  console.log(`${failed} archive check(s) FAILED`);
  process.exit(1);
}
console.log(`all ${checks.length} archive checks passed`);
