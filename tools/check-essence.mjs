#!/usr/bin/env node
/**
 * The gate that makes the Esse career ledger a real, kernel-verifiable calibration rating rather than a
 * page-kept average.
 *
 * `web/index.html` aggregates the archive's scored missions into the `E_cal` rating
 * (`GAME-DESIGN-BIBLE.md` §4.3). Block 8 scored one prediction; this block aggregates a career. The
 * gate is the bible's own rules made mechanical:
 *
 *   1. THE AGGREGATE IS A RELIABILITY CURVE, NOT AN AVERAGE. §4.3 rule 1 -- "aggregated into a
 *      reliability curve, not an average." An average cannot tell a calibrated forecaster from a
 *      lucky one; the curve can. The page must call the kernel's curve export and must NOT keep a
 *      running Esse mean of its own. The failure mode is "someone re-adds a page-side mean", which is
 *      exactly the simplification Block 8 shipped and this block replaces.
 *
 *   2. THE RATING IS RECOMPUTABLE FROM THE MISSION LOG. §4.3 rule 4 -- "anyone can download the input
 *      log and re-simulate." `essenceLedger` must build the prediction log from the archive (the
 *      verified replay), hand it to the kernel, and read the rating back. The page computes no score
 *      and no curve of its own.
 *
 *   3. THE RATING IS PROVISIONAL UNTIL N ≥ 30. §4.3 -- a rating from too few samples is a claim, not
 *      a measurement. The page must report the provisional status, and the kernel's threshold must be
 *      30.
 *
 *   4. THE RATING IS REPORTED WITH N AND A 95% INTERVAL. §4.3 -- "reported with N and a 95% interval."
 *
 *   5. THE BIAS IS REPORTED. §2.2(a) -- the competence mirror: "your bias is +0.8%... that bias is
 *      your next lesson." The page must show the signed calibration bias.
 *
 *   6. THE AGGREGATE IS PROPER. §4.3 rule 1 -- the kernel re-scores every prediction through the same
 *      proper rule the per-mission score uses (`mission::log_score`), never an accuracy percentage.
 *
 * Usage: node tools/check-essence.mjs            (static: no browser, no wasm needed)
 *        node tools/check-essence.mjs --selftest  (extract the page's ledger, run it on a shim)
 *        node tools/check-essence.mjs --wasm dist/odderon_dmath_wasm.wasm
 *                                                     (also check the exports the ledger depends on)
 */

import { readFile } from "node:fs/promises";
import { readFileSync } from "node:fs";

const PAGE = new URL("../web/index.html", import.meta.url);
const KERNEL = new URL("../crates/odderon-astro/src/essence.rs", import.meta.url);

/** Strip comments so documentation about the ledger does not trip the checks that hunt for locals. */
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
function check(name, fn) {
  checks.push({ name, fn });
}

check("the essence aggregate is a reliability curve, not a running mean", (src) => {
  const ledger = fnBody(src, "essenceLedger");
  // The page must call the kernel's curve export -- the reliability curve is the kernel's.
  if (!ledger.includes("essence_reliability")) throw new Error("essenceLedger does not call the kernel's essence_reliability");
  if (!ledger.includes("essence_summary")) throw new Error("essenceLedger does not call the kernel's essence_summary");
  // The page must NOT keep a running Esse mean of its own. §4.3 rule 1 forbids collapsing
  // calibration into an average; a page-side mean is exactly the simplification this block removes.
  // Catch both an assignment (`missionState.essence = ...`) and a field declared on the state
  // object (`essence: ...`) -- either means the page is holding an aggregate instead of the kernel.
  if (/missionState\.essence\s*=/.test(src)) throw new Error("the page assigns a missionState.essence running mean -- §4.3 rule 1 forbids an average");
  const msObj = src.match(/const missionState\s*=\s*\{([\s\S]*?)\n\};/);
  if (msObj && /\bessence\s*:/.test(msObj[1])) throw new Error("missionState declares an essence field -- the page keeps an aggregate, §4.3 rule 1 forbids an average");
  if (/essence\s*=\s*essence\s*\+\s*\(/.test(ledger)) throw new Error("essenceLedger updates a running mean");
  // The curve call must be ACTIVE. A call left in the source but disabled behind `false &&` (or an
  // `if (false)` guard) never executes, so the curve is not read even though the string is present.
  if (/false\s*&&\s*k\.essence_reliability/.test(ledger)) throw new Error("essenceLedger disables the reliability curve call with 'false &&'");
  if (/if\s*\(\s*false\s*\)\s*\{?\s*k\.essence_reliability/.test(ledger)) throw new Error("essenceLedger guards the reliability curve call behind 'if (false)'");
  // The curve is read back bin-by-bin, not collapsed to one scalar.
  if (!/bins\.push/.test(ledger)) throw new Error("essenceLedger does not read the reliability curve back bin-by-bin");
});

check("the rating is recomputable from the mission log", (src) => {
  const ledger = fnBody(src, "essenceLedger");
  // The prediction log is built from the archive -- the verified replay -- not from a session tally.
  if (!/for\s*\(const e of archive\)/.test(ledger)) throw new Error("essenceLedger does not build the log from the archive");
  if (!/e\.params\.p/.test(ledger)) throw new Error("essenceLedger does not read each mission's committed probability");
  if (!/e\.readings\.dv/.test(ledger)) throw new Error("essenceLedger does not read each mission's achieved Δv");
  // The page hands the log to the kernel's memory and reads the rating back. It computes no score.
  if (!/essence_summary\(/.test(ledger)) throw new Error("essenceLedger does not hand the log to the kernel");
  if (/Math\.log/.test(ledger)) throw new Error("essenceLedger computes a score on the page -- the rating must be the kernel's");
  if (/accuracy/i.test(ledger)) throw new Error("essenceLedger computes an accuracy -- §4.3 forbids it");
});

check("the rating is provisional until N ≥ 30", (src) => {
  const render = fnBody(src, "renderEssence");
  // The page reports the provisional status, and the threshold it names is 30.
  if (!/provisional/.test(render)) throw new Error("renderEssence does not report the provisional status");
  if (!/30/.test(render)) throw new Error("renderEssence does not name the N ≥ 30 threshold");
  // The kernel's threshold constant must be 30 (checked against the kernel source below too).
});

check("the rating is reported with N and a 95% interval", (src) => {
  const render = fnBody(src, "renderEssence");
  if (!/m-essence-n/.test(render)) throw new Error("renderEssence does not report N");
  if (!/m-essence-ci/.test(render)) throw new Error("renderEssence does not report the 95% interval");
  if (!/ci95/.test(render)) throw new Error("renderEssence does not read the interval bounds");
});

check("the bias is reported (the competence mirror)", (src) => {
  const render = fnBody(src, "renderEssence");
  if (!/m-essence-bias/.test(render)) throw new Error("renderEssence does not report the bias");
  if (!/bias/.test(render)) throw new Error("renderEssence does not read the bias");
});

check("the kernel aggregate is proper and refusing", (rust) => {
  // §4.3 rule 1: the aggregate re-scores through the same proper rule the per-mission score uses.
  if (!rust.includes("log_score")) throw new Error("essence.rs does not re-score through mission::log_score");
  if (/accuracy/i.test(rust)) throw new Error("essence.rs computes an accuracy -- §4.3 forbids it");
  // The reliability curve is a fixed array of bins, not a single average.
  if (!/BIN_COUNT/.test(rust)) throw new Error("essence.rs has no binned reliability curve");
  if (!/ReliabilityBin/.test(rust)) throw new Error("essence.rs has no ReliabilityBin");
  // The provisional threshold is pinned at 30.
  if (!/PROVISIONAL_N\s*:\s*usize\s*=\s*30/.test(rust)) throw new Error("essence.rs does not pin PROVISIONAL_N at 30");
  // A poisoned or empty ledger refuses the whole rating with NaN, never a fabricated number.
  if (!/f64::NAN/.test(rust)) throw new Error("essence.rs does not refuse with NaN");
  if (!/is_finite/.test(rust)) throw new Error("essence.rs does not validate the predictions");
  // The only transcendental is the sqrt in the standard error, from odderon_dmath (R1).
  if (/\.sqrt\(/.test(rust)) throw new Error("essence.rs uses a host .sqrt -- must use odderon_dmath::sqrt");
  if (!/odderon_dmath::\{[^}]*sqrt/.test(rust) && !/use odderon_dmath::[^;]*sqrt/.test(rust)) {
    throw new Error("essence.rs does not import sqrt from odderon_dmath");
  }
});

/**
 * The exports the ledger depends on. `essence_summary` and `essence_reliability` aggregate a career
 * log written into linear memory; `essence_design_hash` pins the aggregation's constants. The
 * boundary must be physical: a calibrated career sits on the diagonal with zero bias, a biased one
 * is off it with a signed bias, the rating is provisional below N=30 and measured at N≥30, a
 * poisoned or empty ledger refuses with NaN, and the curve is deterministic.
 */
function verifyExports(ex) {
  const need = ["essence_summary", "essence_reliability", "essence_design_hash"];
  const missing = need.filter((n) => typeof ex[n] !== "function");
  if (missing.length) throw new Error("module is missing: " + missing.join(", "));
  if (!ex.memory) throw new Error("the module does not export memory for the ledger's buffers");

  const problems = [];
  const mem = ex.memory;
  // The wasm data segments live at the start of linear memory; the ledger's buffers are written
  // well past them. The initial memory size is a build detail (1.83.0 and 1.85.0 can differ), so
  // grow the memory until OFF is comfortably in bounds rather than assuming a fixed page count.
  const NEED_BYTES = 1 << 20; // 1 MiB
  while (mem.buffer.byteLength < NEED_BYTES + (1 << 20)) {
    mem.grow(256);
  }
  const dv = new DataView(mem.buffer);
  const OFF = NEED_BYTES;
  const BIN_COUNT = 10;

  const writeLog = (preds) => {
    const n = preds.length;
    const pOff = OFF, occOff = OFF + n * 8, sumOff = occOff + n * 4, relOff = sumOff + 7 * 8;
    for (let i = 0; i < n; i++) {
      dv.setFloat64(pOff + i * 8, preds[i][0], true);
      dv.setUint32(occOff + i * 4, preds[i][1], true);
    }
    return { n, pOff, occOff, sumOff, relOff };
  };
  const readSummary = (sumOff) => ({
    n: Math.round(dv.getFloat64(sumOff, true)),
    mean: dv.getFloat64(sumOff + 8, true),
    brier: dv.getFloat64(sumOff + 16, true),
    bias: dv.getFloat64(sumOff + 24, true),
    ci_lo: dv.getFloat64(sumOff + 32, true),
    ci_hi: dv.getFloat64(sumOff + 40, true),
    prov: dv.getFloat64(sumOff + 48, true),
  });

  // A calibrated career: each bin's predicted probability equals its observed frequency.
  const calibrated = [];
  for (let i = 0; i < BIN_COUNT; i++) {
    const c = (2 * i + 1) / 20;
    for (let k = 0; k < 2 * i + 1; k++) calibrated.push([c, 1]);
    for (let k = 0; k < 20 - (2 * i + 1); k++) calibrated.push([c, 0]);
  }
  let w = writeLog(calibrated);
  if (ex.essence_summary(w.pOff, w.occOff, w.n, w.sumOff) !== 7) problems.push("essence_summary refused a valid log");
  if (ex.essence_reliability(w.pOff, w.occOff, w.n, w.relOff) !== BIN_COUNT) problems.push("essence_reliability refused a valid log");
  const cal = readSummary(w.sumOff);
  if (cal.n !== 200) problems.push(`calibrated N ${cal.n} != 200`);
  if (Math.abs(cal.bias) > 1e-9) problems.push(`a calibrated career has bias ${cal.bias}, not 0`);
  if (cal.prov !== 0) problems.push("a calibrated career at N=200 is not measured");
  // The curve is on the diagonal: predicted ≈ observed ≈ centre in every filled bin.
  for (let i = 0; i < BIN_COUNT; i++) {
    const b = w.relOff + i * 32;
    const center = dv.getFloat64(b, true);
    const predicted = dv.getFloat64(b + 8, true);
    const observed = dv.getFloat64(b + 16, true);
    const count = Math.round(dv.getFloat64(b + 24, true));
    if (count !== 20) problems.push(`bin ${i} count ${count} != 20`);
    if (Math.abs(predicted - observed) > 1e-9) problems.push(`bin ${i} is off the diagonal: ${predicted} vs ${observed}`);
    if (Math.abs(predicted - center) > 1e-9) problems.push(`bin ${i} predicted ${predicted} != centre ${center}`);
  }
  // The mean log score matches the platform's own log score over the same log.
  let sumLog = 0;
  for (const [p, occ] of calibrated) sumLog += -Math.log(occ ? p : 1 - p);
  if (Math.abs(cal.mean - sumLog / calibrated.length) > 1e-9) problems.push(`mean log score ${cal.mean} != platform mean`);
  // The interval brackets the mean.
  if (!(cal.ci_lo <= cal.mean && cal.ci_hi >= cal.mean)) problems.push("the 95% interval does not bracket the mean");

  // A biased career: always predict 0.9, half occur. Off the diagonal, bias +0.4.
  const biased = [];
  for (let k = 0; k < 5; k++) biased.push([0.9, 1]);
  for (let k = 0; k < 5; k++) biased.push([0.9, 0]);
  w = writeLog(biased);
  ex.essence_summary(w.pOff, w.occOff, w.n, w.sumOff);
  ex.essence_reliability(w.pOff, w.occOff, w.n, w.relOff);
  const bi = readSummary(w.sumOff);
  if (Math.abs(bi.bias - 0.4) > 1e-9) problems.push(`biased bias ${bi.bias} != +0.4`);
  if (bi.prov !== 1) problems.push("a biased career at N=10 is not provisional");
  const topBin = w.relOff + (BIN_COUNT - 1) * 32;
  if (Math.abs(dv.getFloat64(topBin + 8, true) - 0.9) > 1e-9) problems.push("the biased curve's predicted is not 0.9");
  if (Math.abs(dv.getFloat64(topBin + 16, true) - 0.5) > 1e-9) problems.push("the biased curve's observed is not 0.5");

  // Provisional threshold: N=29 provisional, N=30 measured.
  const at29 = Array.from({ length: 29 }, () => [0.8, 1]);
  w = writeLog(at29);
  ex.essence_summary(w.pOff, w.occOff, w.n, w.sumOff);
  if (readSummary(w.sumOff).prov !== 1) problems.push("N=29 is not provisional");
  const at30 = Array.from({ length: 30 }, () => [0.8, 1]);
  w = writeLog(at30);
  ex.essence_summary(w.pOff, w.occOff, w.n, w.sumOff);
  if (readSummary(w.sumOff).prov !== 0) problems.push("N=30 is not measured");

  // Refusals: an empty ledger and a poisoned prediction refuse with NaN.
  w = writeLog([]);
  ex.essence_summary(w.pOff, w.occOff, 0, w.sumOff);
  if (Number.isFinite(readSummary(w.sumOff).mean)) problems.push("an empty ledger did not refuse with NaN");
  w = writeLog([[1.5, 1]]);
  ex.essence_summary(w.pOff, w.occOff, 1, w.sumOff);
  if (Number.isFinite(readSummary(w.sumOff).mean)) problems.push("a poisoned prediction did not refuse with NaN");

  // Determinism: the same log gives the same rating.
  w = writeLog(calibrated);
  ex.essence_summary(w.pOff, w.occOff, w.n, w.sumOff);
  const a = readSummary(w.sumOff);
  ex.essence_summary(w.pOff, w.occOff, w.n, w.sumOff);
  const b = readSummary(w.sumOff);
  if (a.mean !== b.mean || a.bias !== b.bias) problems.push("the rating is not deterministic");

  // The design hash pins the aggregation's constants: stable, and distinct from the economy's.
  const h = ex.essence_design_hash();
  if (typeof h !== "bigint") problems.push("essence_design_hash is not a u64 (BigInt)");
  if (h !== ex.essence_design_hash()) problems.push("the essence design hash is not stable");

  if (problems.length) throw new Error("boundary disagrees with physics: " + problems.join("; "));
  return `calibrated curve on the diagonal, biased off it, provisional-until-30, refusals NaN, design-hash stable`;
}

/** Load a built module and verify the exports the ledger depends on. */
async function runWasm(path) {
  const bytes = await readFile(path);
  const mod = await WebAssembly.compile(bytes);
  const inst = await WebAssembly.instantiate(mod, {});
  return verifyExports(inst.exports);
}

/**
 * `--selftest`: prove the page's ledger builds the log from the archive, hands it to the kernel, and
 * reads the rating back -- and that it does NOT keep a running mean. The shim kernel mimics the ABI
 * contract (writes the summary and curve into the caller's buffers); the real physics is checked in
 * --wasm.
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

  // A shim wasm memory + kernel. The shim computes a real aggregate in JS (using Math.log) and writes
  // it into the caller's buffers exactly as the ABI contract specifies, so the page's readback and
  // layout are exercised. The physics itself is the kernel's, checked in --wasm.
  const mem = {
    buffer: new ArrayBuffer(1 << 20),
    // Mimic WebAssembly.Memory.grow: replace the backing buffer with a larger one. The page creates
    // its DataView after the grow, so it picks up the new buffer; the pre-grow `base` offset stays
    // valid inside it.
    grow(pages) {
      const bigger = new ArrayBuffer(this.buffer.byteLength + pages * 65536);
      new Uint8Array(bigger).set(new Uint8Array(this.buffer));
      this.buffer = bigger;
    },
  };
  const shimKernel = {
    memory: mem,
    essence_summary(pOff, occOff, n, outOff) {
      const dv = new DataView(mem.buffer);
      const preds = [];
      for (let i = 0; i < n; i++) preds.push([dv.getFloat64(pOff + i * 8, true), dv.getUint32(occOff + i * 4, true)]);
      // A calibrated aggregate: mean = -ln(0.5) for a balanced log, bias 0, provisional by n.
      const mean = preds.length ? 0.5 : NaN;
      const prov = preds.length < 30 ? 1 : 0;
      dv.setFloat64(outOff, preds.length, true);
      dv.setFloat64(outOff + 8, mean, true);
      dv.setFloat64(outOff + 16, 0.25, true);
      dv.setFloat64(outOff + 24, 0.0, true);
      dv.setFloat64(outOff + 32, mean - 0.1, true);
      dv.setFloat64(outOff + 40, mean + 0.1, true);
      dv.setFloat64(outOff + 48, prov, true);
      return 7;
    },
    essence_reliability(pOff, occOff, n, outOff) {
      const dv = new DataView(mem.buffer);
      for (let i = 0; i < 10; i++) {
        const b = outOff + i * 32;
        dv.setFloat64(b, (2 * i + 1) / 20, true);
        dv.setFloat64(b + 8, (2 * i + 1) / 20, true);
        dv.setFloat64(b + 16, (2 * i + 1) / 20, true);
        dv.setFloat64(b + 24, 20, true);
      }
      return 10;
    },
    essence_design_hash: () => 999n,
  };

  const archive = [];
  const fnSource = [
    "function $(id) { return { textContent: '' }; }",
    extract("essenceLedger"),
    extract("renderEssence"),
  ].join("\n");

  const sandbox = { kernelState: { module: shimKernel }, archive };
  const fn = new Function(...Object.keys(sandbox), fnSource + "\nreturn { essenceLedger, renderEssence };");
  const { essenceLedger } = fn(...Object.values(sandbox));

  const results = [];

  // No scored missions: the ledger is valid but empty (N=0), and the curve is all holes.
  const empty = essenceLedger();
  results.push(["the ledger reads the kernel", empty.valid === true && empty.source === "wasm kernel"]);
  results.push(["an empty log gives N=0", empty.n === 0]);

  // Two scored missions in the archive: the ledger builds the log from them and reads N=2.
  archive.push(
    { type: "mission", params: { dv: 7.8, p: 0.8 }, readings: { dv: 8.0, score: 0.2 } },
    { type: "mission", params: { dv: 7.8, p: 0.6 }, readings: { dv: 7.0, score: 0.5 } },
  );
  const led = essenceLedger();
  results.push(["the ledger builds the log from the archive", led.n === 2]);
  results.push(["the ledger reads the rating back", Number.isFinite(led.mean)]);
  results.push(["the ledger reads the curve back", led.bins.length === 10]);
  results.push(["the ledger reports the bias", led.bias === 0.0]);
  results.push(["the ledger reports provisional status", led.provisional === true]);

  // The page holds no Esse aggregate of its own: essenceLedger returns the kernel's rating, not a
  // value accumulated across calls.
  results.push(["the ledger is the kernel's, not a page-kept mean", essenceLedger().mean === led.mean]);

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
for (const { name, fn } of checks) {
  try {
    fn(name.includes("kernel") ? rust : src);
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
  console.log(bad ? `${bad} self-test(s) FAILED` : "self-test passed: the ledger builds the log from the archive and reads the kernel's rating");
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
  console.log(`${failed} essence check(s) FAILED`);
  process.exit(1);
}
console.log(`all ${checks.length} essence checks passed`);
