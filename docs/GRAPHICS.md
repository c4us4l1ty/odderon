# Odderon — The Renderer: Analytical Perceptual Reconstruction (APR)

**Status: authoritative for pixels.** This is the newest rendering document and it supersedes the
pixel-pipeline content of `PERFORMANCE.md`. If any other document disagrees with this one about frame
cost, resolution, draw calls, shadows, textures, bandwidth or GPU strategy, **this one is right and the
other one is stale.**

It is also, deliberately, a **hypothesis with tests attached**. Nothing below has been measured on the
current codebase, because the current codebase does not exist yet. Every claim here is falsifiable by
one of the three tests in §5, and a claim that fails its test is a bug with a number on it — not a
design concession.

Companion docs: [`VISION.md`](VISION.md) (thesis), [`ART-DIRECTION.md`](ART-DIRECTION.md) (the look and
its calibration), [`PERFORMANCE.md`](PERFORMANCE.md) (platform doctrine: no game loop, audio latency),
[`IMMERSION.md`](IMMERSION.md) (no loading screens).

---

## 0. The position

**Do not build a weaker DLSS. Build a different rendering paradigm.**

DLSS exists because a conventional engine is *mathematically blind*. It throws millions of
disconnected triangles and compressed bitmap textures at the screen, traces one noisy ray per pixel,
and then uses a neural network to guess where the edges, shadows and bounce light were supposed to be.
It is an expensive apology for having discarded the maths.

Odderon never discards it. With zero pre-made meshes and zero bitmap textures, the Rust kernel holds
the exact, continuous equation of every surface, curve, material and light in the scene. You do not
need a network to guess where a sub-pixel edge or a shadow penumbra is when you have its analytical
derivative in closed form.

**That is the whole thesis of this document: an engine that knows its own geometry can replace
reconstruction with evaluation.**

---

## 1. Know the silicon

Two chips, two architectures that fail in opposite ways. The target is set by the project, not by
convenience: **Intel HD 520+ (and UHD 620) on a 2015-class laptop, and Android 8+ on a HiSilicon
Kirin 710 (Mali-G51 MP4)**. Both sit in a **4 GB total system RAM** envelope.

| Metric | Intel HD 520 (Skylake GT2) | ARM Mali-G51 MP4 (Kirin 710) | The architectural consequence |
| --- | --- | --- | --- |
| Rendering architecture | Immediate-mode (IMR) | Tile-based deferred (TBDR) | HD 520 writes every draw straight toward system memory; Mali renders 16×16 tiles in on-chip SRAM before writing once. |
| Compute | ~190 GFLOPS (24 EUs) | ~120 GFLOPS (Bifrost, 4-wide clauses) | Both have ample ALU for analytical equations **if you do not stall them waiting on memory.** |
| Memory bandwidth | ~15–25 GB/s (shared DDR3L/DDR4) | ~10–14 GB/s (shared LPDDR3/LPDDR4) | **Bandwidth is the #1 killer.** Deferred shading, SSAO, bloom and TAA will drop both chips to a slideshow. |
| Usable RAM for the app | < 350 MB of 4 GB | < 250 MB of 4 GB | No 4K textures. One uncompressed 4K PBR set (85 MB) is a third of the entire budget. |

> **The golden rule: trade memory bandwidth for ALU.**
> On both chips, reading a texel from system memory costs roughly two orders of magnitude more energy
> and time than evaluating a polynomial or trigonometric expression inside a register. Every decision
> below is that trade, made explicitly.

**Precision note (Mali):** FP16 (`mediump`) runs at roughly twice the rate and half the register
pressure of FP32 on Bifrost. Camera-local coordinates are small enough that all lighting and colour
math can run in native `mediump`. The exceptions are known and must be `highp`: the sun disc test
(§ART-DIRECTION 5), sky gradient near the horizon, and any accumulated temporal buffer.

---

## 2. Five rendering innovations

### Innovation 1 — Thermal-visual unification (zero-cost global illumination)

Why does the reference look alive rather than flat? Indirect bounce: sunlight hits green grass and
bounces onto the underside of the porch; it hits warm timber eaves and bounces into the shade; the
blue sky fills every upward-facing shadowed surface.

Lumen-class engines spend a 300 W GPU on this. Odderon gets it for near-zero GPU time, because the
kernel already models rooms, buildings, spacecraft and habitats as **thermal networks exchanging
radiative heat via view factors**:

```
B_i = ρ_i ( E_i + Σ_j F_{i→j} B_j )
```

That is the radiosity equation, and it is identical for thermal infrared and for visible light. The
consequence is unusual and worth stating plainly: **the gameplay pillar "the apartment is a
spacecraft" and the rendering pillar "free global illumination" are the same code.**

The mechanism:

1. The world is clean parametric surfaces (walls, roofs, ground planes, cylinders), so view factors are
   computable.
2. The Rust kernel computes low-order spherical-harmonic (L1/L2) radiosity transfer **only when
   geometry or sun angle changes** — a few times a second at most, sub-millisecond on CPU.
3. Each surface receives two small SH vectors. Evaluating multi-bounce bounce light in the shader is
   then a handful of multiply-adds and **zero rays**.

This replaces the hand-tuned `skyFill` / `groundFill` constants in `ART-DIRECTION.md` §3 with a
computed version of the same idea — but the hand-tuned values stay as the **calibration target**: if
the solver disagrees with them by a lot, one of the two is wrong, and that is a useful signal.

### Innovation 2 — Analytical contact-hardening shadows (zero shadow-map memory)

Look at the power-line and leaf shadows in the reference. Where the AC unit meets the wall the shadow
is razor-sharp; where a wire is five metres from the wall the edge is soft — because the Sun is a
0.53° disc, not a point.

Conventional engines spend 64 MB on a 4096² shadow map and still show pixelated staircasing. Odderon
rasterises **nothing**: every eave, truss, power line (a catenary) and AC box is a primitive, so each
surface cluster carries a tiny **analytical occluder list** (a few KB of uniform buffer) and the shader
computes the exact signed solid-angle intersection between the **0.53° solar cone** and the primitive.

The penumbra width falls out of the geometry — `w = d_occluder · tan(0.53°)` — so you get
infinite-resolution, noise-free, contact-hardening shadows that never alias, even a millimetre from the
wall, using **0 MB of texture memory**. `ART-DIRECTION.md` §2 documents the box-occluder model this
generalises; the two must agree.

### Innovation 3 — Analytical filter integration (sharpness beyond native 4K)

Why do games need TAA or DLSS? Because when a thin wire or a corrugated sine profile
(`z = A sin(ωx)`) becomes smaller than a pixel, rasterisation point-samples it and you get flicker and
jaggies.

Odderon integrates instead. Because the pattern is a known function `f(u)` with a known integral
`F(u)`, and because WebGL2 gives screen-space derivatives free (`dFdx`/`dFdy`), the shader knows the
exact width `Δu` of the current pixel on the surface and evaluates the exact average across the
footprint:

```
f̄(u) = [ F(u + ½Δu) − F(u − ½Δu) ] / Δu
```

Corrugated walls, wooden slats, dial markings and distant wires then never shimmer, never alias and
never blur — at one sample per pixel. This is mathematically cleaner than DLSS because it is not a
guess; it is the integral of the light hitting that pixel.

**The cost of this innovation is that every procedural pattern must ship with its antiderivative.** A
material that cannot be integrated is a material that will shimmer, so it is not allowed in the
catalogue. That constraint belongs in the material definition, not in a later fix.

### Innovation 4 — Procedural micro-surface synthesis (the 64-byte material)

Weathered wood, painted metal, concrete and grass have to feel tactile with no image texture. So a
material is a **64-byte parametric recipe** — multi-scale analytical noise and cellular functions
evaluated only where visible:

- **Weathered cedar (shopfront):** cylindrical domain-warped sine rings, anisotropic fibre specular via
  a Kajiya-Kay analytical highlight, AO darkened into the plank joints.
- **Corrugated galvanised steel (the left building):** analytical integral of `sin(ωx)` normals (§3
  above), Voronoi crystal spangle, and vertical rain-streak oxidation *gradient derived from the roof
  overhang distance* — because in reality rust only streaks where rain drips off an overhang, and the
  CAD geometry already knows where that overhang is.
- **Summer grass (foreground):** instanced quadratic Bézier blades animated by a travelling
  wind-wave equation in the vertex shader, shaded with an analytical translucent subsurface term when
  backlit.

**Corollary for the material system:** a material is defined by wavelengths, amplitudes, phases and
physical state, never by pixels. A material that needs a bitmap is rejected by test R1.

### Innovation 5 — The "living still" temporal cache (near-zero idle GPU)

In Odderon the screen is usually temporally coherent: a desk, a pad, a console. Conventional engines
redraw 100 % of the screen 60 times a second and heat the laptop while you stare at a gauge.

The **spatio-temporal tile scheduler**:

1. The screen is logically divided into 32×32 tiles.
2. The kernel knows exactly which tiles intersect a moving thing (swaying grass in the bottom fifth,
   the AC fan, drifting cloud, a needle moving through two degrees).
3. Static tiles are retained and skipped via hardware scissor/stencil masking.
4. Better: for static tiles, the first ~8 frames after motion stops accumulate jittered sub-pixel
   samples — so the moment you stop moving, the image sharpens to roughly 8× supersampled quality,
   and GPU usage then drops to a few percent, updating only grass and cloud.

This is the renderer's half of `PERFORMANCE.md` I1/I3: the sim already never runs a loop, and this is
how the presenter also stops.

---

## 3. One scene, two hardware paths

Both chips share the scene representation and the mathematics, and branch at initialisation.

```
                  ┌──────────────────────────────────────────┐
                  │   RUST ANALYTICAL SCENE & THERMAL GRAPH  │
                  │  (Parametric Surfaces + SH Bounce Light) │
                  └────────────────────┬─────────────────────┘
                                       │
              ┌────────────────────────┴────────────────────────┐
              ▼                                                 ▼
┌───────────────────────────────────┐             ┌───────────────────────────────────┐
│   PATH A: INTEL HD 520 (laptop)   │             │    PATH B: MALI-G51 (Android)     │
│    Immediate-mode architecture    │             │     Tile-based (TBDR) silicon     │
├───────────────────────────────────┤             ├───────────────────────────────────┤
│ 1. Zero-overdraw pre-pass:        │             │ 1. Strict single-pass forward:    │
│    a compact visibility/depth     │             │    NEVER flush off-screen buffers │
│    buffer first, so heavy         │             │    to system RAM. Keep shading    │
│    procedural shaders never run   │             │    inside on-chip 16×16 SRAM.     │
│    on occluded pixels.            │             │                                   │
│ 2. Perceptual adaptive shading:   │             │ 2. FP16 (`mediump`) fast-math:    │
│    high-contrast edges at native  │             │    coordinates are camera-local,  │
│    pixels; smooth interiors at    │             │    so lighting and colour run in  │
│    1:4 with bilateral, edge-aware │             │    native FP16 at 2× rate and     │
│    interpolation.                 │             │    half the register pressure.    │
│ 3. Dirty-rect scissor updates:    │             │ 3. Tile transaction elimination:  │
│    when the camera is still, only │             │    identical analytical sky/wall  │
│    redraw the bounding boxes of   │             │    tiles hit Mali hardware CRC    │
│    moving primitives.             │             │    elimination — 0 bytes written  │
│                                   │             │    to LPDDR4.                     │
└───────────────────────────────────┘             └───────────────────────────────────┘
```

**Why Path B is the interesting one.** Mali compares a hardware CRC of each finished 16×16 tile
against the previous frame and skips the write on a match. This normally almost never fires, because
real renderers dither, jitter and temporally accumulate — every tile differs by a few LSBs. Odderon's
static surfaces are *noise-free analytical evaluation*, so a tile where nothing moved is **100 %
bit-identical** and the hardware eliminates it for free, cutting memory-bus power to near zero.

That is a real dependency, and it is the reason dithering is forbidden on static surfaces: **dithering
would cost us the phone's battery.** Any temporal noise must be confined to surfaces already declared
moving.

---

## 4. The memory budget

On a 4 GB machine, crossing roughly ~400 MB gets the tab swapped or killed by the Android low-memory
killer. The envelope:

| Subsystem | Allocation | How it stays small |
| --- | --- | --- |
| WASM binary | ~3.5 MB | Pure Rust, no framework bloat |
| Catalog octree + ephemeris | ~24 MB | Active spatial chunk + Chebyshev polynomials for the solar system |
| Physics & provenance heap | ~32 MB | 1-D fluid/thermal graphs, analytical orbital tables, CAD state |
| Parametric mesh cache | ~18 MB | Generated from CAD equations; LRU-evicted out of view |
| Texture memory | ~0.5 MB | Zero asset textures; one SDF glyph sheet for instrument typography |
| Framebuffers | ~24 MB | Colour + depth + one temporal accumulation buffer |
| WebAudio buffers | ~4 MB | Procedural sound ring buffers |
| **Total engine footprint** | **~106 MB** | Leaves > 3.8 GB to the OS and browser |

Ceiling: **≤ 180 MB desktop / ≤ 120 MB mobile resident**, gated (`GAME-DESIGN-BIBLE.md` §13.4).

---

## 5. Falsifiable rendering tests

Three tests, enforced in code review and in CI. These are the whole of "is the renderer honest".

**R1 — The zero-bitmap law.**
Search the repository and the network tab for `.png .jpg .jpeg .webp .dds .ktx .obj .fbx .gltf
.glb`. If any exists outside the single SDF glyph sheet, **the build fails.** Every visual surface must
be a function `S(u, v, t)` derived from physical state.

**R2 — The static-camera power test (the painting test).**
Load the summer-noon street on Reference A at output resolution. Take your hands off the input. Within
250 ms of the last change, frame time must fall **below 1.5 ms** (< 10 % GPU load), and edge quality
must be indistinguishable from a supersampled offline render of the same scene. *If it does not settle,
the temporal cache is lying about what is static.*

**R3 — The single-pass bandwidth ceiling (the Mali test).**
Profile a frame on Reference B. External memory bandwidth must never exceed **32 bytes per screen
pixel** (one colour write + one temporal read/write). Any effect that requires flushing a full-screen
intermediate to RAM and reading it back is forbidden — which is what rules out SSAO, bloom, and a
conventional TAA chain before anyone proposes them.

A fourth, project-level gate lives in `PERFORMANCE.md` and `IMMERSION.md` N9: **no shader compiles
after boot, no synchronous GPU stalls in the loop, no allocation in the frame path.**

---

## 6. What this buys, and what it costs

**Buys:** a world that looks like a Makoto Shinkai film on a $150 2015 laptop and a mid-range 2017
phone; a 2-second load on a poor connection; zero-bitmap download; near-zero idle GPU; and a renderer
whose correctness is a property of the maths rather than of a neural network's approximation.

**Costs, stated honestly:**

| Cost | Consequence | Mitigation |
| --- | --- | --- |
| Every procedural pattern must ship an antiderivative | Material authoring is harder than texture painting | Bake the antiderivative into the material definition; reject non-integrable materials at the catalogue boundary |
| Every occluder must be an analytical primitive | No shadow from arbitrary geometry | Geometry is parametric by law; if it cannot be described as a primitive, it cannot occlude |
| Static surfaces must be noise-free | No dithering on statics | Confine noise to declared-moving surfaces (§3) |
| FP16 lighting | Precision artefacts at high dynamic range | `highp` for sun disc, sky gradient, temporal accumulation |
| Two GPU paths | Two shader sets to maintain | The analytical scene is shared; only the execution strategy branches (§3) |
| **The whole document is unmeasured** | It may not hold on real silicon | §5's tests, run on Reference A and B before any further rendering work |

That last row is the one to take seriously. The previous build was abandoned partly because its
performance claims were never measured against the quantities the hardware actually charges for.
**No number in this document is a result until it appears in a benchmark run on Reference A and
Reference B, with the misses included.**
