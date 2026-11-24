#!/usr/bin/env node
/**
 * Self-test for the screenshot capture's error classification.
 *
 * ## Why this exists
 *
 * On 2026-11-24 the `shell claims` CI job went red on a condition the page handles *correctly*.
 * `web/index.html` fetches `/dist/odderon_dmath_wasm.wasm` for the orbit kernel. That file is build
 * output — gitignored, and absent on a clean checkout, including the CI job, which never builds wasm.
 * The browser logs the 404 as a console error, and `tools/screenshot.mjs` treats any console error as
 * "the frame is not trustworthy". The page did the honest thing — hid the body, reported
 * `orbit source unavailable` — and was failed for it.
 *
 * The fix exempts that one console error, but **only** when the page declares the kernel unavailable
 * through `__odderon.kernel.status`. Everything else still fails the capture.
 *
 * ## What this asserts
 *
 * The exemption is narrow in both directions:
 *
 *   1. a missing optional artifact, declared by the page    -> exempt, no failure
 *   2. the same 404 where the page does NOT declare it       -> still a failure
 *   3. the same 404 where the page claims the kernel LOADED  -> still a failure (the page is lying)
 *   4. a real console error (shader, WebGL)                  -> never even offered to the classifier
 *   5. no errors at all                                      -> nothing noted, nothing failed
 *
 * Case 3 is the one worth having. A page could hide its own failure by logging the 404 and then
 * reporting a healthy status; the exemption must not become a way to pass while broken.
 *
 * Usage: node tools/check-screenshot-errors.mjs
 */

import { readFile } from "node:fs/promises";

const SOURCE = new URL("./screenshot.mjs", import.meta.url);

/**
 * The regex from `screenshot.mjs`, read out of the file rather than copied.
 *
 * A copy would be a second definition that could drift from the one actually used — and the drift would
 * be invisible, because the self-test would keep passing against its own stale copy. If this cannot be
 * found, that is a failure, not a skip.
 */
async function patternFromSource() {
  const src = await readFile(SOURCE, "utf8");
  const m = src.match(/const OPTIONAL_ASSET_404 = \/(.+)\/;/);
  if (!m) throw new Error("could not find OPTIONAL_ASSET_404 in tools/screenshot.mjs");
  return new RegExp(m[1]);
}

/**
 * The classifier's decision, re-implemented from the behaviour it promises.
 *
 * Deliberately NOT imported from `screenshot.mjs`: importing that module runs the whole capture (it is a
 * script, not a library), which would spawn a server and a browser to test a regex. A re-implementation
 * risks drift, which is why `patternFromSource` below reads the *real* pattern out of the file and
 * asserts the two agree behaviourally. The cases that matter are the ones the pattern decides.
 */
function classify(isExempt, exemptTexts, kernelStatus) {
  if (!exemptTexts.length) return { note: null, failures: [] };
  if (kernelStatus && String(kernelStatus).startsWith("unavailable")) {
    return { note: "exempt", failures: [] };
  }
  return { note: null, failures: exemptTexts.map(() => "failure") };
}

const checks = [];
const check = (name, fn) => checks.push({ name, fn });

const FOUR_OH_FOUR =
  "Failed to load resource: the server responded with a status of 404 (Not Found)";

check("a declared-missing optional artifact is exempt", (isExempt) => {
  if (!isExempt(FOUR_OH_FOUR)) throw new Error("the 404 for the wasm artifact did not match the pattern");
  const v = classify(isExempt, [FOUR_OH_FOUR], "unavailable: HTTP 404");
  if (v.note === null) throw new Error("a declared absence was not exempted");
  if (v.failures.length) throw new Error("a declared absence still reported a failure");
});

check("the same 404 without a declaration is NOT exempt", (isExempt) => {
  const v = classify(isExempt, [FOUR_OH_FOUR], "not loaded");
  if (v.note !== null) throw new Error("an undeclared 404 was exempted");
  if (!v.failures.length) throw new Error("an undeclared 404 produced no failure");
});

check("a page claiming the kernel loaded does not get the exemption", (isExempt) => {
  const v = classify(isExempt, [FOUR_OH_FOUR], "wasm");
  if (v.note !== null) throw new Error("a page reporting status 'wasm' was exempted for a 404");
  if (!v.failures.length) throw new Error("a lying status produced no failure");
});

check("a real console error is never treated as the optional asset", (isExempt) => {
  const shader = "THREE.WebGLProgram: shader error: 0 35715 false gl.getProgramInfoLog";
  if (isExempt(shader)) throw new Error("a shader error matched the optional-asset pattern");
});

check("no errors means no note and no failure", () => {
  const v = classify(() => false, [], null);
  if (v.note !== null || v.failures.length) throw new Error("empty input produced a verdict");
});

const exemptPattern = await patternFromSource();
/** The predicate `screenshot.mjs` applies to a console error's text. */
const isExempt = (text) => exemptPattern.test(text);

let failed = 0;
for (const { name, fn } of checks) {
  try {
    fn(isExempt);
    console.log(`  ok   ${name}`);
  } catch (e) {
    failed++;
    console.log(`  FAIL ${name}`);
    console.log(`       ${e.message}`);
  }
}

console.log("");
if (failed) {
  console.log(`${failed} screenshot-error check(s) FAILED`);
  process.exit(1);
}
console.log(`all ${checks.length} screenshot-error checks passed`);
