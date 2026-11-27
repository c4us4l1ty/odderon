# Odderon

> A real aerospace workstation and a real universe, disguised as a summer afternoon.

A browser simulator of astronautics, astrodynamics and space-company economics that runs on a 2015
laptop with integrated graphics — and produces artifacts a real engineer can audit: a committed
prediction, a debrief, a technical report, an exported GMAT script or NASA CEA deck.

**Status: built, block by block.** P0–P5 are all merged to `main`: the spine (P0), the pad (P1), the
mark (P2), flight and comms (P3), the archive (P4), and the failure-design layer — the anomaly
drawer, the logic rack and the E_an rating (P5). P5 now includes the J2+J3+J4 zonal-harmonic
propagator for the rendezvous case, a Dormand–Prince 5(4) adaptive integrator of the full Earth
gravitational field (point mass + J2 + J3 + J4), cross-checked to conserve energy and self-consistent
to under 50 m over 7 days at LEO. The kernel also solves Lambert's problem (two positions + time of
flight → transfer velocities) via the y-parameter universal-variable method, validated against the
Vallado/Curtis published solution to ~1e-9 relative with the 180° degenerate case solved directly,
and models the first-order Schwarzschild periapsis advance (relativistic corrections) — reproducing
the published 43.0″/century precession of Mercury from first principles. The kernel is pure `no_std`
Rust compiled to WASM; the page is a single `web/index.html`; every
block ships a CI gate (`tools/check-*.mjs`) that someone else can run.

**Start here: [`docs/README.md`](docs/README.md)** — the documentation index, in reading order, with an
authority table for which document decides what.

## The argument in four lines

```
   physics gives you an advantage  ->  you learn the physics to win
            ^                                   |
            |                                   v
   a stranger trusts the proof  <-  your work product is real
```

Every educational simulator breaks at the last step: the skill is real but the artifact is a game
score. Odderon's whole bet is that the artifact can be real.

## What is fixed

- **Real physics, no shortcuts.** One kernel; conservation of energy, momentum and mass absolute; no
  FTL; relativity where it matters.
- **Real universe, never invented.** Public catalogue data only, preprocessed offline. What is unknown
  is *shown* as unknown, with error bars — never filled with procedural noise.
- **Zero 3D assets.** Every nozzle, tank, pipe, building and instrument is generated from mathematics.
- **Zero UI.** The control panel is a physical object: valves, throttles, switches and hands. There is
  no button that says "add propellant".
- **Near-zero idle cost.** The universe is a function of time, not a loop. When nothing happens, nothing
  is scheduled.
- **Transferable skill is priority one.** Exports in formats real tools read; ratings verified by
  deterministic replay, not by our word.

## What to build first

Block 1, *The Room* (`docs/GAME-DESIGN-BIBLE.md` §17): wake up, walk, close the curtain, and watch the
room cool because the apartment runs on the same thermal solver as the spacecraft. Then measure the
frame cost on the reference hardware and publish the number, including the misses.
