#!/usr/bin/env bash
# Native vs wasm32 determinism check.
#
# The kernel must produce identical bits on every target, or a replay cannot be verified and an Esse
# rating is worthless (docs/CREDIBILITY.md §5). This compares the golden hash computed by the native
# build with the one computed by the wasm build, in a single job so nothing has to leave CI.
#
# Assets required:
#   crates/odderon-dmath/examples/hash.rs   -> prints the native hash
#   crates/odderon-dmath-wasm               -> cdylib exporting `golden`
set -euo pipefail

echo "== native =="
NATIVE=$(cargo run -q -p odderon-dmath --example hash)
echo "native : $NATIVE"

echo "== wasm32 =="
cargo build -q --release --target wasm32-unknown-unknown -p odderon-dmath-wasm

WASM_PATH=$(find target/wasm32-unknown-unknown/release -maxdepth 1 -name 'odderon_dmath_wasm.wasm' | head -1)
if [ -z "$WASM_PATH" ]; then
  echo "::error::no wasm artifact produced by odderon-dmath-wasm"
  ls -la target/wasm32-unknown-unknown/release | head -20
  exit 1
fi
echo "wasm   : $WASM_PATH ($(stat -c%s "$WASM_PATH") bytes)"

# Node runs wasm natively, so no browser or bindings generator is needed. The module is deliberately
# import-free precisely so that `{}` is a valid import object.
WASM=$(node -e '
  const fs = require("fs");
  const mod = new WebAssembly.Module(fs.readFileSync(process.argv[1]));
  const inst = new WebAssembly.Instance(mod, {});
  if (typeof inst.exports.golden !== "function") {
    console.error("exports:", Object.keys(inst.exports).join(", "));
    process.exit(1);
  }
  // A `u64` above 2^63 crosses the C ABI as a signed `i64`, so JS sees a *negative* BigInt and
  // `toString(16)` renders it with a minus sign — which is not the unsigned value at all. This bit us
  // on the first run: the two hashes were identical and the comparison failed. `asUintN` is the only
  // correct conversion.
  const h = BigInt.asUintN(64, inst.exports.golden());
  console.log("0x" + h.toString(16).padStart(16, "0"));
' "$WASM_PATH")
echo "wasm   : $WASM"

if [ "$NATIVE" != "$WASM" ]; then
  echo "::error::DETERMINISM FAILURE — native $NATIVE != wasm $WASM"
  echo "A transcendental is behaving differently on the two targets. Bisect the probe set in"
  echo "odderon_dmath::PROBES to find the first divergent input."
  exit 1
fi

echo ""
echo "PASS: native and wasm32 agree bit-for-bit ($NATIVE)"
