#!/usr/bin/env bash
# Full local acceptance sweep. Mirrors .github/workflows/rust.yml as closely as
# the sandbox allows: the bundled toolchain has no rustdoc, so doctests run in
# CI rather than here.
set -euo pipefail
cd "$(dirname "$0")/.."
source scripts/env.sh

step() { printf '\n=== %s ===\n' "$*"; }

step "cargo fmt"
cargo fmt --all -- --check

step "clippy -D warnings"
cargo clippy --locked --offline --workspace --all-targets -- -D warnings

step "workspace tests (lib + integration)"
cargo test --locked --offline --workspace --lib --tests --no-fail-fast

step "server authority: local HTTP/SQLite adapter + anti-cheat oracles (no cloud credentials)"
cargo clippy --locked --offline -p odd-worker --all-targets --features local -- -D warnings
cargo test --locked --offline -p odd-worker --features local --lib --tests
cargo build --locked --offline -p odd-worker --features local --bin odderon-authority --example segment_chain

step "Cloudflare authority wasm"
cargo build --locked --offline --release -p odd-authority-wasm --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/release/odd_authority_wasm.wasm crates/odd-worker-cloudflare/src/authority.wasm

step "release wasm"
cargo build --locked --offline --release -p odd-view --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/release/odd_view.wasm web/odd_view.wasm

step "core oracle (native + wasm)"
cargo build --locked --offline -p odd-core-oracle
cargo build --locked --offline -p odd-core-oracle --lib --target wasm32-unknown-unknown

step "native / node / chrome parity"
node tools/core-oracle/check-parity.mjs

step "node unit tests"
npm test

step "browser E2E + idle gate"
npm run test:browser

step "assembly E2E (delegation + cryogenic levers, flown and signed)"
npm run test:assembly

step "ranked, public replay, and tamper E2E: native HTTP/SQLite"
npm run test:authority

step "ranked, public replay, and tamper E2E: workerd/D1/R2"
npm run test:authority:cloud

step "D1/R2 concurrency, transactional Elo and retry races"
npm run test:cloud-races

printf '\nAll local gates passed.\n'
