# Odderon

Odderon is an early-stage, scientifically grounded space engineering game.
The current code includes an interactive Earth–Moon observatory, **not yet a playable game**.
The project vision and proposed design live in [BRAINSTORM.md](BRAINSTORM.md),
[ARCHITECTURE_LAWS.md](ARCHITECTURE_LAWS.md), [blueprint.md](blueprint.md),
[UI.md](UI.md), and [ELO.md](ELO.md).

## Current implementation

Phase 0 is accepted as a foundation with explicit scientific, scope, and
performance limitations; see [Phase 0 status](docs/phase-0-status.md).
The next roadmap item is WP 1.1, which is not yet complete.

Work Package 0.1 provides SI quantities, epochs, UTC conversion, hierarchical
reference-frame snapshots, and versioned hashing in `crates/odd-core`.
[Foundation notes](docs/foundation.md) describe the scientific assumptions,
accuracy and coverage limits, and corrections to unrealistic precision claims.
Work Package 0.2 adds bounded, real JPL Earth–Moon ephemerides; see
[ephemeris notes](docs/ephemeris.md) for data provenance, validation, and
orientation limitations. `crates/odd-astro` separately provides ideal two-body orbit calculations and
numerical propagation. These are simplified models, not a flight simulator.

With Rust and the `wasm32-unknown-unknown` target installed:

```sh
cargo test --locked --workspace
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo build --locked --workspace --target wasm32-unknown-unknown
cargo fmt --all -- --check
```

CI additionally compares native and WebAssembly fixtures in Node and headless
Chrome. To run that check locally, build `odd-core-oracle` natively and for wasm,
then run `node tools/core-oracle/check-parity.mjs` with Chrome on `PATH` (or set
`CHROME_BIN`).

## Earth–Moon observatory (WP 0.3)

Build the WebAssembly ephemeris and serve the viewport locally:

```sh
cargo build --locked --release -p odd-view --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/release/odd_view.wasm web/odd_view.wasm
python3 -m http.server 8000 --directory web
```

Open <http://localhost:8000/>. See [observatory notes](docs/observatory.md)
for controls, browser tests, rendering choices, and scientific/performance
limitations. Positions are calculated from DE440s; visible surface features
and the optional vehicle study are illustrative.
