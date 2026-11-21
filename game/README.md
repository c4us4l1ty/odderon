# Odderon — Block 1: *The Room*

A playable vertical slice of the vision documents. Everything here exists to prove a specific claim
from `docs/` on real hardware, and nothing here exists for decoration.

```sh
./run.sh          # http://localhost:8080
./run.sh test     # 159 assertions, no dependencies
node test/scene-probe.mjs /tmp/room.png 0.75 -0.12 -2.10 -1.60   # render the room on the CPU
node test/vectors.mjs   # the JS kernel's answers, for the Rust kernel to be held to (CI does this)
```

## Controls

| | |
| --- | --- |
| **click** | capture the mouse — the pointer is locked and the view follows your hand |
| **`Esc`** | release the mouse and get your cursor back, and drop every held key |
| `W A S D` | walk (you step up onto the platform, or down the treads) |
| the read-out box | clickable: real UI keeps its click, and never steals the pointer |
| `E` | open / close the curtain |
| `F` | the air conditioner |
| `M` | **grease-pencil the thermometer** — mark where you think the needle will settle |
| `R` | **let reality answer** — resolve the mark and get δ |
| `V` | flip the Eidos visor down / up |
| `P` | provenance of the number on the wall |
| `G` | the ghost twin: what you assumed vs. what happened |
| `H` | help |

## What is actually being proven

| Claim | Where | Verified by |
| --- | --- | --- |
| The apartment is a spacecraft, not a special case | `sim/thermo.js` — one solver, `Model` | `test/physics.test.mjs` |
| The Day-1 experiment is physical | close the curtain at midday: solar gain through 3 m² of glazing falls from ~1440 W to ~120 W, the AC's draw drops, and the room cools | `test/physics.test.mjs` |
| A real AC can **lose** | at midday the 2 kW unit saturates, the room climbs to ~28 °C, and the wall meter reads 669 W instead of 349 W | `test/physics.test.mjs` |
| Expectation is a gesture, not a form | `sim/marks.js` — a mark is 0.5 s, committed to a hash | `test/physics.test.mjs` |
| δ is perceived, not read | the yellow tick on the glass vs. the steel needle | visual |
| Attribution needs no paragraph | the ghost twin + one arrow at `argmax|∂δ/∂xᵢ|` | `test/provenance.test.mjs` |
| Honesty costs no frame rate | `sim/provenance.js` — flat `f64` in the loop, a ~120-node lineage graph on demand | `test/provenance.test.mjs` |
| The kernel is deterministic | `test/golden.test.mjs` runs the same 7200-step simulation in **two separate processes** and compares digests | CI |
| There are no assets | 148 KB of source, no images, no audio files, no meshes | CI's cold-payload gate |
| The sun is where the sun is | `sim/world.js` — NOAA solar position | `test/physics.test.mjs` |
| There is no game loop | `main.js` `_idle()` — when nothing changes, no frame is requested | `test/dom.test.mjs` |
| The shaders are well-formed | `test/glsl.test.mjs` — delimiters, declaration order, GLSL ES 3.00 literal rules, and a march bound of 56 steps | CI |
| The mouse is captured, and Esc returns it | `main.js` — `requestPointerLock` on the scene canvas, not the `pointer-events:none` overlay; `Esc` → `exitPointerLock` → the cursor comes back, and every held key is released | `test/dom.test.mjs` |
| The previews are the same room the shader draws | `test/scene-probe.mjs` — the shader's geometry ported to JS and ray-traced on the CPU, so the room can be *looked at* without a GPU; `test/scene.test.mjs` asserts the composition invariants | `test/scene.test.mjs` |
| The sun actually reaches the floor | `main.js` orients the hab: the window wall faces 270°, so at 16:26 the low sun is 0.85 of the way through the glass. Before this, `+z` was *north* and no sunlight ever entered the room | `test/scene.test.mjs` |
| The instruments are objects, not an overlay | `ui/hud.js` `project()` inverts the shader's own camera, so the gauge is drawn on the plaster at a real position; turn away and it is gone | `test/dom.test.mjs` |
| A still room cannot repaint itself | `sim/schedule.js` — a pure, tested function: no time term in the image, a present quantised to its bucket, idle request rate → 0 | `test/schedule.test.mjs` |
| Rust and JavaScript are the same kernel | `kernel/` compiles in CI (native + `wasm32-unknown-unknown`), and `kernel/tests/parity.rs` replays the vectors that `test/vectors.mjs` extracts from the JavaScript kernel: transcendentals to 1e-14, and 2 h of the thermal solver to 1e-9 K | `.github/workflows/ci.yml` |

## Layout

```
index.html            the whole game is this page + 12 scripts
game/
  sim/
    dmath.js          deterministic transcendentals (no Math.sin anywhere in the kernel)
    rng.js            xoshiro128** over integer state
    provenance.js     the lineage graph: every number can be interrogated
    thermo.js         THE solver. apartment, capsule, lunar base, O'Neill cylinder
    world.js          time, solar geometry, irradiance, sky colour, the event queue
    marks.js          the grease pencil, δ, proper scoring, the epistemic peak
    schedule.js       the frame budget as a pure function: when to present, when not to
  render/
    glsl.js           the scene, as a signed distance field. No textures, no shadow maps
    renderer.js       two draws for the whole room; skips the frame if nothing changed
    audio.js          cicadas, the compressor, its start-up clunk — all synthesized
  ui/
    hud.js            instruments drawn as objects: a thermometer, a meter, a drawer, the visor
  main.js             the camera, the hand, and the render-on-demand scheduler
kernel/               the same kernel in Rust: dmath, the thermal solver, the replay digest
  src/lib.rs          no dependencies, no libm, bit-compatible with game/sim/
  tests/parity.rs     replays the JavaScript kernel's vectors and fails if a bit moved
.github/workflows/    the compiler: native + wasm32 + node + two emulated device classes
```

## How the Rust half is compiled

There is no Rust toolchain on the machine this was written on, so **the compiler is CI**
(`.github/workflows/ci.yml`). Every push:

1. builds `kernel/` natively with `clippy -D warnings` and runs its tests;
2. builds it for `wasm32-unknown-unknown` in two profiles, runs `wasm-opt -Oz`, and **fails the
   build above 512 KB** (the core budget is ~400 KB);
3. regenerates `test/vectors.json` from the **JavaScript** kernel and makes the Rust tests replay it —
   so the two kernels are proven to agree rather than assumed to;
4. runs the node suite on Node 20 and 22, enforces the cold-payload budget, and refuses to let a
   binary asset into the repo;
5. runs the same suite under `cpulimit` at 12% and 25% of a core, as a floor for a throttling Android
   phone and a 2015 UHD laptop.

The wasm artifacts are uploaded per run, so a build from any commit can be downloaded and inspected
without a toolchain anywhere.

## The room, as built (round 3)

The reference is Star Citizen's apartment habs, and what they are actually built from is:

- **a raised platform with wide shallow treads**, not a flat floor. The bed sits up on it; you walk
  up three 160 mm steps to reach it, and the treads wrap round two sides of the platform. This one
  move is most of why the reference reads as designed rather than as a box.
- **light inside the architecture**: a lit strip on every tread nosing, a strip under the platform
  edge, and a bright cove down both sides of the ceiling. `stripLight()` is a light source in the
  shading, not emissive paint.
- **two long horizontal strip windows** above the bed with a heavy frame and three mullions, which
  project a hard-edged pattern of bands and bars across the floor.
- **pale stone below, warm walnut on the platform, white bedding**, pale grey panel walls with
  structural ribs, a dark skirting. That palette is why the room now reads as bright instead of grey.
- a curved glass balustrade, one dark lacquer console, one ochre pipe run hugging a wall, a cube
  seat, a cantilevered desk.

The frame's whole job is the window, the bed and the sun patch, which is what `test/scene.test.mjs`
checks at the spawn point.

## What is deliberately *not* here yet

- **The Rust kernel in the browser.** `kernel/` compiles and passes parity today, but the page still
  loads the JavaScript kernel: 90 KB, zero toolchain, instant start. Wiring the wasm in is Block 2,
  and the parity test is the harness that will prove the swap changed nothing.
- **`wasm-bindgen` glue.** Not needed yet: the first wasm surface will be raw exported functions.
- **Progressive refinement / reprojection** (`PERFORMANCE.md` I2). Block 1 ships the simple path
  first, as the doctrine says: measure the straightforward architecture, then add cleverness only
  where the measurement demands it.
- **A worker + SharedArrayBuffer.** The server already sends COOP/COEP. The split becomes worth its
  complexity when the solver is heavier than a 25 m² room.
- **The static fire.** That is Block 2, and it is where the same solver starts running a turbopump.

## Honest limitations of this build

1. **The shader has never run on a GPU.** This environment has no browser and no GL. Everything that
   could be checked without one is checked, and it has already paid for itself: round 1's geometry
   was re-implemented in JavaScript on the CPU to ray-trace the spawn view, which is how the camera
   pointing at a wall instead of the window was found before shipping. Visual quality is still
   unverified until you run it.
2. **Frame rate: the 1080p60 claim failed on real hardware** — round 1 measured **11 fps** on the
   reviewer's machine at 1880×959, which is exactly why round 2 exists. What changed:
   - the shaded resolution is now capped at **1.35 MP** and is deliberately below the CSS size
     (a ray-marched pixel is expensive; upscaling is free);
   - the march was cut from 56+48 steps to **40+32**, the FBM from 4 octaves to 3, and the shadow
     ray for the sun was replaced by a closed-form window intersection (one division, no march);
   - the per-frame work is now scheduled: a still room is not redrawn, a moving camera always is;
   - a governor drops resolution (not cadence) when the measured frame *cost* is over 26 ms.
   This is still arithmetic until it runs on your laptop. Tick the *frame read-out* box: it now shows
   fps, presents, skips, the framebuffer size and the quality factor, so one screenshot tells me
   which of the four levers is binding.
3. **The AC's thermostat is bang-bang with hysteresis, not a real vapour-compression cycle.** The
   compressor, condenser, expansion valve and evaporator are represented by capacity, COP, inrush and
   duty. Making that a full cycle is exactly what Block 2 does when the same code becomes a turbopump.
