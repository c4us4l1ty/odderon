# Odderon

Odderon is an early-stage, scientifically grounded space engineering game.
The current code includes an interactive Earth–Moon observatory and an
experimental orbit-rescue planning prototype; the full game is not yet complete.
The project vision and proposed design live in [BRAINSTORM.md](BRAINSTORM.md),
[ARCHITECTURE_LAWS.md](ARCHITECTURE_LAWS.md), [blueprint.md](blueprint.md),
[UI.md](UI.md), and [ELO.md](ELO.md). The implementation and deployment
boundary is summarized in [current project status](docs/project-status.md).

# One principle should govern: Odderon's ambition is civilization-scale. Its execution must be bottleneck-scale.

## Play locally

```sh
git clone https://github.com/c4us4l1ty/odderon.git
cd odderon
./scripts/provision.sh   # first clone only; builds the browser WASM
bun run dev              # Bun-native local game server
```

Open the local URL printed by the server. Odderon opens at its game menu;
`observatory.html` remains available as an in-game reference screen. The core
game build is static and does not depend on a storefront or Cloudflare Pages.
See the [portable game-shell boundary](docs/game-shell.md) for player routes,
packaging constraints and browser acceptance coverage.

## Current implementation

Phase 0 is accepted as a foundation with explicit scientific, scope, and
performance limitations; see [Phase 0 status](docs/phase-0-status.md).
WP 1.1 adds ideal two-body orbital elements and transfer calculations; see
[orbital-elements notes](docs/orbital-elements.md) for validation and limits.

The WP 1.3/1.4 experimental mission loop adds a Rust-backed Visual Physics
Lens and causal warning DAG, multi-node maneuver planning, locked predictions,
impact handling, numerical perigee-kick targeting, exact Shapley error
attribution, isolated finite-burn loss and three playable mission scenarios,
including the cislunar Contract 103. The mission page includes an opt-in,
event-driven seven-lesson guide whose completion checks read the real controls;
progress stays off until the player consents to local storage. A patched
encounter preview exposes lunar B-plane coordinates and a practice targeting
aid. It remains a simplified prototype—not a lunar-capture optimizer—with
explicit model, performance and scoring limits; see
[mission-planning feasibility](docs/mission-planning-feasibility.md).

The Aerospace Flight License and signed flight records are implemented:
`crates/odd-identity` derives a 12-word mnemonic, an `ODD-` flight key and an
Ed25519 keypair from one 128-bit secret. `crates/odd-report` emits a canonical
signed recorder and a print-ready dossier with a vector QR. The browser stores
its license and a cache of receipts; identity remains browser-local, not key
custody.

Astro-Elo is **server-authoritative** when a trusted authority is configured.
The pilot commits the vehicle, maneuver plan and prediction before the honest
browser flies; the authority replays the record and signs the first-attempt
ranked receipt. Later attempts are practice. Callsigns, leaderboard, public
replay and receipt-bearing dossiers are implemented for native SQLite and a
Cloudflare-compatible WASM/D1/R2 shell. The public-key pin is empty by default,
so a static-only deployment offers practice without ranked certification.
See [server-authority](docs/server-authority.md) and the
[Cloudflare deployment prerequisites](crates/odd-worker-cloudflare/README.md).
Local emulator tests and a dry-run do not mean the service is deployed. The
[bug-hunt report](docs/bug-hunt-2026-11-14.md) lists recent fixes, regressions,
compatibility effects and remaining model limitations.

Cryogenic tanks and the four Chief-AI departments are wired into flown designs,
replay and the dossier; Contract 103 offers an optional paid recommended
thermal card. [Thermal notes](docs/thermal.md) describe the coupled v4 boil-off
model, the frozen one-way v1–v3 behaviour, the measured scoring-baseline defect
and objective-reaching manual/delegated tests.

Work Package 1.5 replaces the fixed engine card with a solved liquid engine:
`crates/odd-propulsion` computes chamber properties, nozzle flow, Summerfield
separation, Bartz throat heating and dry mass from a physical design, using
only `f64` and `libm` so native and wasm results are bit-identical. The mission
page exposes it as an Engine CAD & Schlieren lab — drag the throat, exit, bell
length, mixture ratio and chamber pressure on the cross-section itself, sweep
test-stand altitude, and watch the flow separate and re-attach. The signed recorder stores the *design*, and
`crates/odd-verify` recomputes thrust, specific impulse and engine mass from it
before replaying the flight, so a client cannot claim performance its geometry
cannot produce. See [propulsion notes](docs/propulsion.md) for the calibration
anchors, the contour-corrected separation criterion, the plume reconstruction
and the model's limits.

Work Package 0.1 provides SI quantities, epochs, UTC conversion, hierarchical
reference-frame snapshots, and versioned hashing in `crates/odd-core`.
[Foundation notes](docs/foundation.md) describe the scientific assumptions,
accuracy and coverage limits, and corrections to unrealistic precision claims.
Work Package 0.2 adds a bounded real JPL Earth–Moon ephemeris and a
lower-accuracy long-range analytical fallback; see
[ephemeris notes](docs/ephemeris.md) for data provenance, validation, and
orientation limitations. `crates/odd-astro` separately provides ideal two-body orbit calculations and
numerical propagation. Launch-site state and gravity-turn guidance kernels now
exist, but no surface-launch contract or ascent product calls them. These are
simplified models, not a flight simulator.

With Rust and the `wasm32-unknown-unknown` target installed:

```sh
cargo test --locked --workspace
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo build --locked --workspace --target wasm32-unknown-unknown
cargo fmt --all -- --check
```

For the sandbox's full offline acceptance sweep, first run
`./scripts/provision.sh --full` to install its toolchain, vendored dependencies
and headless browser, then run `./scripts/gate.sh`. The gate includes the browser
and both authority adapters; its disposable test seed is not a production key.

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

Open <http://localhost:8000/> for the game menu, or
<http://localhost:8000/observatory.html> for the observatory. See
[observatory notes](docs/observatory.md) for controls, browser tests, rendering
choices, and scientific/performance limitations. Positions are calculated from
DE440s; visible surface features and the optional vehicle study are illustrative.

After building and serving the WebAssembly module as above, open
<http://localhost:8000/mission.html> for the three experimental mission
contracts. Contract 103's single-burn example demonstrates the perigee-kick
trap; its practice-only splitter targets numerical periapses for the assembled
stage. A static-only server cannot issue ranked receipts; ranked requires the
same-origin authority and a pinned public key described above.
