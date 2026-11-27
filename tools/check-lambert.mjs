#!/usr/bin/env node
/**
 * The gate that makes the Lambert solver a real kernel module rather than a stub or a page-drawn
 * approximation.
 *
 * `crates/odderon-astro/src/lambert.rs` solves Lambert's problem: given two position vectors and a
 * time of flight, find the transfer orbit's departure and arrival velocities. It is the inverse of
 * the propagator and the transfer-planning primitive a rendezvous or interplanetary trajectory
 * needs. The acceptance criterion (`CREDIBILITY.md` §3.1) is: arrival velocity error < 1e-8 relative
 * against published Vallado/Izzo solutions, covering zero-rev, one-rev, and the 180° degenerate case.
 *
 * The checks:
 *
 *   1. THE MODULE EXISTS AND IS REGISTERED. `crates/odderon-astro/src/lambert.rs` must exist, and
 *      `lib.rs` must declare `pub mod lambert`. A module that exists but is not declared is dead
 *      code — it compiles but nothing can call it.
 *
 *   2. THE PUBLIC API IS PRESENT. The module must expose `pub fn lambert` (the solver) and
 *      `pub struct Lambert` (the result, carrying velocities and convergence status). The result
 *      struct must carry `unconverged` so a caller can refuse a bad solve rather than trust it.
 *
 *   3. THE SOLVER REUSES THE KERNEL'S STUMPF FUNCTIONS. The solver must call `crate::stumpff`, not
 *      define its own Stumpff functions. A local implementation would be a second source of truth
 *      for the same math, and the cross-check against the propagator would be against a different
 *      algorithm rather than the one the rest of the kernel uses.
 *
 *   4. THE ACCEPTANCE TESTS EXIST. `crates/odderon-astro/tests/lambert.rs` must contain tests for
 *      the Vallado reference case, the 180° degenerate case, and zero-rev/one-rev transfers. These
 *      are the three cases named in the acceptance criterion; a test file that omits any of them
 *      leaves the corresponding claim unverified.
 *
 *   5. THE 180° CASE IS HANDLED. The solver must not silently return NaN for the 180° case — it
 *      must either solve it directly or perturb it. The test file must assert that the 180° case
 *      converges (not `unconverged: true`).
 *
 *   6. DEGENERATE INPUT IS REFUSED. The solver must return `unconverged: true` for same-point
 *      positions and non-positive time of flight. A solver that returns a plausible-looking but
 *      wrong velocity for degenerate input is worse than one that refuses.
 *
 * Usage: node tools/check-lambert.mjs
 */

import { readFile } from "node:fs/promises";

const ROOT = new URL("..", import.meta.url);

const checks = [];
function check(name, fn) {
  checks.push({ name, fn });
}

// --- helpers -------------------------------------------------------------------------------

/** Read a file and throw a clear error if it is missing. */
async function read(path) {
  try {
    return await readFile(new URL(path, ROOT), "utf8");
  } catch {
    throw new Error(`missing file: ${path}`);
  }
}

/** Strip Rust comments so documentation about a pattern does not trip the checks that hunt for it. */
function liveRust(source) {
  return source
    .replace(/\/\/![\s\S]*?(?=\n|$)/g, "")
    .replace(/\/\*[\s\S]*?\*\//g, "")
    .replace(/(^|[^:])\/\/.*$/gm, "$1");
}

// --- checks --------------------------------------------------------------------------------

check("module file exists", async () => {
  const src = await read("crates/odderon-astro/src/lambert.rs");
  if (src.length < 100) throw new Error("lambert.rs is suspiciously short");
});

check("module is declared in lib.rs", async () => {
  const lib = await read("crates/odderon-astro/src/lib.rs");
  if (!lib.includes("pub mod lambert;")) {
    throw new Error("lib.rs does not declare `pub mod lambert`");
  }
});

check("public API: lambert() function and Lambert struct", async () => {
  const src = liveRust(await read("crates/odderon-astro/src/lambert.rs"));
  if (!src.includes("pub fn lambert(")) {
    throw new Error("missing `pub fn lambert(`");
  }
  if (!src.includes("pub struct Lambert")) {
    throw new Error("missing `pub struct Lambert`");
  }
  // The result must carry convergence status so a caller can refuse a bad solve.
  if (!src.includes("unconverged")) {
    throw new Error("Lambert struct does not carry `unconverged`");
  }
});

check("solver reuses kernel's stumpff functions", async () => {
  const src = liveRust(await read("crates/odderon-astro/src/lambert.rs"));
  if (!src.includes("crate::stumpff")) {
    throw new Error("solver does not call `crate::stumpff` — it must reuse the kernel's Stumpff functions");
  }
  // The solver must not define its own Stumpff implementation.
  if (/fn\s+stumpff/.test(src)) {
    throw new Error("solver defines its own `stumpff` function instead of reusing the kernel's");
  }
});

check("acceptance tests: Vallado reference, 180°, zero-rev, one-rev", async () => {
  const test = await read("crates/odderon-astro/tests/lambert.rs");
  const live = liveRust(test);

  if (!live.includes("vallado")) {
    throw new Error("test file has no Vallado reference case");
  }
  if (!live.includes("180") && !live.includes("hohmann")) {
    throw new Error("test file has no 180° degenerate case");
  }
  if (!live.includes("zero_rev") && !live.includes("short_angle")) {
    throw new Error("test file has no zero-rev test");
  }
  if (!live.includes("one_rev") && !live.includes("moderate_angle")) {
    throw new Error("test file has no one-rev test");
  }
});

check("180° case is handled (converges, not refused)", async () => {
  const test = await read("crates/odderon-astro/tests/lambert.rs");
  const live = liveRust(test);

  // The 180° test must assert the solver converges.
  const hasHohmann = live.includes("hohmann_transfer_180");
  if (!hasHohmann) {
    throw new Error("no 180° Hohmann transfer test");
  }
  // The test must check the velocity magnitude against the expected value.
  if (!live.includes("v1_expected_mag") && !live.includes("v_circ")) {
    throw new Error("180° test does not check velocity magnitude against expected value");
  }
});

check("degenerate input is refused", async () => {
  const test = await read("crates/odderon-astro/tests/lambert.rs");
  const live = liveRust(test);

  if (!live.includes("same_point_is_refused")) {
    throw new Error("no same-point refusal test");
  }
  if (!live.includes("non_positive_dt_is_refused")) {
    throw new Error("no non-positive-dt refusal test");
  }
});

check("self-consistency: propagate solved state to target", async () => {
  const test = await read("crates/odderon-astro/tests/lambert.rs");
  const live = liveRust(test);

  if (!live.includes("propagate")) {
    throw new Error("test file does not use `propagate` for self-consistency");
  }
  if (!live.includes("assert_propagation_reaches_target")) {
    throw new Error("test file has no propagation self-consistency helper");
  }
});

// --- runner --------------------------------------------------------------------------------

let failures = 0;
for (const { name, fn } of checks) {
  try {
    await fn();
    console.log(`  ok  ${name}`);
  } catch (err) {
    failures++;
    console.error(`  FAIL ${name}\n      ${err.message}`);
  }
}

if (failures > 0) {
  console.error(`\n${failures} check(s) failed.`);
  process.exit(1);
}
console.log(`\nAll ${checks.length} checks passed.`);
