# Odderon

Odderon is an early-stage, scientifically grounded space engineering game.
The current code is a first foundation slice, **not yet a playable game**.
The project vision and proposed design live in [BRAINSTORM.md](BRAINSTORM.md),
[ARCHITECTURE_LAWS.md](ARCHITECTURE_LAWS.md), [blueprint.md](blueprint.md),
[UI.md](UI.md), and [ELO.md](ELO.md).

## Current implementation

`crates/odd-core` contains split TDB epochs and local reference-frame
transformations, including instantaneous rotation and velocity. `crates/odd-astro`
provides ideal two-body circular-orbit, vis-viva, Hohmann transfer, and numerical
velocity-Verlet propagation calculations. These are simplified models, not a
complete flight simulator. See [foundation notes](docs/foundation.md) for model
limitations, references, and the remaining work before Work Package 0.1 is complete.

With Rust and the `wasm32-unknown-unknown` target installed:

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace --target wasm32-unknown-unknown
```

GitHub Actions runs these checks plus `cargo fmt --all -- --check`.
