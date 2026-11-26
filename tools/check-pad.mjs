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
    "pad_chilldown_residual_heat", "pad_flash_fraction",
    // P2: The Mark -- the ghost twin, the one arrow, and the design hash.
    "pad_ghost_twin_actual", "pad_ghost_twin_divergence",
    "pad_attribution_which", "pad_attribution_sensitivity_chill", "pad_attribution_sensitivity_bleeds",
    "pad_design_hash"];
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

// --- 2c: the debrief is the consolidation step, grounded in the wreck ----------------------------------

check("the page debriefs a wreck, grounded in the wreck's own numbers", (src) => {
  // The bible's consolidation step after a productive failure. It must be derived from the wreck -- reading
  // its residual heat, inlet fraction and failure time -- not a fixed string that says the same thing for
  // every wreck. A debrief that does not reference the wreck it explains is instruction dressed as a
  // diagnosis, and does not consolidate anything.
  const m = src.match(/function debriefWreck\([^)]*\)\s*\{([\s\S]*?)\n\}/);
  if (!m) throw new Error("the page has no debriefWreck() function");
  const body = m[1];
  for (const field of ["residualHeat", "inletFraction", "failureTime"]) {
    if (!new RegExp("\\b" + field + "\\b").test(body)) {
      throw new Error(`the debrief does not read the wreck's ${field}; it must be grounded in the wreck`);
    }
  }
});

check("the debrief explains the cause and the remedy", (src) => {
  // A debrief that only says what happened is a status line. It must say why (the cause) and what to do
  // (the remedy) -- the two halves of a consolidation.
  const m = src.match(/function debriefWreck\([^)]*\)\s*\{([\s\S]*?)\n\}/);
  if (!m) throw new Error("the page has no debriefWreck() function");
  if (!/cause/.test(m[1]) || !/remedy/.test(m[1])) {
    throw new Error("the debrief does not state both a cause and a remedy");
  }
});

check("the debrief's remedy is verified by the kernel, not asserted", (src) => {
  // The remedy must be a checklist the kernel says runs clean -- derived by calling pad_is_wreck -- rather
  // than a suggestion the page invents. The same pure function that produced the failure verifies the fix,
  // so the debrief cannot recommend something that would wreck again.
  const m = src.match(/function suggestCleanChecklist\([^)]*\)\s*\{([\s\S]*?)\n\}/);
  if (!m) throw new Error("the page has no suggestCleanChecklist() function");
  if (!/k\.pad_is_wreck\s*\(/.test(m[1])) {
    throw new Error("the remedy is not verified by pad_is_wreck(); it must come from the kernel");
  }
});

check("the debrief is rendered beside the wreck", (src) => {
  // A debrief computed but never shown consolidates nothing. It must be rendered for the selected wreck.
  if (!/function renderDebrief/.test(src) || !/pad-debrief/.test(src)) {
    throw new Error("the debrief is not rendered");
  }
});

// --- 2d: P2 -- the grease pencil, ghost twin, one arrow, Akashi ----------------------------------------

check("the page places a grease-pencil mark (the player's expectation)", (src) => {
  // §6.1: the mark is the player's expectation, recorded before the fill. The page must have a place-mark
  // function that stores the expected value -- a pure input, not a reading. A page with no mark control
  // has no Loop of Consequence to fire.
  if (!/function placeMark/.test(src) || !/padState\.mark/.test(src)) {
    throw new Error("the page has no grease-pencil mark: no placeMark() storing padState.mark");
  }
});

check("the page clears a grease-pencil mark", (src) => {
  // The mark is optional: clearing it must reset the ghost twin and the arrow, so a cleared mark does not
  // leave stale readings behind. A page that clears the mark but keeps the ghost is showing a prediction
  // error for a prediction that no longer exists.
  const m = src.match(/function clearMark\([^)]*\)\s*\{([\s\S]*?)\n\}/);
  if (!m) throw new Error("the page has no clearMark() function");
  if (!/padState\.mark = null/.test(m[1]) || !/padState\.ghost = null/.test(m[1])) {
    throw new Error("clearMark() does not reset the mark and the ghost twin");
  }
});

check("the page computes the ghost twin from the kernel, not from its own arithmetic", (src) => {
  // §6.2: the ghost twin is the kernel's computation. The page must call the ghost twin exports (actual
  // and divergence), not compute δ itself. A page that subtracts its own actual from the mark is a page
  // telling itself what it expected -- the same defect as a page computing its own temperature.
  const m = src.match(/function computeGhostTwin\([^)]*\)\s*\{([\s\S]*?)\n\}/);
  if (!m) throw new Error("the page has no computeGhostTwin() function");
  const body = m[1];
  if (!/k\.pad_ghost_twin_actual\s*\(/.test(body) || !/k\.pad_ghost_twin_divergence\s*\(/.test(body)) {
    throw new Error("computeGhostTwin() does not read both ghost twin exports from the kernel");
  }
});

check("the page reads the one arrow from the kernel's attribution", (src) => {
  // §6.2: the one arrow is the kernel's sensitivity analysis. The page must call the attribution exports
  // (the verdict and both sensitivities), not name the contributor itself. A page that hardcodes which
  // input is responsible is a lecture, not an attribution.
  const m = src.match(/function computeGhostTwin\([^)]*\)\s*\{([\s\S]*?)\n\}/);
  if (!m) throw new Error("the page has no computeGhostTwin() function");
  const body = m[1];
  for (const ex of ["pad_attribution_which", "pad_attribution_sensitivity_chill",
    "pad_attribution_sensitivity_bleeds"]) {
    if (!new RegExp("k\\." + ex + "\\s*\\(").test(body)) {
      throw new Error(`computeGhostTwin() does not call k.${ex}(); the arrow must come from the kernel`);
    }
  }
});

check("the ghost twin is rendered after a fill", (src) => {
  // The Loop of Consequence fires end-to-end: a fill with a mark produces the ghost twin. The page must
  // call computeGhostTwin() after a run when a mark is placed, so the divergence is visible.
  const m = src.match(/function runPad\([^)]*\)\s*\{([\s\S]*?)\n\}/);
  if (!m) throw new Error("the page has no runPad() function");
  if (!/computeGhostTwin\s*\(/.test(m[1])) {
    throw new Error("runPad() does not compute the ghost twin after a fill");
  }
});

check("the page exports an Akashi entry as a self-contained report", (src) => {
  // §4.4: the Akashi entry is the product. The page must have an export function that generates a
  // report from the kernel's readings (design hash, ghost twin, arrow, wreck) -- not a stub.
  const m = src.match(/function exportAkashi\([^)]*\)\s*\{([\s\S]*?)\n\}/);
  if (!m) throw new Error("the page has no exportAkashi() function");
  const body = m[1];
  if (!/pad_design_hash/.test(body)) {
    throw new Error("exportAkashi() does not read the design hash from the kernel");
  }
  if (!/Blob/.test(body) || !/download/.test(body)) {
    throw new Error("exportAkashi() does not produce a downloadable report");
  }
});

check("the Akashi entry is stamped with an epoch counter", (src) => {
  // Two exports of the same input log must be distinguishable. The epoch is a page counter that
  // increments per export, so the report's identity is (design hash, epoch, input log).
  const m = src.match(/function exportAkashi\([^)]*\)\s*\{([\s\S]*?)\n\}/);
  if (!m) throw new Error("the page has no exportAkashi() function");
  if (!/akashiEpoch/.test(m[1])) {
    throw new Error("exportAkashi() does not use an epoch counter");
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

  // --- P2: The Mark -- the ghost twin, the one arrow, the design hash ----------------------------------

  // THE GHOST TWIN: the divergence is actual - expected. The under-specified plan flashes to ~1.0; marking
  // below that produces a positive δ, marking above produces a negative one. The sign and magnitude must
  // be the arithmetic difference, measured from the binary, not a page-side guess.
  const gtActual = ex.pad_ghost_twin_actual(5.0, 0, 0.5, 0.50, 60.0);
  const gtDiv = ex.pad_ghost_twin_divergence(5.0, 0, 0.5, 0.50, 60.0);
  if (!(Number.isFinite(gtActual) && Number.isFinite(gtDiv))) {
    problems.push(`the ghost twin was refused for a valid mark (actual=${gtActual}, divergence=${gtDiv})`);
  } else if (Math.abs(gtDiv - (gtActual - 0.5)) > 1e-9) {
    problems.push(`the ghost twin divergence ${gtDiv} is not actual (${gtActual}) minus expected (0.5)`);
  }
  // A negative divergence: marking above the actual.
  const gtDivHigh = ex.pad_ghost_twin_divergence(5.0, 0, gtActual + 0.1, 0.50, 60.0);
  if (!(gtDivHigh < 0)) {
    problems.push(`a mark above the actual produced a non-negative divergence (${gtDivHigh})`);
  }
  // A non-finite mark is refused.
  if (Number.isFinite(ex.pad_ghost_twin_actual(5.0, 0, NaN, 0.50, 60.0))) {
    problems.push("a NaN mark was answered with a number instead of refused");
  }
  // The ghost twin is reproducible.
  if (ex.pad_ghost_twin_actual(5.0, 0, 0.5, 0.50, 60.0) !== gtActual ||
      ex.pad_ghost_twin_divergence(5.0, 0, 0.5, 0.50, 60.0) !== gtDiv) {
    problems.push("the ghost twin is not reproducible from the same mark and plan");
  }

  // THE ONE ARROW: an interior-flash plan (chill_s=60, bleeds=0) produces a live arrow. The under-specified
  // plan flashes to 1.0 (the rail), where the arrow correctly points at nothing. The arrow names the larger
  // of the two sensitivities, and both must be non-negative and finite.
  const arrowWhich = ex.pad_attribution_which(60.0, 0, 0.1, 0.50, 60.0);
  const arrowChill = ex.pad_attribution_sensitivity_chill(60.0, 0, 0.1, 0.50, 60.0);
  const arrowBleeds = ex.pad_attribution_sensitivity_bleeds(60.0, 0, 0.1, 0.50, 60.0);
  if (arrowWhich === 0) {
    problems.push("the interior-flash plan produced no arrow; a divergent mark must attribute");
  } else if (arrowWhich === 1) {
    if (!(arrowChill > arrowBleeds)) {
      problems.push(`the arrow says chill time but bleeds sensitivity (${arrowBleeds}) is larger (${arrowChill})`);
    }
  } else if (arrowWhich === 2) {
    if (!(arrowBleeds > arrowChill)) {
      problems.push(`the arrow says bleeds but chill sensitivity (${arrowChill}) is larger (${arrowBleeds})`);
    }
  } else {
    problems.push(`the arrow returned an unknown verdict: ${arrowWhich}`);
  }
  // The rail plan (flash=1.0) produces no arrow: the needle does not respond to the plan at the rail.
  const railWhich = ex.pad_attribution_which(5.0, 0, 0.5, 0.50, 60.0);
  if (railWhich !== 0) {
    problems.push(`the rail plan produced an arrow (${railWhich}); it must point at nothing`);
  }
  // The arrow is reproducible.
  if (ex.pad_attribution_which(60.0, 0, 0.1, 0.50, 60.0) !== arrowWhich) {
    problems.push("the one arrow is not reproducible from the same mark and plan");
  }

  // THE DESIGN HASH: deterministic and non-zero. An Akashi entry is stamped with it.
  const hash1 = ex.pad_design_hash();
  const hash2 = ex.pad_design_hash();
  if (hash1 !== hash2) {
    problems.push(`the design hash is not deterministic: ${hash1} vs ${hash2}`);
  }
  if (hash1 === 0) {
    problems.push("the design hash is the zero value; it must not be");
  }

  if (problems.length) throw new Error("pad boundary disagrees with the claim: " + problems.join("; "));
  return `under-specified fails at ${underT} s, specified survives, inlet ${underFrac} vs ${specFrac}, ` +
    `ghost twin δ=${gtDiv}, arrow=${arrowWhich}, design hash=${hash1}, reproducible`;
}

/** `--selftest`: prove `verifyPad` accepts a correct boundary and rejects the ways it can be wrong. */
function selftest() {
  // A shim, not the kernel: it has the properties `verifyPad` checks -- the under-specified plan fails,
  // the specified one survives, a non-finite fill rate is refused, the ghost twin divergence is
  // actual minus expected, the one arrow names the larger sensitivity, and the design hash is
  // deterministic. It is *honest shim behaviour* rather than a pretend network, which is the distinction
  // that matters here.
  const good = {
    pad_failure_time: (chill, bleeds, fill, t) => {
      if (!Number.isFinite(fill)) return NaN;
      return chill < 60 && bleeds === 0 ? 0.0 : -1.0;
    },
    pad_inlet_vapour_fraction: (chill, bleeds) => (chill < 60 && bleeds === 0 ? 1.0 : 0.0),
    pad_is_wreck: (chill, bleeds, fill) => (Number.isFinite(fill) && chill < 60 && bleeds === 0 ? 1 : 0),
    pad_vapour_pressure_kpa: (t) => (Math.abs(t - 90.188) < 1e-9 ? 101.325 : 200.0),
    // P2: the ghost twin. The under-specified plan's actual is 1.0; the divergence is actual - expected.
    pad_ghost_twin_actual: (chill, bleeds, expected, fill, t) => {
      if (!Number.isFinite(expected) || !Number.isFinite(fill)) return NaN;
      return chill < 60 && bleeds === 0 ? 1.0 : 0.0;
    },
    pad_ghost_twin_divergence: (chill, bleeds, expected, fill, t) => {
      if (!Number.isFinite(expected) || !Number.isFinite(fill)) return NaN;
      const actual = chill < 60 && bleeds === 0 ? 1.0 : 0.0;
      return actual - expected;
    },
    // P2: the one arrow. The interior-flash plan (chill>=60) attributes to bleeds (sensitivity 0.3 > 0.01);
    // the rail plan (chill<60) produces no arrow.
    pad_attribution_which: (chill, bleeds, expected, fill, t) => {
      if (!Number.isFinite(expected) || !Number.isFinite(fill)) return 0;
      if (chill >= 60) return 2; // bleeds
      return 0; // rail: no arrow
    },
    pad_attribution_sensitivity_chill: () => 0.01,
    pad_attribution_sensitivity_bleeds: () => 0.3,
    // P2: the design hash is deterministic and non-zero.
    pad_design_hash: () => 1234567890123456789,
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

  // P2: a ghost twin whose divergence is NOT actual minus expected -- the page would be reading a
  // prediction error that is not the gap between the mark and the needle.
  const wrongDiv = {
    ...good,
    pad_ghost_twin_divergence: (chill, bleeds, expected, fill, t) => 0.0,
  };
  try {
    verifyPad(wrongDiv);
    results.push(["a ghost twin divergence that is not actual-expected is rejected", false]);
  } catch {
    results.push(["a ghost twin divergence that is not actual-expected is rejected", true]);
  }

  // P2: a one arrow that names the wrong input -- the larger sensitivity is bleeds, but the arrow says
  // chill time. The attribution would point the player at the wrong lever.
  const wrongArrow = { ...good, pad_attribution_which: () => 1 };
  try {
    verifyPad(wrongArrow);
    results.push(["a one arrow that names the wrong input is rejected", false]);
  } catch {
    results.push(["a one arrow that names the wrong input is rejected", true]);
  }

  // P2: a rail plan that produces an arrow. The needle is pinned at the rail; an arrow there points at
  // nothing and is decoration, not attribution.
  const railArrow = { ...good, pad_attribution_which: () => 2 };
  try {
    verifyPad(railArrow);
    results.push(["a rail plan that produces an arrow is rejected", false]);
  } catch {
    results.push(["a rail plan that produces an arrow is rejected", true]);
  }

  // P2: a non-deterministic design hash. An Akashi entry stamped with a changing hash cannot be told
  // apart from one produced by a different kernel version.
  const unstableHash = { ...good, pad_design_hash: () => Math.random() * 1e18 | 0 };
  try {
    verifyPad(unstableHash);
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
