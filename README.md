# Odderon

> **A real aerospace workstation and a real universe, disguised as a summer afternoon.**

**Checkpoint 1A — Block 1: *The Sunlit Room* (`Uchinoura Hab #204`)**

Built in pure **Rust → WebAssembly (`105 KB`) + raw WebGL2 + WebAudio Physical Synthesis**, containing **0 KB of 3D models, textures, or audio files**, engineered to hold **60 FPS on a 2015 Intel UHD laptop (4 GB RAM)**, **30+ FPS on Android 8+**, and **0 scheduled frames/sec (`0.0% CPU / 0.0% GPU`)** when idle.

## Crate Architecture

- `crates/odderon-dmath` — `#![no_std]` bit-reproducible transcendentals (`dsin`, `dcos`, `datan2`, `dexp`, `dln`, `dsqrt`), `i128` nanometer Galactic Sector coordinates (`±100,000+ ly`), analytical Uchinoura solar ephemeris, `Provenance<T>` uncertainty propagation DAG, and deterministic `Proof-of-Flight` hashing.
- `crates/odderon-thermo` — Coupled 1D habitation atmospheric volume, solar glazing/Shoji radiative transfer, Carnot-derated R32 vapour-compression heat pump, and 100V/15A electrical breaker bus with bi-metallic overload trip and analytical Eidos partial derivatives.
- `crates/odderon-geom` — Zero-asset parametric 3D mesh generator (Star Citizen chamfered hab bulkheads & bunk pod, Cyberpunk Japantown raised tatami, Tansu chest, Bonsai & Shoji Andon lantern, Hinoki window engineering desk, CRT scope, analog gauges, corrugated exterior workshops, catenary power lines, and coastal Rocket Pad-1).
- `crates/odderon-sim` — Event-driven deterministic kernel enforcing Law 4 (`Actuator`-only world interaction), $343\text{ m/s}$ acoustic shockwave propagation across $2.82\text{ km}$, China-marker prediction scoring, and replay logging.
- `crates/odderon-wasm` — Zero-per-frame-allocation C-ABI linear-memory bridge to WebGL2 & WebAudio.

## Quick Start

```bash
# Serve the web client immediately (0 npm dependencies required):
npm run dev
# Open http://localhost:3000 (or your LAN IP on an Android 8+ phone)
```

To run the deterministic validation & doctrine test suite and recompile WASM from Rust source:

```bash
rustup target add wasm32-unknown-unknown
./scripts/build-wasm.sh
```
