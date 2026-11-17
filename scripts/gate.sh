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

step "v4 oracles: boil-off coupling, fitted-mass trade, versioned compatibility"
# Named gates from the v4 plan, run explicitly so a renamed or deleted oracle
# fails the sweep instead of silently reducing coverage.
cargo test --locked --offline -p odd-systems --test v4_coupling \
    oracle_v4_boiloff_couples_to_mass_and_flameout
cargo test --locked --offline -p odd-verify --test v4_trade_study \
    oracle_v4_fitted_mass_trade_study
cargo test --locked --offline -p odd-verify --test v4_tank_volume \
    oracle_tank_volume_zero_discrepancy
cargo test --locked --offline -p odd-systems --test v4_resumption \
    oracle_true_single_segment_resumption_cost
cargo test --locked --offline -p odd-worker --test v4_resumable_segment \
    oracle_true_single_segment_resumption_cost
cargo test --locked --offline -p odd-verify --test fixtures_v1_v2_v3 \
    oracle_v1_v2_v3_backward_compatibility

step "server authority: local HTTP/SQLite adapter + anti-cheat oracles (no cloud credentials)"
cargo clippy --locked --offline -p odd-worker --all-targets --features local -- -D warnings
cargo test --locked --offline -p odd-worker --features local --lib --tests
cargo build --locked --offline -p odd-worker --features local --bin odderon-authority --example segment_chain
# The browser gates replay a coupled v4 flight, so they use the optimized
# authority: an unoptimized replay of a six-day cryogenic coast exceeds the
# client's submission budget and would fail for the wrong reason.
cargo build --locked --offline --release -p odd-worker --features local --bin odderon-authority

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

step "WP 1.12 onboarding, privacy, analyzer and status oracles"
cargo test --locked --offline -p odd-app --lib \
    oracle_playtest_consent_privacy_and_export_analyzer
node --test --test-name-pattern='oracle_(lesson7_schema5_redesign_threshold|placement_diagnostic_routing|playtest_consent_privacy_and_export)' \
    tests/browser/onboarding.test.js
python3 scripts/check-status-sync.py

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
