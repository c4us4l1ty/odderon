#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="$HOME/.local/rust/bin:$HOME/.cargo/bin:$PATH"

echo "[Odderon] Running workspace deterministic physics & geometry tests..."
cargo test --workspace

echo "[Odderon] Compiling odderon-wasm -> wasm32-unknown-unknown (release)..."
cargo build -p odderon-wasm --target wasm32-unknown-unknown --release
mkdir -p web/wasm
cp target/wasm32-unknown-unknown/release/odderon_wasm.wasm web/wasm/odderon_wasm.wasm
ls -lh web/wasm/odderon_wasm.wasm
echo "[Odderon] Build complete."
