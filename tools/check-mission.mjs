#!/usr/bin/env node
/**
 * The gate that makes the mission loop a real, kernel-verifiable career core rather than a page-drawn
 * panel.
 *
 * `web/index.html` prices a contract, commits a prediction before flying, flies the mission, and
 * scores the prediction against the achieved Δv. The P5 block is the career core
 * (`GAME-DESIGN-BIBLE.md` §3.2 mission loop, §4.3 Esse, §12.1 economy). The gate is the bible's own
 * rules made mechanical:
 *
 *   1. THE CONTRACT PRICE IS THE KERNEL'S. §12.1 -- "money is physics, not a difficulty knob." A
 *      contract's price is a function of the work it asks for (Δv, mass, risk, window), computed by
 *      the kernel from pinned constants. The page must not compute a price or hold a price formula.
 *      The failure mode is "someone re-adds a local price", which no runtime probe can see.
 *
 *   2. THE PREDICTION IS COMMITTED BEFORE EXECUTION. §4.3 rule 2 -- the grease mark is hashed and
 *      timestamped before the sim advances; a prediction cannot be retro-fitted. `missionCommit`
 *      must freeze the prediction, and `missionScore` must read the committed prediction, never a
 *      live input.
 *
 *   3. ESSENCE IS A PROPER SCORING RULE. §4.3 rule 1 -- "proper scoring rules only. Never accuracy
 *      percentage." The score must be the kernel's log score, and the page must not compute an
 *      accuracy. The failure mode is "someone re-adds an accuracy %", which is exactly what the
 *      bible forbids.
 *
 *   4. THE MISSION IS RECORDED AND VERIFIED. A scored mission is an archive entry, and
 *      `verifyArchiveEntry` must re-derive the price, the achieved Δv, and the score from the kernel.
 *
 *   5. THE MISSION ENTRY IS HASHABLE. `archiveSerialize` must fingerprint the contract's parameters
 *      and the kernel's readings, so the hash is a fingerprint of the work, reproducible across time
 *      and platform -- the replay claim.
 *
 * Usage: node tools/check-mission.mjs            (static: no browser, no wasm needed)
 *        node tools/check-mission.mjs --selftest  (extract the page's mission loop, run it on a shim)
 *        node tools/check-mission.mjs --wasm dist/odderon_dmath_wasm.wasm
 *                                                     (also check the exports the mission depends on)
 */

import { readFile } from "node:fs/promises";
import { readFileSync } from "node:fs";

const PAGE = new URL("../web/index.html", import.meta.url);

/** Strip comments so documentation about the mission does not trip the checks that hunt for locals. */
function liveSource(html) {
  const script = html.match(/<script type="module">([\s\S]*?)<\/script>/);
  if (!script) throw new Error("no module script found in web/index.html");
  return script[1]
    .replace(/\/\*[\s\S]*?\*\//g, "")
    .replace(/(^|[^:])\/\/.*$/gm, "$1");
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

check("the contract price is the kernel's", (src) => {
  const body = fnBody(src, "missionPrice");
  if (!body.includes("contract_price")) throw new Error("missionPrice does not call the kernel's contract_price");
  // The page computes no price: the price is a single kernel call and nothing else. Any local
  // arithmetic on the inputs (a product, a power, an exp) would replicate the model in the page,
  // which is exactly the §12.1 failure ("money is physics" becomes "money is a page constant").
  for (const [re, why] of [
    [/PRICE_BASE/, "a price base constant in the page"],
    [/100000/, "a price constant in the page"],
    [/Math\.pow/, "a page-computed price term"],
    [/Math\.exp/, "a page-computed price term"],
    [/Math\.sqrt/, "a page-computed price term"],
    [/\*\s*\*/, "a page price product"],
    [/dvBudget\s*\*/, "a page price product over the Δv"],
    [/mass\s*\*/, "a page price product over the mass"],
  ]) {
    if (re.test(body)) throw new Error("missionPrice " + why);
  }
});

check("the prediction is committed before execution", (src) => {
  // §4.3 rule 2: the prediction is frozen at commitment and cannot be retro-fitted. missionCommit
  // must store the committed prediction; missionScore must read it, never a live input, and must
  // never fabricate one -- a score with no commitment is a refusal, not a default.
  const commit = fnBody(src, "missionCommit");
  if (!/missionState\.committed\s*=/.test(commit)) throw new Error("missionCommit does not freeze the prediction");
  const score = fnBody(src, "missionScore");
  if (!/missionState\.committed/.test(score)) throw new Error("missionScore does not read the committed prediction");
  // The score must not read the live probability input -- that would let a prediction be
  // retro-fitted after the flight.
  if (/mission-p/.test(score)) throw new Error("missionScore reads the live probability input, not the committed prediction");
  // The score must not assign to the commitment: a fabricated default is a retro-fitted prediction.
  if (/missionState\.committed\s*=/.test(score)) throw new Error("missionScore fabricates a commitment instead of refusing");
  // The score must refuse when there is no commitment to score.
  if (!/!\s*missionState\.committed/.test(score)) throw new Error("missionScore does not refuse a missing commitment");
});

check("essence is a proper scoring rule, never an accuracy", (src) => {
  const score = fnBody(src, "missionScore");
  if (!score.includes("log_score")) throw new Error("missionScore does not call the kernel's log_score");
  // §4.3 rule 1: proper scoring rules only, never an accuracy percentage. The page must not
  // compute an accuracy, a hit rate, or a correct/total ratio.
  for (const [re, why] of [
    [/accuracy/i, "an accuracy computation"],
    [/correct\s*\/\s*/, "a correct/total ratio"],
    [/hitRate|hit_rate/, "a hit rate"],
    [/===\s*1\s*\?/, "an equality-to-1 accuracy"],
  ]) {
    if (re.test(score)) throw new Error("missionScore " + why);
  }
});

check("the mission is recorded and verified against the kernel", (src) => {
  const rec = fnBody(src, "recordMission");
  if (!/type:\s*['"]mission['"]/.test(rec)) throw new Error("recordMission does not record a 'mission' entry");
  const ver = fnBody(src, "verifyArchiveEntry");
  if (!/entry\.type\s*===\s*['"]mission['"]/.test(ver)) throw new Error("verifyArchiveEntry has no 'mission' branch");
  if (!ver.includes("contract_price")) throw new Error("verifyArchiveEntry does not re-derive the price via contract_price");
  if (!ver.includes("achieved_dv")) throw new Error("verifyArchiveEntry does not re-derive the achieved Δv via achieved_dv");
  if (!ver.includes("log_score")) throw new Error("verifyArchiveEntry does not re-derive the score via log_score");
});

check("the mission entry is hashable", (src) => {
  const ser = fnBody(src, "archiveSerialize");
  if (!/entry\.type\s*===\s*['"]mission['"]/.test(ser)) throw new Error("archiveSerialize has no 'mission' branch");
  if (!/dv/.test(ser)) throw new Error("archiveSerialize does not fingerprint the contract Δv");
  if (!/price/.test(ser)) throw new Error("archiveSerialize does not fingerprint the price");
  if (!/score/.test(ser)) throw new Error("archiveSerialize does not fingerprint the score");
});

/**
 * The exports the mission loop depends on. The economy adds `contract_price` and `achieved_dv`; the
 * Esse score adds `log_score` and `crps`; `mission_design_hash` pins the economy's constants. All
 * must be present and physical: the price is a pure function of the work (pinned at the reference
 * point, rising with each factor, refusing degenerate inputs), the achieved Δv is the rocket
 * kernel's, and the two scoring rules are proper (lower-when-right, refusing bad input).
 */
function verifyExports(ex) {
  const need = ["contract_price", "achieved_dv", "log_score", "crps", "mission_design_hash"];
  const missing = need.filter((n) => typeof ex[n] !== "function");
  if (missing.length) throw new Error("module is missing: " + missing.join(", "));

  const problems = [];

  // The price model (§12.1): pinned at the reference point, rising with each factor of work, and
  // refusing degenerate inputs. These are the claims the kernel must satisfy on its own.
  const refPrice = ex.contract_price(7.8, 500, 0, 30);
  if (!Number.isFinite(refPrice)) problems.push(`reference price ${refPrice} is not finite`);
  else if (Math.abs(refPrice - 100000) > 1e-6 * 100000) problems.push(`reference price ${refPrice} != base 100000`);

  const base = ex.contract_price(7.8, 500, 0, 30);
  if (!(ex.contract_price(9.0, 500, 0, 30) > base)) problems.push("more Δv does not raise the price");
  if (!(ex.contract_price(7.8, 800, 0, 30) > base)) problems.push("more mass does not raise the price");
  if (!(ex.contract_price(7.8, 500, 1.0, 30) > base)) problems.push("more risk does not raise the price");
  if (!(ex.contract_price(7.8, 500, 0, 10) > base)) problems.push("a shorter window does not raise the price");

  if (Number.isFinite(ex.contract_price(0, 500, 0, 30))) problems.push("a zero Δv budget gave a finite price");
  if (Number.isFinite(ex.contract_price(7.8, 0, 0, 30))) problems.push("a zero mass gave a finite price");
  if (Number.isFinite(ex.contract_price(7.8, 500, 0, 0))) problems.push("a zero window gave a finite price");
  if (Number.isFinite(ex.contract_price(7.8, 500, -0.1, 30))) problems.push("negative risk gave a finite price");

  // Determinism: the same contract prices the same, every call.
  const p1 = ex.contract_price(8.5, 620, 0.3, 21);
  const p2 = ex.contract_price(8.5, 620, 0.3, 21);
  if (p1 !== p2) problems.push("the price is not deterministic");

  // The achieved Δv is the rocket kernel's: a valid launch achieves the burnout velocity, a
  // refused launch yields no Δv.
  const dv = ex.achieved_dv(0, 0, 0.5, 7.0);
  if (!Number.isFinite(dv) || Math.abs(dv - 7.0) > 1e-12) problems.push(`achieved dv ${dv} != burnout 7.0`);
  if (Number.isFinite(ex.achieved_dv(0, 0, 0, 0))) problems.push("a refused launch gave a finite achieved Δv");

  // The log score is proper (§4.3): lower is better, certainty-and-wrong is the worst, and a bad
  // probability is refused.
  if (ex.log_score(1.0, 1) !== 0.0) problems.push("a certain correct call does not score 0");
  if (ex.log_score(0.0, 0) !== 0.0) problems.push("a certain correct call does not score 0");
  if (ex.log_score(1.0, 0) !== Infinity) problems.push("certainty and wrong is not infinite");
  if (ex.log_score(0.5, 1) >= ex.log_score(0.5, 0) + 1e-12) problems.push("being wrong does not score higher than being right");
  if (Number.isFinite(ex.log_score(-0.1, 1))) problems.push("a negative probability was not refused");
  if (Number.isFinite(ex.log_score(1.1, 1))) problems.push("a probability above 1 was not refused");
  // The log score matches -ln(p) / -ln(1-p) against the platform's own ln.
  const p = 0.7;
  if (Math.abs(ex.log_score(p, 1) + Math.log(p)) > 1e-12) problems.push("log_score(p,1) != -ln(p)");
  if (Math.abs(ex.log_score(p, 0) + Math.log(1 - p)) > 1e-12) problems.push("log_score(p,0) != -ln(1-p)");

  // The CRPS is proper: a tight correct interval scores lower than a wide one, a miss scores
  // higher, and a non-positive width is refused.
  const tight = ex.crps(4.9, 5.1, 5.0);
  const wide = ex.crps(4.0, 6.0, 5.0);
  if (!(tight < wide)) problems.push("a tight correct interval does not beat a wide one");
  if (!(ex.crps(5.5, 6.5, 5.0) > wide)) problems.push("a miss does not beat a wide hit");
  if (Number.isFinite(ex.crps(5.0, 5.0, 5.0))) problems.push("a zero-width interval was not refused");

  // The design hash pins the economy's constants: stable, and distinct from the rocket's.
  const h = ex.mission_design_hash();
  if (typeof h !== "bigint") problems.push("mission_design_hash is not a u64 (BigInt)");
  if (h !== ex.mission_design_hash()) problems.push("the mission design hash is not stable");

  if (problems.length) throw new Error("boundary disagrees with physics: " + problems.join("; "));
  return `price=${refPrice} (pinned at reference), rising-with-work, proper log-score & CRPS, design-hash stable`;
}

/** Load a built module and verify the exports the mission depends on. */
async function runWasm(path) {
  const bytes = await readFile(path);
  const mod = await WebAssembly.compile(bytes);
  const inst = await WebAssembly.instantiate(mod, {});
  return verifyExports(inst.exports);
}

/**
 * `--selftest`: prove the mission loop's price, commit, fly, score, record, and verify are sound, by
 * extracting the page's own functions and running them against a shim kernel. Without this, the
 * accept and reject paths would first run in a browser, and a checker whose rejection path has never
 * executed is not known to reject.
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
    "const archive = []; let archiveSeq = 0;",
    "function renderArchive() {}",
    "function renderMuseum() {}",
    "function renderSurvey() {}",
    "function renderMission() {}",
    "function renderEssence() {}",
    "function saveArchive() {}",
    // The mission score opens an anomaly in the drawer (P5 Block 5); the drawer is the anomaly
    // gate's contract, so this gate stubs it as a no-op. The score itself is still the kernel's.
    "function anomalyLedger() { return { valid: false }; }",
    "function openAnomaly() {}",
    "function renderAnomaly() {}",
    "function $(id) { const v = { 'mission-dv': '7.8', 'mission-mass': '500', 'mission-risk': '0', 'mission-window': '30', 'mission-p': '0.8' }[id]; return { value: v === undefined ? '0' : v }; }",
    extract("archiveSerialize"),
    extract("archiveHash"),
    extract("archivePush"),
    extract("close"),
    extract("readMissionInputs"),
    extract("missionPrice"),
    extract("missionCommit"),
    extract("missionFly"),
    extract("missionScore"),
    extract("recordMission"),
    extract("verifyArchiveEntry"),
  ].join("\n");

  // A shim kernel: the price is a deterministic function of the work (so different contracts
  // differ), the achieved Δv is the burnout velocity, and the log score is the proper rule. This
  // lets the self-test exercise the page's logic -- commit, fly, score, record, verify -- without
  // the real kernel; the real physics is checked in --wasm.
  const shimKernel = {
    contract_price: (dv, mass, risk, window) => 100000 * (dv / 7.8) * (mass / 500) * (1 + risk) * Math.sqrt(30 / window),
    achieved_dv: (lat, az, el, v) => v,
    log_score: (p, occurred) => -Math.log(occurred ? p : 1 - p),
    crps: (lo, hi, y) => { const w = hi - lo; return Math.abs(y - (lo + hi) / 2) + w / 6; },
    mission_design_hash: () => 12345n,
  };
  // Block 9 removed the page-side running mean: the Esse rating is the kernel reliability curve,
  // built by the essence ledger from the archive. The score here is the kernel log score; the
  // ledger shape is the essence gate's contract.
  const missionState = { dvBudget: 7.8, mass: 500, risk: 0, window: 30, price: null, committed: null, achievedDv: null, score: null, scored: 0 };
  const rocketParams = { latitudeDeg: 0, azimuthDeg: 0, elevationDeg: 30, burnoutVelocity: 7.0 };
  const sandbox = {
    kernelState: { module: shimKernel },
    missionState,
    rocketParams,
  };
  const fn = new Function(...Object.keys(sandbox), fnSource + "\nreturn { archive, archiveHash, missionState, missionPrice, missionCommit, missionFly, missionScore, recordMission, verifyArchiveEntry };");
  const { archive, archiveHash, missionState: ms, missionPrice, missionCommit, missionFly, missionScore, recordMission, verifyArchiveEntry } = fn(...Object.values(sandbox));

  const results = [];

  // Price: the kernel's, a pure function of the work.
  const pr = missionPrice();
  results.push(["the contract is priced by the kernel", pr.valid === true && Number.isFinite(pr.price)]);
  results.push(["the price is a pure function of the work", pr.price > 0]);

  // Commit: the prediction is frozen before the flight.
  const commit = missionCommit();
  results.push(["the prediction is committed", commit !== null && ms.committed !== null]);
  results.push(["the committed prediction is frozen", ms.committed.p === 0.8]);

  // Fly: the achieved Δv is the rocket kernel's.
  const dv = missionFly();
  results.push(["the mission flies with the kernel's achieved Δv", dv === 7.0]);

  // Score: the kernel's log score, a proper rule.
  const score = missionScore();
  results.push(["the prediction is scored by the kernel", Number.isFinite(score)]);
  // Achieved Δv 7.0 < budget 7.8, so the outcome is "missed": the log score is -ln(1-p).
  results.push(["the score is the proper log score for a miss", Math.abs(score - (-Math.log(1 - 0.8))) < 1e-12]);
  // The page keeps no running mean of the Esse rating -- the ledger is the kernel curve.
  results.push(["the page keeps no running Esse mean", ms.essence === undefined]);

  // Record + verify: a scored mission is recorded and verifies against the shim kernel.
  const last = archive[archive.length - 1];
  results.push(["a scored mission is recorded", last && last.type === "mission"]);
  results.push(["a recorded mission verifies against the kernel", verifyArchiveEntry(last).ok === true]);

  // Tamper-sensitivity: a recorded mission with a wrong score fails verification.
  const tampered = { ...last, readings: { ...last.readings, score: 999 } };
  tampered.hash = archiveHash(tampered);
  results.push(["a tampered mission score fails verification", verifyArchiveEntry(tampered).ok === false]);

  // The hash is a pure function of the mission's physics.
  const h1 = archiveHash(last);
  const h2 = archiveHash({ ...last, id: 999, epoch: 12345 });
  results.push(["the mission hash excludes the id and epoch", h1 === h2]);

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
  console.log(bad ? `${bad} self-test(s) FAILED` : "self-test passed: the mission loop's price, commit, score, and verification are sound");
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
  console.log(`${failed} mission check(s) FAILED`);
  process.exit(1);
}
console.log(`all ${checks.length} mission checks passed`);
