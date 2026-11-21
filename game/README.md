# Odderon — Block 1: *The Room*

A playable vertical slice of the vision documents. Everything here exists to prove a specific claim
from `docs/` on real hardware, and nothing here exists for decoration.

```sh
./run.sh          # http://localhost:8080
./run.sh test     # 107 assertions, no dependencies
```

## Controls

| | |
| --- | --- |
| `W A S D` | walk (mouse to look; click the canvas to capture the pointer) |
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
| There are no assets | 90.8 KB total, no images, no audio files, no meshes | `curl` the server |
| The sun is where the sun is | `sim/world.js` — NOAA solar position | `test/physics.test.mjs` |
| There is no game loop | `main.js` `_idle()` — when nothing changes, no frame is requested | `test/dom.test.mjs` |
| The shaders are well-formed | `test/glsl.test.mjs` — delimiters, declaration order, GLSL ES 3.00 literal rules | CI |

## Layout

```
index.html            the whole game is this page + 11 scripts
game/
  sim/
    dmath.js          deterministic transcendentals (no Math.sin anywhere in the kernel)
    rng.js            xoshiro128** over integer state
    provenance.js     the lineage graph: every number can be interrogated
    thermo.js         THE solver. apartment, capsule, lunar base, O'Neill cylinder
    world.js          time, solar geometry, irradiance, sky colour, the event queue
    marks.js          the grease pencil, δ, proper scoring, the epistemic peak
  render/
    glsl.js           the scene, as a signed distance field. No textures, no shadow maps
    renderer.js       two draws for the whole room; skips the frame if nothing changed
    audio.js          cicadas, the compressor, its start-up clunk — all synthesized
  ui/
    hud.js            instruments drawn as objects: a thermometer, a meter, a drawer, the visor
  main.js             the camera, the hand, and the render-on-demand scheduler
```

## What is deliberately *not* here yet

- **Rust/WASM.** The kernel is written behind module boundaries that mirror the planned Rust crates
  (`odderon-dmath`, `odderon-thermo`, `odderon-sim`), so the port is mechanical. There was no Rust
  toolchain and no Chrome in the environment where this was built, so shipping untested Rust would
  have been worse than shipping tested JavaScript. The port is Block 2 work, and
  `test/golden.test.mjs` is the harness that will prove the port did not change a single bit.
- **Progressive refinement / reprojection** (`PERFORMANCE.md` I2). Block 1 ships the simple path
  first, as the doctrine says: measure the straightforward architecture, then add cleverness only
  where the measurement demands it.
- **A worker + SharedArrayBuffer.** The server already sends COOP/COEP. The split becomes worth its
  complexity when the solver is heavier than a 25 m² room.
- **The static fire.** That is Block 2, and it is where the same solver starts running a turbopump.

## Honest limitations of this build

1. **The shader has never run on a GPU.** This environment has no browser and no GL. Everything that
   could be checked without one — GLSL well-formedness, declaration order, loop bounds, uniform
   wiring, 600 frames of the app against a stubbed WebGL2 — is checked, and one real bug was found
   and fixed that way (the window was modelled as a box inside the wall, so it could never be the
   nearest hit). Visual quality is unverified until you run it.
2. **Frame rate is unmeasured.** The 1080p60 claim in `PERFORMANCE.md` is arithmetic, not a
   measurement. Tick the *frame read-out* checkbox to see it on your machine, and tell me the number.
3. **The AC's thermostat is bang-bang with hysteresis, not a real vapour-compression cycle.** The
   compressor, condenser, expansion valve and evaporator are represented by capacity, COP, inrush and
   duty. Making that a full cycle is exactly what Block 2 does when the same code becomes a turbopump.
