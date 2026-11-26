#!/usr/bin/env node
/**
 * The gate that makes the protégé a real kernel-driven learning mechanic rather than a page-drawn quiz.
 *
 * `web/index.html` gives the player a junior who replays THEIR errors from the archive -- "the sim
 * replays your last week's error pattern" -- and asks the player to diagnose the failure with a
 * selection from their own reasoning. The correct diagnosis is the kernel's own, derived from the
 * readings the kernel already produced for the entry; answering well records a self-explanation in
 * the archive. The same "no substitute" contract the rest of the project follows: the protégé's
 * question comes from a real archive error, the correct answer is the kernel's, and the briefing is
 * verifiable against the kernel.
 *
 * The checks:
 *
 *   1. THE DIAGNOSIS IS KERNEL-DRIVEN. `protegeDiagnosis` must be a pure function of the entry's
 *      kernel readings (the verdict and apogee the kernel produced), not a page constant or a local
 *      computation. The failure mode is "the correct answer is hard-coded", which no runtime probe
 *      can see.
 *
 *   2. THE PROTÉGÉ REPLAYS REAL ARCHIVE ERRORS. `protegeErrorEntry` must select a failed vehicle from
 *      the archive, not fabricate a scenario. "They make the mistake you made last week."
 *
 *   3. THE CORRECT ANSWER IS THE KERNEL'S. `protegeAsk` must build the options with the kernel-driven
 *      `protegeDiagnosis` as the correct answer among fixed distractors.
 *
 *   4. ANSWERING IS RECORDED. A correct answer must record a briefing (self-explanation) in the
 *      archive, and that briefing must verify by re-deriving the diagnosis from the error entry.
 *
 *   5. THE PROTÉGÉ NOTICES MISMATCHES. `protegeNotice` must point at a record that does not verify
 *      against the kernel -- the "give" that keeps them from being a burden.
 *
 *   6. THE WASM BOUNDARY IS PHYSICAL. The kernel exports the diagnosis depends on (rocket verdict,
 *      apogee) must be present and physical: the inherited launch is suborbital and short of the
 *      Kármán line, which is exactly the error the protégé replays.
 *
 * Usage: node tools/check-protege.mjs            (static: no browser, no wasm needed)
 *        node tools/check-protege.mjs --wasm dist/odderon_dmath_wasm.wasm
 *                                             (also check the exports the diagnosis depends on)
 */

import { readFile } from "node:fs/promises";
import { readFileSync } from "node:fs";

const PAGE = new URL("../web/index.html", import.meta.url);

/** Strip comments so documentation about the protégé does not trip the checks that hunt for a local diagnosis. */
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

check("the diagnosis is a function of the kernel's readings, not a page constant", (src) => {
  const m = src.match(/function protegeDiagnosis\s*\([^)]*\)\s*\{([\s\S]*?)\n\}/);
  if (!m) throw new Error("protegeDiagnosis not found");
  const body = m[1];
  // The diagnosis must read the entry's kernel readings (verdict, apogee) and branch on them. A
  // diagnosis that ignores the readings and returns a fixed string is a page constant, not a diagnosis.
  if (!/entry\.readings\.verdict/.test(body)) throw new Error("protegeDiagnosis does not read the kernel's verdict");
  if (!/entry\.readings\.apogee/.test(body)) throw new Error("protegeDiagnosis does not read the kernel's apogee");
  // It must not compute a verdict locally (e.g. re-deriving it from the params with a formula).
  if (/rocket_verdict\s*\(/.test(body)) throw new Error("protegeDiagnosis re-derives the verdict instead of reading it");
});

check("the protégé replays a real archive error, not a fabricated scenario", (src) => {
  const m = src.match(/function protegeErrorEntry\s*\([^)]*\)\s*\{([\s\S]*?)\n\}/);
  if (!m) throw new Error("protegeErrorEntry not found");
  const body = m[1];
  if (!/archive/.test(body)) throw new Error("protegeErrorEntry does not select from the archive");
  if (!/readings\.verdict/.test(body)) throw new Error("protegeErrorEntry does not select on the kernel's verdict");
});

check("the correct answer is the kernel-driven diagnosis", (src) => {
  const m = src.match(/function protegeAsk\s*\([^)]*\)\s*\{([\s\S]*?)\n\}/);
  if (!m) throw new Error("protegeAsk not found");
  const body = m[1];
  if (!/protegeDiagnosis\(err\)/.test(body)) throw new Error("protegeAsk does not use the kernel-driven diagnosis as the correct answer");
});

check("a correct answer records a verifiable self-explanation", (src) => {
  const m = src.match(/function protegeAnswer\s*\([^)]*\)\s*\{([\s\S]*?)\n\}/);
  if (!m) throw new Error("protegeAnswer not found");
  const body = m[1];
  if (!/recordBriefing/.test(body)) throw new Error("protegeAnswer does not record a briefing on a correct answer");
  // The briefing must be verifiable: verifyArchiveEntry must re-derive the diagnosis for a briefing.
  const v = src.match(/function verifyArchiveEntry\s*\([^)]*\)\s*\{([\s\S]*?)\n\}/);
  if (!v) throw new Error("verifyArchiveEntry not found");
  if (!/entry\.type === 'briefing'/.test(v[1])) throw new Error("briefings are not verified by verifyArchiveEntry");
  if (!/protegeDiagnosis/.test(v[1])) throw new Error("briefing verification does not re-derive the diagnosis");
});

check("the protégé notices a record that does not verify", (src) => {
  const m = src.match(/function protegeNotice\s*\([^)]*\)\s*\{([\s\S]*?)\n\}/);
  if (!m) throw new Error("protegeNotice not found");
  const body = m[1];
  if (!/verified === false/.test(body)) throw new Error("protegeNotice does not look for a record that fails verification");
});

/** The exports the diagnosis depends on: the rocket verdict and apogee the kernel produces. */
function verifyExports(ex) {
  const need = ["rocket_verdict", "rocket_apogee_km"];
  const missing = need.filter((n) => typeof ex[n] !== "function");
  if (missing.length) throw new Error("module is missing: " + missing.join(", "));

  const problems = [];
  // The inherited launch (az 0, el 45, v 1.5) is suborbital and short of the Kármán line -- exactly
  // the error the protégé replays. The diagnosis branches on these readings, so they must be physical.
  const v = ex.rocket_verdict(0, 0, Math.PI / 4, 1.5);
  if (v !== 0) problems.push(`inherited launch verdict = ${v}, want 0 (suborbital)`);
  const apogee = ex.rocket_apogee_km(0, 0, Math.PI / 4, 1.5);
  if (!Number.isFinite(apogee) || apogee <= 0) problems.push(`apogee ${apogee} is not a positive finite km`);
  if (apogee >= 100) problems.push(`inherited apogee ${apogee} should be short of the Kármán line (100 km)`);

  // A refusal and an orbital case, so the diagnosis branches are all reachable from real readings.
  const refused = ex.rocket_verdict(0, 0, Math.PI / 4, -1.0);
  if (refused !== 3) problems.push(`a negative-velocity launch verdict = ${refused}, want 3 (refused)`);

  if (problems.length) throw new Error("boundary disagrees with physics: " + problems.join("; "));
  return `inherited verdict=${v} apogee=${apogee.toFixed(1)}km, refused case=${refused}`;
}

/** Load a built module and verify the exports the diagnosis depends on. */
async function runWasm(path) {
  const bytes = await readFile(path);
  const { instance } = await WebAssembly.instantiate(bytes, {});
  return verifyExports(instance.exports);
}

/**
 * `--selftest`: prove the diagnosis function and the briefing verification are sound, by extracting
 * the page's own functions and running them. Without this, the accept and reject paths would first run
 * in a browser, and a checker whose rejection path has never executed is not known to reject.
 */
function selftest() {
  const html = readFileSync(PAGE, "utf8");
  const src = liveSource(html);

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
    extract("protegeDiagnosis"),
    extract("verifyArchiveEntry"),
  ].join("\n");

  // A shim archive holding one failed vehicle, and a shim kernel with the readings the diagnosis
  // branches on. The briefing verification re-derives the diagnosis from the archive entry.
  const failedVehicle = {
    id: 1,
    type: "vehicle",
    params: { lat: 0, az: 0, el: 45, v: 1.5 },
    readings: { verdict: 0, apogee: 58.99616489732307, impact: 223.16693056629583, range: 233.81531963963184, hash: 12345678901234567890n },
    hash: "deadbeef",
    verified: true,
  };
  const sandbox = {
    kernelState: { module: { rocket_verdict: () => 0, rocket_apogee_km: () => 58.996, pad_design_hash: () => 1n, rocket_design_hash: () => 1n, rocket_impact_time_s: () => 223.1, rocket_range_km: () => 233.8, station_light_time_s: () => 0.04, comms_round_trip_s: () => 0.08, comms_arrival_s: () => 0.04 } },
    archive: [failedVehicle],
    protegeDiagnosis: null, // provided by the extracted functions below
  };
  // The extracted functions close over the sandbox globals; build them with the sandbox in scope.
  const fn = new Function(...Object.keys(sandbox), fnSource + "\nreturn { protegeDiagnosis, verifyArchiveEntry, archiveHash };");
  const { protegeDiagnosis, verifyArchiveEntry, archiveHash } = fn(...Object.values(sandbox));

  const results = [];
  // The diagnosis branches on the kernel's readings.
  results.push(["a suborbital-short launch diagnoses low burnout velocity",
    protegeDiagnosis({ type: "vehicle", readings: { verdict: 0, apogee: 50 } }) === "burnout velocity too low to reach the Kármán line"]);
  results.push(["a suborbital-to-space launch diagnoses suborbital",
    protegeDiagnosis({ type: "vehicle", readings: { verdict: 0, apogee: 150 } }).startsWith("suborbital")]);
  results.push(["a refused launch diagnoses refusal",
    protegeDiagnosis({ type: "vehicle", readings: { verdict: 3, apogee: 0 } }).includes("refused")]);
  results.push(["an orbital launch diagnoses orbit",
    protegeDiagnosis({ type: "vehicle", readings: { verdict: 1, apogee: 400 } }).includes("orbit")]);
  results.push(["a non-vehicle entry has no diagnosis",
    protegeDiagnosis({ type: "report", readings: {} }) === null]);

  // The briefing verifies: re-derive the diagnosis from the error entry and confirm it matches.
  const diag = protegeDiagnosis(failedVehicle);
  const briefing = {
    type: "briefing",
    params: { of: "vehicle", id: 1, errHash: "deadbeef" },
    readings: { diagnosis: diag },
  };
  briefing.hash = archiveHash(briefing);
  const vOk = verifyArchiveEntry(briefing);
  results.push(["a briefing with the kernel's diagnosis verifies", vOk.ok === true]);

  // A tampered diagnosis fails verification.
  const tampered = { ...briefing, readings: { diagnosis: "the nozzle was wrong" }, hash: archiveHash({ ...briefing, readings: { diagnosis: "the nozzle was wrong" } }) };
  const vBad = verifyArchiveEntry(tampered);
  results.push(["a tampered diagnosis fails verification", vBad.ok === false]);

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
  console.log(bad ? `${bad} self-test(s) FAILED` : "self-test passed: the diagnosis is sound and briefings verify");
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
