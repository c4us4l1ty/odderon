# Odderon

Odderon is an early-stage, scientifically grounded space engineering game.
The current code includes an interactive Earth–Moon observatory and an
experimental orbit-rescue planning prototype; the full game is not yet complete.
The project vision and proposed design live in [BRAINSTORM.md](BRAINSTORM.md),
[ARCHITECTURE_LAWS.md](ARCHITECTURE_LAWS.md), [blueprint.md](blueprint.md),
[UI.md](UI.md), and [ELO.md](ELO.md).

# One principle should govern: Odderon's ambition is civilization-scale. Its execution must be bottleneck-scale.

## Current implementation

Phase 0 is accepted as a foundation with explicit scientific, scope, and
performance limitations; see [Phase 0 status](docs/phase-0-status.md).
WP 1.1 adds ideal two-body orbital elements and transfer calculations; see
[orbital-elements notes](docs/orbital-elements.md) for validation and limits.

The WP 1.3/1.4 experimental mission loop adds a Rust-backed Visual Physics
Lens and causal warning DAG, multi-node maneuver planning, locked predictions,
impact handling, numerical perigee-kick targeting, exact Shapley error
attribution, isolated finite-burn loss and three playable mission scenarios,
including the cislunar Contract 103. It remains a simplified prototype with
explicit model, performance and scoring limits; see
[mission-planning feasibility](docs/mission-planning-feasibility.md).

Phases 1–3 add a persistent Aerospace Flight License and signed flight records:
`crates/odd-identity` derives a 12-word mnemonic, an `ODD-` flight key and an
RFC 8032 Ed25519 keypair from one 128-bit secret, and `crates/odd-report` signs a
canonical flight data recorder and emits a deterministic print-ready dossier with
a vector QR verification payload. The browser persists the license and ranked
Astro-Elo locally, certifies each contract once, and can verify any shared
recorder by checking the signature and re-running the flight bit for bit. Identity
is local to one browser, not key custody; see
[mission-planning feasibility](docs/mission-planning-feasibility.md) for the gate
map and the limits.

Work Package 0.1 provides SI quantities, epochs, UTC conversion, hierarchical
reference-frame snapshots, and versioned hashing in `crates/odd-core`.
[Foundation notes](docs/foundation.md) describe the scientific assumptions,
accuracy and coverage limits, and corrections to unrealistic precision claims.
Work Package 0.2 adds a bounded real JPL Earth–Moon ephemeris and a
lower-accuracy long-range analytical fallback; see
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

After building and serving the WebAssembly module as above, open
<http://localhost:8000/mission.html> for the three experimental mission
contracts. Contract 103's single-burn example demonstrates the perigee-kick
trap; its splitter targets actual numerical periapses.
