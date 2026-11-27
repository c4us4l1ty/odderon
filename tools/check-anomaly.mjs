#!/usr/bin/env node
/**
 * The gate that makes the Anomaly Drawer and the logic rack a real, kernel-verifiable failure-design
 * and session layer rather than a page-kept tally.
 *
 * `web/index.html` opens an anomaly when a mission diverges, caps the drawer at five, resolves an
 * anomaly through a duel-style Elo, and compiles logic-rack rules into the deterministic tick loop
 * (`GAME-DESIGN-BIBLE.md` §7.3, §7.4, §5.5, §4, §9.2). The gate is the bible's own rules made
 * mechanical:
 *
 *   1. THE DRAWER CAP IS THE KERNEL'S. §7.3 -- "at most five." The page must call the kernel's
 *      `drawer_can_open` to decide whether a sixth anomaly can be opened, and must NOT hardcode the
 *      cap as a page-side rule. The failure mode is "someone re-adds a page-side `open.length < 5`".
 *
 *   2. THE ANOMALY IS THE KERNEL'S. §4 / §7.3 -- the anomaly is derived from a mission's divergence.
 *      The page must call the kernel's `anomaly_from_mission` and must NOT compute the divergence or
 *      the anomaly's depth/elo of its own.
 *
 *   3. THE E_an RATING IS THE KERNEL'S ELO. §4 -- "duel-style Elo per mission; weighted by how deep
 *      the fault was hidden and how little telemetry you were given." The page must call the kernel's
 *      `elo_update` and must NOT compute an Elo of its own.
 *
 *   4. THE RACK RULE IS THE KERNEL'S. §5.5 -- `IF sensor > threshold THEN action` compiles into the
 *      deterministic tick loop. The page must call the kernel's `rack_compile` and `rack_fires` and
 *      must NOT evaluate the predicate of its own.
 *
 *   5. THE DRAWER PERSISTS ACROSS SESSIONS. §9.2 -- "the save is an input log, so it always exists."
 *      The drawer state must be written to localStorage and re-read on load.
 *
 *   6. THE KERNEL IS PROPER. The anomaly derivation, the Elo, and the rack are pure functions with
 *      NaN refusals, and the transcendentals (fabs, pow) come from odderon_dmath (R1).
 *
 * Usage: node tools/check-anomaly.mjs            (static: no browser, no wasm needed)
 *        node tools/check-anomaly.mjs --selftest  (extract the page's drawer, run it on a shim)
 *        node tools/check-anomaly.mjs --wasm dist/odderon_dmath_wasm.wasm
 *                                                     (also check the exports the drawer depends on)
 */

import { readFile } from "node:fs/promises";
import { readFileSync } from "node:fs";

const PAGE = new URL("../web/index.html", import.meta.url);
const KERNEL = new URL("../crates/odderon-astro/src/anomaly.rs", import.meta.url);

/** Strip comments so documentation about the drawer does not trip the checks that hunt for locals. */
function liveSource(html) {
  const script = html.match(/<script type="module">([\s\S]*?)<\/script>/);
  if (!script) throw new Error("no module script found in web/index.html");
  return script[1]
    .replace(/\/\*[\s\S]*?\*\//g, "")
    .replace(/(^|[^:])\/\/.*$/gm, "$1");
}

/** Strip Rust comments. */
function liveRust(rs) {
  return rs.replace(/\/\*[\s\S]*?\*\//g, "").replace(/(^|[^:])\/\/.*$/gm, "$1");
}

/** Extract a function's body from the live source. */
function fnBody(src, name) {
  const re = new RegExp("function " + name + "\\s*\\([^)]*\\)\\s*\\{([\\s\\S]*?)\\n\\}");
  const m = src.match(re);
  if (!m) throw new Error("could not find function " + name);
  return m[1];
}

const checks = [];
function check(name, fn, isKernel) {
  checks.push({ name, fn, isKernel });
}

check("the drawer cap is the kernel's, not a page-side rule", (src) => {
  const ledger = fnBody(src, "anomalyLedger");
  const open = fnBody(src, "openAnomaly");
  // The page must call the kernel's cap predicate. §7.3 -- "at most five" is the kernel's rule.
  if (!ledger.includes("drawer_can_open")) throw new Error("anomalyLedger does not call the kernel's drawer_can_open");
  if (!open.includes("drawer_can_open")) throw new Error("openAnomaly does not call the kernel's drawer_can_open before opening");
  // The cap constant in the page's render is a DISPLAY label, not the rule -- but the rule itself
  // must not be a page-side comparison. A page-side `open.length < 5` as the cap decision is the
  // defect: the kernel's cap is bypassed.
  if (/open\.length\s*<\s*5/.test(open)) throw new Error("openAnomaly decides the cap with a page-side 'open.length < 5' -- the kernel's drawer_can_open is bypassed");
  if (/openCount\s*<\s*5/.test(ledger)) throw new Error("anomalyLedger decides the cap with a page-side 'openCount < 5'");
  // The cap predicate call must be ACTIVE. A call disabled behind `false &&` never executes.
  if (/false\s*&&\s*k\.drawer_can_open/.test(ledger)) throw new Error("anomalyLedger disables the drawer_can_open call with 'false &&'");
  if (/false\s*&&\s*k\.drawer_can_open/.test(open)) throw new Error("openAnomaly disables the drawer_can_open call with 'false &&'");
});

check("the anomaly is the kernel's, not a page-side derivation", (src) => {
  const ledger = fnBody(src, "anomalyLedger");
  const open = fnBody(src, "openAnomaly");
  // The page must call the kernel's anomaly derivation.
  if (!ledger.includes("anomaly_from_mission")) throw new Error("anomalyLedger does not call the kernel's anomaly_from_mission");
  if (!open.includes("anomaly_from_mission")) throw new Error("openAnomaly does not call the kernel's anomaly_from_mission");
  // The page must NOT compute the divergence of its own. The divergence is |p - occurred|; a
  // page-side computation of it (e.g. `Math.abs(p - met)` or `p - met`) means the anomaly is
  // derived on the page, not the kernel.
  if (/Math\.abs\s*\(\s*\w+\.params\.p\s*-\s*met\s*\)/.test(ledger)) throw new Error("anomalyLedger computes the divergence on the page -- the kernel's anomaly_from_mission is bypassed");
  if (/Math\.abs\s*\(\s*p\s*-\s*met\s*\)/.test(open)) throw new Error("openAnomaly computes the divergence on the page");
  // The anomaly's depth/elo must be read back from the kernel's buffer, not computed on the page.
  if (!/dv\.getFloat64\(OFF \+ 8, true\)/.test(ledger)) throw new Error("anomalyLedger does not read the kernel's depth back");
  if (!/dv\.getFloat64\(OFF \+ 24, true\)/.test(ledger)) throw new Error("anomalyLedger does not read the kernel's elo back");
});

check("the E_an rating is the kernel's Elo, not a page-side formula", (src) => {
  const resolve = fnBody(src, "resolveAnomaly");
  // The page must call the kernel's Elo update.
  if (!resolve.includes("elo_update")) throw new Error("resolveAnomaly does not call the kernel's elo_update");
  // The page must NOT compute an Elo of its own. The Elo formula is 1/(1+10^((opp-rating)/400))
  // with a weighted K; a page-side `Math.pow(10, ...)` or a hand-rolled delta is the defect.
  if (/Math\.pow\s*\(\s*10/.test(resolve)) throw new Error("resolveAnomaly computes an Elo on the page -- the kernel's elo_update is bypassed");
  if (/rating\s*\+\s*.*\*\s*\(/.test(resolve) && !resolve.includes("elo_update")) {
    throw new Error("resolveAnomaly computes an Elo delta on the page");
  }
  // The elo_update call must be ACTIVE.
  if (/false\s*&&\s*k\.elo_update/.test(resolve)) throw new Error("resolveAnomaly disables the elo_update call with 'false &&'");
  // The new rating is read from the kernel's return, not accumulated on the page.
  if (!/Number\.isFinite\(newRating\)/.test(resolve)) throw new Error("resolveAnomaly does not check the kernel's returned rating");
});

check("the rack rule is the kernel's, not a page-side predicate", (src) => {
  const compile = fnBody(src, "rackCompile");
  const fire = fnBody(src, "rackFire");
  // The page must call the kernel's compile and fire.
  if (!compile.includes("rack_compile")) throw new Error("rackCompile does not call the kernel's rack_compile");
  if (!fire.includes("rack_fires")) throw new Error("rackFire does not call the kernel's rack_fires");
  // The page must NOT evaluate the predicate of its own. A page-side comparison (`reading > threshold`
  // or `reading < threshold` etc.) means the rule fires on the page, not the kernel.
  const predicate = /reading\s*(>|<|>=|<=|===|!==|==|!=)\s*(rule\.threshold|threshold|r\.threshold)/;
  if (predicate.test(fire)) throw new Error("rackFire evaluates the predicate on the page -- the kernel's rack_fires is bypassed");
  // The compile call must be ACTIVE.
  if (/false\s*&&\s*k\.rack_compile/.test(compile)) throw new Error("rackCompile disables the rack_compile call with 'false &&'");
  if (/false\s*&&\s*k\.rack_fires/.test(fire)) throw new Error("rackFire disables the rack_fires call with 'false &&'");
});

check("the drawer persists across sessions", (src) => {
  // §9.2 -- "the save is an input log, so it always exists." The drawer state must be written to
  // localStorage and re-read on load.
  if (!/localStorage\.setItem/.test(src)) throw new Error("the drawer state is not written to localStorage -- §9.2 requires the save to be an input log");
  if (!/localStorage\.getItem/.test(src)) throw new Error("the drawer state is not read from localStorage on load");
  if (!/loadAnomalyState/.test(src)) throw new Error("there is no loadAnomalyState to re-read the persisted drawer");
  if (!/saveAnomalyState/.test(src)) throw new Error("there is no saveAnomalyState to persist the drawer");
});

check("the kernel anomaly module is proper", (rust) => {
  // The cap is pinned at five.
  if (!/pub const DRAWER_CAP:\s*usize\s*=\s*5/.test(rust)) throw new Error("DRAWER_CAP is not pinned at 5");
  // The divergence threshold is pinned.
  if (!/pub const DIVERGENCE_THRESHOLD/.test(rust)) throw new Error("DIVERGENCE_THRESHOLD is not pinned");
  // The anomaly derivation, the Elo, and the rack are all present.
  if (!/pub fn anomaly_from_mission/.test(rust)) throw new Error("anomaly_from_mission is missing");
  if (!/pub fn drawer_can_open/.test(rust)) throw new Error("drawer_can_open is missing");
  if (!/pub fn elo_expected/.test(rust)) throw new Error("elo_expected is missing");
  if (!/pub fn elo_update/.test(rust)) throw new Error("elo_update is missing");
  if (!/pub fn rack_compile/.test(rust)) throw new Error("rack_compile is missing");
  if (!/pub fn rack_fires/.test(rust)) throw new Error("rack_fires is missing");
  // The transcendentals come from odderon_dmath (R1), not host float methods.
  if (!/use odderon_dmath::\{[^}]*fabs/.test(rust)) throw new Error("fabs is not imported from odderon_dmath");
  if (!/use odderon_dmath::\{[^}]*pow/.test(rust)) throw new Error("pow is not imported from odderon_dmath");
  // Refusals: the anomaly derivation returns None on invalid input, the Elo returns NaN.
  if (!/return None/.test(rust)) throw new Error("the anomaly derivation does not refuse with None");
  if (!/f64::NAN/.test(rust)) throw new Error("the Elo does not refuse with NaN");
  // The Elo is duel-style: the expected score uses 10^((opponent-rating)/400).
  if (!/pow\(10\.0,\s*exponent\)/.test(rust)) throw new Error("the Elo expected score does not use the duel formula 10^((opp-rating)/400)");
  // The rack operator set is complete (all six comparisons).
  for (const op of ["Gt", "Lt", "Ge", "Le", "Eq", "Ne"]) {
    if (!new RegExp("Op::" + op).test(rust)) throw new Error("the rack is missing the " + op + " operator");
  }
}, true);

/**
 * `--selftest`: prove the page's drawer opens an anomaly from a diverged mission, refuses a sixth
 * when the drawer is full, resolves one through the kernel's Elo, and compiles and fires a rack rule
 * -- all against a shim kernel that mimics the ABI contract. The real physics is checked in --wasm.
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

  // A shim wasm memory + kernel. The shim derives a real anomaly in JS (using Math.abs for the
  // divergence) and writes it into the caller's buffer exactly as the ABI contract specifies, so the
  // page's readback and layout are exercised. The physics itself is the kernel's, checked in --wasm.
  const mem = {
    buffer: new ArrayBuffer(1 << 20),
    grow(pages) {
      const bigger = new ArrayBuffer(this.buffer.byteLength + pages * 65536);
      new Uint8Array(bigger).set(new Uint8Array(this.buffer));
      this.buffer = bigger;
    },
  };
  const shimKernel = {
    memory: mem,
    // A diverged mission (divergence >= 0.05) opens an anomaly; below the threshold, a refusal (0).
    anomaly_from_mission(sourceId, p, occurred, risk, window, outOff) {
      const divergence = Math.abs(p - occurred);
      if (divergence < 0.05) return 0;
      const dv = new DataView(mem.buffer);
      const depth = Math.min(Math.max(divergence + risk * 0.5, 0), 1);
      const telemetry = Math.min(Math.max(window / 30, 0), 1);
      const elo = 1000 + (depth + (1 - telemetry)) * 200;
      dv.setFloat64(outOff, sourceId, true);
      dv.setFloat64(outOff + 8, depth, true);
      dv.setFloat64(outOff + 16, telemetry, true);
      dv.setFloat64(outOff + 24, elo, true);
      dv.setFloat64(outOff + 32, 0, true);
      return 5;
    },
    drawer_can_open(count) { return count < 5 ? 1 : 0; },
    elo_expected(rating, opponent) { return 1 / (1 + Math.pow(10, (opponent - rating) / 400)); },
    elo_update(rating, opponent, outcome, depth, telemetry) {
      const expected = 1 / (1 + Math.pow(10, (opponent - rating) / 400));
      const k = 32 * (1 + depth) * (1 + (1 - telemetry));
      return rating + k * (outcome - expected);
    },
    rack_compile(sensor, op, threshold, action, outOff) {
      if (op > 5) return 0;
      if (!Number.isFinite(threshold)) return 0;
      const dv = new DataView(mem.buffer);
      dv.setFloat64(outOff, sensor, true);
      dv.setFloat64(outOff + 8, op, true);
      dv.setFloat64(outOff + 16, threshold, true);
      dv.setFloat64(outOff + 24, action, true);
      return 4;
    },
    rack_fires(ruleOff, reading) {
      const dv = new DataView(mem.buffer);
      const op = dv.getFloat64(ruleOff + 8, true);
      const threshold = dv.getFloat64(ruleOff + 16, true);
      if (!Number.isFinite(reading)) return 0;
      const ops = [(a, b) => a > b, (a, b) => a < b, (a, b) => a >= b, (a, b) => a <= b, (a, b) => a === b, (a, b) => a !== b];
      return ops[Math.round(op)](reading, threshold) ? 1 : 0;
    },
    anomaly_design_hash: () => 999n,
  };

  const archive = [];
  const anomalyState = { elo: 1000.0, open: [], rack: [] };
  const fnSource = [
    // The page's persistence and render helpers are stubbed: saveAnomalyState writes to localStorage
    // (not available in the sandbox), loadAnomalyState reads it back (no-op on first run), and the
    // render functions touch the DOM ($ is already stubbed). The logic under test -- open, resolve,
    // compile, fire -- is the page's own, calling the kernel's exports.
    "function saveAnomalyState() {}",
    "function loadAnomalyState() {}",
    "function renderAnomaly() {}",
    "function renderRack() {}",
    extract("anomalyLedger"),
    extract("openAnomaly"),
    extract("resolveAnomaly"),
    extract("rackCompile"),
    extract("rackFire"),
  ].join("\n");

  // A $() stub for the page's DOM reads. The anomaly functions do not read the DOM; the rack
  // functions read the panel inputs, so this stub returns the values the rack tests need.
  const $vals = (id) => ({
    textContent: '', innerHTML: '',
    value: id === 'rack-threshold' ? '92' : id === 'rack-action' ? '3' : id === 'rack-reading' ? '95' : '0',
    addEventListener: () => {},
  });
  const sandbox = { kernelState: { module: shimKernel }, archive, anomalyState, $: $vals };
  const fn = new Function(...Object.keys(sandbox), fnSource + "\nreturn { anomalyLedger, openAnomaly, resolveAnomaly, rackCompile, rackFire, anomalyState };");
  const { anomalyLedger, openAnomaly, resolveAnomaly, rackCompile, rackFire } = fn(...Object.values(sandbox));

  const results = [];

  // No missions: the drawer is valid but empty.
  const empty = anomalyLedger();
  results.push(["the drawer reads the kernel", empty.valid === true && empty.source === "wasm kernel"]);
  results.push(["an empty archive opens no anomalies", empty.openCount === 0]);
  results.push(["the drawer can open when empty", empty.canOpen === true]);

  // A diverged mission in the archive: the drawer derives the anomaly from it.
  archive.push({ id: 1, type: "mission", params: { dv: 7.8, p: 0.8, risk: 0, window: 30 }, readings: { dv: 7.0, score: 0.5 } });
  const led1 = anomalyLedger();
  results.push(["a diverged mission is an anomaly", led1.anomalies.length === 1]);
  results.push(["the anomaly's source is the mission", led1.anomalies[0].source_id === 1]);

  // Open it: the drawer takes it (not full).
  const opened = openAnomaly();
  results.push(["a diverged mission opens an anomaly", opened !== null && opened.source_id === 1]);
  results.push(["the opened anomaly is in the drawer", anomalyState.open.includes(1)]);
  results.push(["the drawer reports one open", anomalyLedger().openCount === 1]);

  // Open four more: the drawer fills to the cap.
  for (let i = 2; i <= 5; i++) {
    archive.push({ id: i, type: "mission", params: { dv: 7.8, p: 0.9, risk: 0, window: 30 }, readings: { dv: 7.0, score: 0.5 } });
    openAnomaly();
  }
  results.push(["the drawer fills to the cap", anomalyLedger().openCount === 5]);
  results.push(["the full drawer refuses to open", anomalyLedger().canOpen === false]);

  // A sixth diverged mission: the drawer is full, so it refuses.
  archive.push({ id: 6, type: "mission", params: { dv: 7.8, p: 0.95, risk: 0, window: 30 }, readings: { dv: 7.0, score: 0.5 } });
  const refused = openAnomaly();
  results.push(["a sixth anomaly is refused when the drawer is full", refused === null]);
  results.push(["the refusal leaves the drawer at the cap", anomalyLedger().openCount === 5]);

  // Resolve one: the E_an rating moves through the kernel's Elo, and the anomaly closes.
  const eloBefore = anomalyState.elo;
  const resolved = resolveAnomaly(1);
  results.push(["resolving moves the E_an rating", resolved !== null && resolved !== eloBefore]);
  results.push(["a correct resolution raises the rating", resolved > eloBefore]);
  results.push(["the resolved anomaly leaves the drawer", !anomalyState.open.includes(1)]);
  results.push(["the drawer can open again after a resolve", anomalyLedger().canOpen === true]);

  // The rack: compile a rule and fire it. Reuse the same sandbox (same $ stub, same state).
  const fn2 = new Function(...Object.keys(sandbox), fnSource + "\nreturn { rackCompile, rackFire, anomalyState };");
  const { rackCompile: rc, rackFire: rf } = fn2(...Object.values(sandbox));
  const rule = rc();
  results.push(["the rack compiles a rule", rule !== null && rule.threshold === 92]);
  results.push(["the compiled rule is stored", anomalyState.rack.length === 1]);
  const fired = rf();
  results.push(["the rack fires a rule above its threshold", fired.length === 1 && fired[0] === 3]);

  let bad = 0;
  for (const [name, ok] of results) {
    console.log(`  ${ok ? "ok  " : "FAIL"} ${name}`);
    if (!ok) bad++;
  }
  return bad;
}

const html = await readFile(PAGE, "utf8");
const src = liveSource(html);
const rust = liveRust(readFileSync(KERNEL, "utf8"));

let failed = 0;
for (const { name, fn, isKernel } of checks) {
  try {
    fn(isKernel ? rust : src);
    console.log(`  ok   ${name}`);
  } catch (e) {
    failed++;
    console.log(`  FAIL ${name}`);
    console.log(`       ${e.message}`);
  }
}

if (process.argv.includes("--selftest")) {
  // The selftest is wrapped so a throw inside it is reported, not crashing the whole gate.
  let bad = 0;
  try {
    bad = selftest();
  } catch (e) {
    bad = 1;
    console.log("  FAIL self-test threw: " + e.message);
  }
  console.log("");
  console.log(bad ? `${bad} self-test(s) FAILED` : "self-test passed: the drawer opens, caps, resolves, and the rack compiles and fires");
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
  console.log(`${failed} anomaly check(s) FAILED`);
  process.exit(1);
}
console.log(`all ${checks.length} anomaly checks passed`);

/**
 * Load a built module and verify the exports the drawer and the rack depend on.
 *
 * The boundary is physical: a diverged mission derives an anomaly with the right depth, telemetry,
 * and Elo; a non-diverged mission refuses; the drawer caps at five; the Elo expected score matches
 * the duel formula; a correct resolution raises the rating and a wrong one lowers it; the rack
 * compiles valid rules and refuses invalid ones; every operator fires correctly; and the design hash
 * is stable.
 */
async function runWasm(path) {
  const bytes = await readFile(path);
  const mod = await WebAssembly.compile(bytes);
  const inst = await WebAssembly.instantiate(mod, {});
  return verifyExports(inst.exports);
}

function verifyExports(ex) {
  const need = ["anomaly_from_mission", "drawer_can_open", "elo_expected", "elo_update", "rack_compile", "rack_fires", "anomaly_design_hash"];
  const missing = need.filter((n) => typeof ex[n] !== "function");
  if (missing.length) throw new Error("module is missing: " + missing.join(", "));
  if (!ex.memory) throw new Error("the module does not export memory for the drawer's buffers");

  const problems = [];
  const mem = ex.memory;
  const NEED_BYTES = 1 << 20;
  while (mem.buffer.byteLength < NEED_BYTES + (1 << 20)) {
    mem.grow(256);
  }
  const dv = new DataView(mem.buffer);
  const OFF = NEED_BYTES;

  // A diverged mission: predicted 0.8, occurred 0 -> divergence 0.8, opens.
  const rc = ex.anomaly_from_mission(7, 0.8, 0, 0.0, 30.0, OFF);
  if (rc !== 5) problems.push("anomaly_from_mission refused a diverged mission");
  const sourceId = Math.round(dv.getFloat64(OFF, true));
  const depth = dv.getFloat64(OFF + 8, true);
  const telemetry = dv.getFloat64(OFF + 16, true);
  const elo = dv.getFloat64(OFF + 24, true);
  if (sourceId !== 7) problems.push(`source_id ${sourceId} != 7`);
  if (Math.abs(depth - 0.8) > 1e-9) problems.push(`depth ${depth} != 0.8`);
  if (Math.abs(telemetry - 1.0) > 1e-9) problems.push(`telemetry ${telemetry} != 1.0`);
  if (Math.abs(elo - 1160.0) > 1e-9) problems.push(`elo ${elo} != 1160`);

  // A non-diverged mission: predicted 0.96, occurred 1 -> divergence 0.04 < 0.05, refuses.
  if (ex.anomaly_from_mission(1, 0.96, 1, 0.0, 30.0, OFF) !== 0) problems.push("a non-diverged mission did not refuse");

  // An invalid mission refuses.
  if (ex.anomaly_from_mission(1, 1.5, 0, 0.0, 30.0, OFF) !== 0) problems.push("an invalid p did not refuse");

  // The drawer cap.
  if (ex.drawer_can_open(4) !== 1) problems.push("the drawer should open at count 4");
  if (ex.drawer_can_open(5) !== 0) problems.push("the drawer should be full at count 5");

  // The Elo expected score: equal ratings score 0.5.
  if (Math.abs(ex.elo_expected(1000.0, 1000.0) - 0.5) > 1e-9) problems.push("elo_expected(1000,1000) != 0.5");
  // A 200-point edge: 10^(-0.5) = 1/sqrt(10); expected = sqrt(10)/(sqrt(10)+1).
  const sqrt10 = Math.sqrt(10);
  if (Math.abs(ex.elo_expected(1200.0, 1000.0) - sqrt10 / (sqrt10 + 1.0)) > 1e-9) problems.push("elo_expected(1200,1000) != the duel formula");

  // The Elo update: a correct resolution raises the rating, a wrong one lowers it.
  const up = ex.elo_update(1000.0, 1160.0, 1, 0.8, 1.0);
  if (!(up > 1000.0)) problems.push("a correct resolution did not raise the rating");
  const down = ex.elo_update(1000.0, 1160.0, 0, 0.8, 1.0);
  if (!(down < 1000.0)) problems.push("a wrong resolution did not lower the rating");
  // The weighting: a deeper, scarcer anomaly moves the rating more.
  const easyDelta = ex.elo_update(1000.0, 1100.0, 1, 0.6, 1.0) - 1000.0;
  const hardDelta = ex.elo_update(1000.0, 1100.0, 1, 1.0, 0.1) - 1000.0;
  if (!(hardDelta > easyDelta)) problems.push("the weighting does not reward a deeper, scarcer anomaly");
  // A refusal: invalid depth.
  if (!Number.isNaN(ex.elo_update(1000.0, 1160.0, 1, 1.5, 1.0))) problems.push("elo_update did not refuse an invalid depth");

  // The rack: compile and fire.
  const rrc = ex.rack_compile(0, 0, 92.0, 3, OFF);
  if (rrc !== 4) problems.push("rack_compile refused a valid rule");
  if (Math.abs(dv.getFloat64(OFF, true) - 0) > 1e-9) problems.push("the rule's sensor is wrong");
  if (Math.abs(dv.getFloat64(OFF + 16, true) - 92.0) > 1e-9) problems.push("the rule's threshold is wrong");
  if (ex.rack_fires(OFF, 95.0) !== 1) problems.push("a reading above the threshold did not fire Gt");
  if (ex.rack_fires(OFF, 90.0) !== 0) problems.push("a reading below the threshold fired Gt");
  // Every operator fires correctly.
  const ops = [[0, 95, 1], [0, 90, 0], [1, 90, 1], [1, 95, 0], [2, 92, 1], [3, 92, 1], [4, 92, 1], [4, 93, 0], [5, 93, 1]];
  for (const [op, reading, expect] of ops) {
    ex.rack_compile(0, op, 92.0, 1, OFF);
    if (ex.rack_fires(OFF, reading) !== expect) problems.push(`operator ${op} fired wrong on reading ${reading}`);
  }
  // An invalid op refuses.
  if (ex.rack_compile(0, 9, 92.0, 3, OFF) !== 0) problems.push("rack_compile did not refuse an invalid op");
  // A NaN reading does not fire.
  ex.rack_compile(0, 0, 92.0, 1, OFF);
  if (ex.rack_fires(OFF, NaN) !== 0) problems.push("a NaN reading fired the rack");

  // The design hash is stable and a u64 (BigInt).
  const h = ex.anomaly_design_hash();
  if (typeof h !== "bigint") problems.push("anomaly_design_hash is not a u64 (BigInt)");
  if (h !== ex.anomaly_design_hash()) problems.push("the anomaly design hash is not stable");

  if (problems.length) throw new Error("boundary disagrees with physics: " + problems.join("; "));
  return "a diverged mission opens, the drawer caps at five, the Elo is duel-weighted, the rack compiles and fires, refusals NaN, design-hash stable";
}
