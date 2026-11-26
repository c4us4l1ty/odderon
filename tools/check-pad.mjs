#!/usr/bin/env node
/**
 * The gate for P1's claim: a reproducible failure from the input log.
 *
 * `GAME-DESIGN-BIBLE.md` §17 and `docs/BUILD-PLAN.md` §5 make the gate to leave P1 *a reproducible
 * failure from the input log*: the player's clipboard drives a chilldown, the cryo/fluid network decides
 * whether the pump cavitates, and the same checklist fails the same way every run. `web/index.html` is the
 * clipboard; `odderon_astro::cryo` (exposed through the wasm module) is the network.
 *
 * Three parts, each able to fail against the code as it could be wrong:
 *
 *   1. THE PAGE DRIVES THE KERNEL, AND THE KERNEL IS IN THE CONTRACT. The page must call `pad_failure_time`
 *      at a real call site (not in a comment), and the `needed` export list in `loadKernel()` must name
 *      every pad export the page reads. A page that computes the failure itself, or that reads an export the
 *      contract does not promise, fails here.
 *
 *   2. THE CHECKLIST IS THE INPUT LOG. The clipboard must default to the bible's under-specified first load
 *      -- a short chill and no purges -- so the first thing the panel shows is the productive failure. A
 *      page that defaults to a correct chilldown hides the failure the phase exists to make visible.
 *
 *   3. THE KERNEL'S FAILURE IS REPRODUCIBLE, MEASURED FROM THE BINARY. With a build, the wasm exports are
 *      run: the under-specified plan cavitates, the specified plan survives, the same log reproduces the
 *      same failure time, the saturation curve anchors at one atmosphere, and a non-finite fill rate is
 *      refused. Without a build the static half still runs; the boundary is skipped with a note, the same
 *      way `check-orbit.mjs` does.
 *
 * `--selftest` drives the boundary verifier with a deliberate shim so both the accept and reject paths run
 * here, because a checker whose rejection path has never executed is not yet known to reject anything.
 *
 * Usage: node tools/check-pad.mjs                (static: no browser, no wasm needed)
 *        node tools/check-pad.mjs --wasm dist/odderon_dmath_wasm.wasm
 */

import { readFile } from "node:fs/promises";

const PAGE = new URL("../web/index.html", import.meta.url);
const WASM = new URL("../dist/odderon_dmath_wasm.wasm", import.meta.url);

/** Strip comments so documentation *about* the pad does not trip the checks that hunt a missing call site. */
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

// --- 1: the page drives the kernel, and the kernel is in the contract ---------------------------------

check("the page calls the pad's failure export (a real call site, not a comment)", (src) => {
  if (!/k\.pad_failure_time\s*\(/.test(src)) {
    throw new Error("the page does not call pad_failure_time(); the clipboard must drive the kernel");
  }
});

check("the page calls the pad's inlet-vapour-fraction export", (src) => {
  if (!/k\.pad_inlet_vapour_fraction\s*\(/.test(src)) {
    throw new Error("the page does not call pad_inlet_vapour_fraction()");
  }
});

check("the kernel export list names every pad export the page calls", (src) => {
  // The `needed` array is the page's contract that a partially-built module fails at the boundary. Extract
  // it and check it against the pad call sites, the way `check-room-clock.mjs` does for the room.
  const nl = src.indexOf("const needed =");
  if (nl < 0) throw new Error("page has no `const needed =` export contract");
  const needed = [...src.slice(nl, src.indexOf("];", nl)).matchAll(/'([a-z_]+)'/g)].map((m) => m[1]);
  const padExports = ["pad_failure_time", "pad_inlet_vapour_fraction", "pad_is_wreck", "pad_liquid_kg",
    "pad_pressure_kpa", "pad_temperature_k", "pad_vapour_pressure_kpa",
    "pad_chilldown_residual_heat", "pad_flash_fraction"];
  const missing = padExports.filter((n) => !needed.includes(n));
  if (missing.length) throw new Error("pad exports missing from the contract: " + missing.join(", "));
});

check("the page holds no cryo constant of its own", (src) => {
  // The room rule, applied to the pad: every number comes from the kernel. The page's only pad constants
  // are the fill window and the reference fill rate, both named and derived. A page that re-derives the
  // chilldown, the latent heat or the threshold is a second copy of the model that can drift.
  for (const forbidden of ["LOX_LATENT_HEAT", "CAVITATION_THRESHOLD", "LOX_NBP_K", "BLEED_FRACTION"]) {
    if (new RegExp("\\b" + forbidden + "\\b").test(src)) {
      throw new Error(`the page carries its own ${forbidden}; it must read it from the kernel`);
    }
  }
});

// --- 2: the checklist is the input log ---------------------------------------------------------------

check("the clipboard defaults to the bible's under-specified first load", (src) => {
  // The productive failure is the point of P1: the first load is under-specified on purpose, so the panel
  // must start on a plan that fails. A page that defaults to a correct chill hides the failure.
  const m = src.match(/const PAD_CHECKLIST = \{ chillS: ([^,]+), bleeds: (\d+) \}/);
  if (!m) throw new Error("no named PAD_CHECKLIST constant: the default checklist must be one obvious value");
  const chillS = Number(m[1]);
  const bleeds = Number(m[2]);
  if (!(chillS > 0 && chillS < 60)) {
    throw new Error(`PAD_CHECKLIST chillS=${chillS} is not the under-specified first load (a short chill)`);
  }
  if (bleeds !== 0) {
    throw new Error(`PAD_CHECKLIST bleeds=${bleeds} is not the under-specified first load (no purges)`);
  }
});

check("the fill button runs the pad", (src) => {
  if (!/pad-fill'\)\.addEventListener\s*\(\s*'click'\s*,\s*\(\s*\)\s*=>\s*\{\s*runPad\(\)/.test(src)) {
    throw new Error("the fill button does not call runPad()");
  }
});

// --- 2b: the wreck is recorded and examinable ---------------------------------------------------------

check("the page asks the kernel whether a fill leaves a wreck", (src) => {
  // The wreck is the kernel's decision, not the page's: the page must call pad_is_wreck() rather than
  // deciding on its own that a run failed. A page that invents a wreck from its own arithmetic is the same
  // defect as a page that invents a temperature.
  if (!/k\.pad_is_wreck\s*\(/.test(src)) {
    throw new Error("the page does not call pad_is_wreck(); the wreck must come from the kernel");
  }
});

check("a failed fill is recorded as an examinable wreck", (src) => {
  // The bible's rule: the wreckage is always examinable, never cut to black on failure. The page must keep
  // a wreck record that carries the input log (so it is reproducible from itself), pushed when the kernel
  // says the run is a wreck.
  if (!/padState\.wrecks\.push\(/.test(src)) {
    throw new Error("the page does not record wrecks on the stand");
  }
  // The record must carry the input log: chill, bleeds, fill rate and window, so a reader can re-simulate
  // it. A wreck that only carries a verdict is not examinable.
  const push = src.match(/padState\.wrecks\.push\(\{([\s\S]{0,400}?)\}\)/);
  if (!push) throw new Error("the wreck record is not a pushed object");
  for (const field of ["chillS", "bleeds", "fillRate", "window", "failureTime"]) {
    if (!new RegExp("\\b" + field + "\\b").test(push[1])) {
      throw new Error(`the wreck record does not carry ${field}; it must carry the input log`);
    }
  }
});

check("the wreck stand is rendered, not merely stored", (src) => {
  // A wreck kept in a variable and never shown is a record nobody can examine. The stand must be rendered
  // from the wreck list.
  if (!/function renderStand/.test(src) || !/pad-stand/.test(src)) {
    throw new Error("the wreck stand is not rendered");
  }
});

// --- 3: the kernel's failure, measured from the binary ------------------------------------------------

/**
 * The physics the pad exports must satisfy, as a function of the exports alone. Separated from the file
 * loading because the wasm build only exists in CI; `--selftest` drives it with a shim so both paths run.
 */
function verifyPad(ex) {
  const problems = [];

  // The under-specified first load must cavitate: that is the bible's productive failure.
  const underT = ex.pad_failure_time(5.0, 0, 0.50, 60.0);
  if (!(underT >= 0)) {
    problems.push(`the under-specified load did not cavitate (failure time ${underT})`);
  }

  // The specified chilldown must survive: the player learns chilldown by fixing the checklist.
  const specT = ex.pad_failure_time(300.0, 3, 0.50, 60.0);
  if (specT !== -1) {
    problems.push(`the specified load cavitated (failure time ${specT}); it must run clean`);
  }

  // The inlet vapour fraction is the quantity the predicate reads: under-specified crosses the threshold,
  // specified stays below it.
  const underFrac = ex.pad_inlet_vapour_fraction(5.0, 0, 0.50, 60.0);
  const specFrac = ex.pad_inlet_vapour_fraction(300.0, 3, 0.50, 60.0);
  if (!(underFrac > specFrac)) {
    problems.push(`inlet fraction did not separate the plans: ${underFrac} vs ${specFrac}`);
  }

  // REPRODUCIBLE: the same input log, run twice, returns the same failure time exactly.
  const a = ex.pad_failure_time(5.0, 0, 0.50, 120.0);
  const b = ex.pad_failure_time(5.0, 0, 0.50, 120.0);
  if (a !== b) {
    problems.push(`the same log gave two failure times: ${a} vs ${b}`);
  }

  // THE WRECK PREDICATE agrees with the failure: a run is a wreck exactly when it cavitated. A predicate
  // that disagreed with the failure it summarises would let the page record a wreck for a clean run -- or
  // miss one for a failed run.
  const wreckUnder = ex.pad_is_wreck(5.0, 0, 0.50, 60.0);
  const wreckSpec = ex.pad_is_wreck(300.0, 3, 0.50, 60.0);
  if (wreckUnder !== 1) problems.push(`the under-specified load is not a wreck (pad_is_wreck=${wreckUnder})`);
  if (wreckSpec !== 0) problems.push(`the specified load is a wreck (pad_is_wreck=${wreckSpec})`);
  // ...and it is reproducible from the input log, like the failure itself.
  if (ex.pad_is_wreck(5.0, 0, 0.50, 120.0) !== ex.pad_is_wreck(5.0, 0, 0.50, 120.0)) {
    problems.push("pad_is_wreck is not reproducible from the input log");
  }
  // A refused run is not a wreck: a run that never happened leaves nothing to examine.
  if (ex.pad_is_wreck(5.0, 0, NaN, 60.0) !== 0) {
    problems.push("a NaN fill rate was recorded as a wreck");
  }

  // The saturation curve anchors at the normal boiling point: one atmosphere, by construction.
  const pAtNbp = ex.pad_vapour_pressure_kpa(90.188);
  if (Math.abs(pAtNbp - 101.325) > 1e-9) {
    problems.push(`P_sat(NBP) = ${pAtNbp}, want exactly 101.325 kPa`);
  }

  // A non-finite fill rate is refused, not answered with a number.
  if (Number.isFinite(ex.pad_failure_time(5.0, 0, NaN, 60.0))) {
    problems.push("a NaN fill rate was answered with a number instead of refused");
  }

  if (problems.length) throw new Error("pad boundary disagrees with the claim: " + problems.join("; "));
  return `under-specified fails at ${underT} s, specified survives, inlet ${underFrac} vs ${specFrac}, reproducible`;
}

/** `--selftest`: prove `verifyPad` accepts a correct boundary and rejects the ways it can be wrong. */
function selftest() {
  // A shim, not the kernel: it has the three properties `verifyPad` checks -- the under-specified plan
  // fails, the specified one survives, and a non-finite fill rate is refused (NaN, not a number). It is
  // *honest shim behaviour* rather than a pretend network, which is the distinction that matters here.
  const good = {
    pad_failure_time: (chill, bleeds, fill, t) => {
      if (!Number.isFinite(fill)) return NaN;
      return chill < 60 && bleeds === 0 ? 0.0 : -1.0;
    },
    pad_inlet_vapour_fraction: (chill, bleeds) => (chill < 60 && bleeds === 0 ? 1.0 : 0.0),
    pad_is_wreck: (chill, bleeds, fill) => (Number.isFinite(fill) && chill < 60 && bleeds === 0 ? 1 : 0),
    pad_vapour_pressure_kpa: (t) => (Math.abs(t - 90.188) < 1e-9 ? 101.325 : 200.0),
  };
  const results = [];
  try {
    verifyPad(good);
    results.push(["a correct boundary is accepted", true]);
  } catch (err) {
    results.push([`a correct boundary is accepted (${err.message})`, false]);
  }

  // The exact defect P1 exists to prevent: a boundary where the under-specified load does NOT fail.
  const noFailure = { ...good, pad_failure_time: () => -1.0 };
  try {
    verifyPad(noFailure);
    results.push(["an under-specified load that never fails is rejected", false]);
  } catch {
    results.push(["an under-specified load that never fails is rejected", true]);
  }

  // A boundary that fails the specified load too: the player cannot learn by fixing the checklist.
  const alwaysFails = { ...good, pad_failure_time: () => 0.0 };
  try {
    verifyPad(alwaysFails);
    results.push(["a boundary that fails every load is rejected", false]);
  } catch {
    results.push(["a boundary that fails every load is rejected", true]);
  }

  // A boundary that is not reproducible: the same log gives two answers.
  const unstable = { ...good, pad_failure_time: (chill, bleeds, fill, t) => (t > 60 ? 0.0 : 3.0) };
  try {
    verifyPad(unstable);
    results.push(["an unreproducible failure is rejected", false]);
  } catch {
    results.push(["an unreproducible failure is rejected", true]);
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
    const bytes = await readFile(path);
    const { instance } = await WebAssembly.instantiate(bytes, {});
    const detail = verifyPad(instance.exports);
    console.log(`  ok   pad boundary: ${detail}`);
  } catch (e) {
    failed++;
    console.log(`  FAIL pad boundary`);
    console.log(`       ${e.message}`);
  }
} else {
  console.log("  ..   pad boundary not run (pass --wasm <file> where a build exists)");
}

console.log("");
if (failed) {
  console.log(`${failed} pad check(s) FAILED`);
  process.exit(1);
}
console.log(`all ${checks.length} pad checks passed`);
