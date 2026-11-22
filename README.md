# Odderon

> A real aerospace workstation and a real universe, disguised as a summer afternoon.

A browser simulator of astronautics, astrodynamics and space-company economics that runs on a 2015
laptop with integrated graphics — and produces artifacts a real engineer can audit: a committed
prediction, a debrief, a technical report, an exported GMAT script or NASA CEA deck.

**Status: design, restarting from scratch.** No code exists yet. The previous attempt was abandoned;
this repository contains the thinking that replaces it, not its remains.

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
