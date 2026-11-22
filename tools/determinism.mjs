#!/usr/bin/env node
/**
 * Cross-platform determinism check: native vs wasm.
 *
 * The kernel's transcendentals must produce identical bits on every platform, or a replay cannot be
 * verified and an Esse rating is worthless (docs/CREDIBILITY.md §5). The native side is checked by
 * `cargo test` in CI. This script checks the *other* side: it loads the wasm artifact that CI built
 * and compares the same golden hash.
 *
 * Usage:
 *   node tools/determinism.mjs [path/to/odderon_dmath.wasm]
 *
 * Exit code 0 on match, 1 on mismatch or missing artifact. No browser required — Node runs wasm
 * natively, which is why the compile/run split across CI and this sandbox works at all.
 */

import { readFileSync, existsSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, "..");

const wasmPath = resolve(root, process.argv[2] ?? "dist/odderon_dmath.wasm");
const goldenPath = resolve(root, "crates/odderon-dmath/tests/golden.txt");

if (!existsSync(wasmPath)) {
  console.error(`no wasm at ${wasmPath}\n  run: npm run fetch:wasm`);
  process.exit(1);
}

// Wasm modules built from this crate are standalone (no imports). If that ever changes, the missing
// imports will be listed here, which is more useful than a generic instantiation failure.
const imports = {};
const { instance } = await WebAssembly.instantiate(readFileSync(wasmPath), imports);
const wasm = instance.exports;

const required = ["golden", "sin", "cos", "ln", "atan2", "probe_count"];
const missing = required.filter((n) => !(n in wasm));
if (missing.length) {
  console.error(`wasm is missing exports: ${missing.join(", ")}`);
  console.error(`available: ${Object.keys(wasm).join(", ")}`);
  process.exit(1);
}

// `u64` above 2^63 crosses the C ABI as a signed `i64`, so JS hands back a negative BigInt. Reading
// it as signed would render a minus sign and compare unequal for identical values.
const fmt = (h) => "0x" + BigInt.asUintN(64, h).toString(16).padStart(16, "0");
const wasmHash = fmt(wasm.golden());

console.log(`wasm      : ${wasmPath}`);
console.log(`probes    : ${wasm.probe_count()}`);
console.log(`wasm hash : ${wasmHash}`);

if (!existsSync(goldenPath)) {
  console.log(`\nno golden file yet (${goldenPath}).`);
  console.log(`Bootstrap: write this single line to arm the gate —`);
  console.log(wasmHash);
  process.exit(0);
}

const expected = readFileSync(goldenPath, "utf8").trim();
console.log(`golden    : ${expected}`);

if (wasmHash !== expected) {
  console.error("\nFAIL: wasm and the committed golden hash disagree.");
  console.error("Either the artefact is stale, or a change altered the arithmetic.");
  console.error("A real change means updating golden.txt AND noting it in docs/DETERMINISM.md.");
  process.exit(1);
}

console.log("\nPASS: wasm reproduces the golden hash bit-for-bit.");
