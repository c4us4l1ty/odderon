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
Load the summer-noon street on Reference A, **shading at no more than 1.15 megapixels** (CSS-upscaled
to the display, see the resolution clause below). Take your hands off the input. Within 250 ms of the
last change, no further frame is drawn at all, and frame cost must fall **below 1.5 ms** (< 10 % GPU
load). Edge quality must be indistinguishable from a supersampled offline render of the same scene.
*If it does not settle, the temporal cache is lying about what is static.*

**R2's resolution clause, MEASURED AND CORRECTED.** An earlier revision of this section named 1.15 Mpx
as the clause. That number was an assumption, and the Reference B measurement falsified it: a single
texture read costs **2.29 ms at 1.04 Mpx**, so at 1.15 Mpx a pure blit would be 2.53 ms — over budget
before any shading happens. The clause is therefore **0.68 Mpx**, derived from the measurement rather
than chosen, and what it buys can be checked by hand.

**R2's resolution clause, and why it had to exist.** R2 was written as a time with no pixel count, and
that makes it unfalsifiable rather than strict: frame cost is very nearly linear in pixels, so "1.5 ms"
describes an entirely different shader at 0.5 Mpx than at 4 Mpx. Measured on Reference B (Mali-G51),
34.50 ms at 0.96 Mpx is 36 ns/pixel, which is roughly 340 ALU operations per pixel; a 1.5 ms budget is
roughly **15**. Fifteen operations is a flat colour, or a counted few layers — not analytic shadows,
corrugation, aerial perspective, tonemap and grain. So R2 is satisfiable and unsatisfiable depending
only on resolution, which makes it a policy statement wearing a hardware test's clothes. The clause
names the budget: **0.68 Mpx**, i.e. roughly 825x825, or a phone at 0.81x device pixels.

**R2 and R3, measured against each other.** These were written independently and had never been
checked against each other. They are not in conflict, but the margin is narrow and worth stating.
Measured on Reference B at 1.04 Mpx:

| pass | measured | ratio to a flat fill |
| --- | --- | --- |
| flat fill, no memory traffic | **0.67 ms** | 1.0x |
| **"blit" — actually a texture read of the cache** | **1.25 ms** | 1.9x |
| one texture read into the same target | 1.58 ms | 2.4x |

**This table is retracted.** A reference-device run on 2026-11-24 (Mali-G51, 1.04 Mpx) measured the
shipped texture-read pass at **1.65 ms**, and repeated the earlier **1.27 ms blit** and **0.67 ms flat
fill**. The shipped pass does **not** pass R2: 1.65 ms exceeds the 1.5 ms budget at native resolution,
before any shading, interpolation or input. `1.25 ms` is **withdrawn** — it is not reproducible on the
device and path it claims to describe, and the row it came from ("blit — actually a texture read") was
one of the two *identical* operations the next section describes.

What the same run measured instead:

| pass at 1.04 Mpx | measured | ratio to flat fill |
| --- | --- | --- |
| flat fill, no memory traffic | **0.67 ms** | 1.0x |
| `blitFramebuffer`, no shader | **1.27 ms** | 1.9x |
| **texture read — the pass `web/index.html` actually runs** | **1.65 ms** | 2.4x |

So the shipped per-frame pass costs **1.59 ms per megapixel**, and 1.5 ms buys **0.94 Mpx**, a render
scale of **0.95x** on this screen. A hardware blit would pass at 1.27 ms; the renderer does not perform
one. **The gap between 1.27 and 1.65 ms is the whole of R2's margin on this device**, which is why
`blitFramebuffer` versus texture read stopped being a detail and became the decision.

**The bandwidth ceiling derived from 1.25 ms goes with it.** Subtracting the flat fill from 1.25 ms left
0.58 ms for 8 bytes/pixel and gave **~11 B/px**; from the measured 1.65 ms it is 0.98 ms and **~6.5
B/px** at native resolution. Neither figure is quoted as a result: the whole calculation is a
difference between two microbenchmark rows on a device whose timer resolves 125 us, and it is being
re-derived from a number (`1.25 ms`) that does not reproduce. What survives is the shape of the
argument — **R3 is not the binding constraint at native resolution; R2 is**, and it binds on the
texture read rather than on bandwidth. *An earlier revision of
this section claimed R2 and R3 were incompatible, based on a 2.29 ms texture-read figure that turned out
to be measuring a different operation from the one the renderer actually performs. The correction is
recorded rather than quietly deleted.*

**Three floors, and they are not interchangeable.** A flat fill, a texture read, and a hardware
framebuffer copy are three different operations. An earlier note treated "one texture read" as the cost
of the blit and concluded the cache was insufficient. **That correction was itself wrong in its
explanation, and the error is corrected here rather than deleted.**

This section previously said the cached pass is *cheaper* than an ordinary texture read "because the HUD
samples a framebuffer's colour attachment rather than a general texture." That sentence describes a
mechanism the renderer does not use and a cost difference that was never measured as such:

- **There is no `blitFramebuffer` in `web/index.html`.** The per-frame path is
  `gl.bindTexture(TEXTURE_2D, cache.tex)` followed by a fragment shader whose entire body is
  `texture(uShaded, uv)`. The word "blit" in this document has always been a nickname for *a shader that
  samples the cache*. It is not a blit.
- **The reason given does not exist.** Once a colour attachment is bound as a `sampler2D` it *is* a
  texture; there is no cheaper sampling path being taken. The two "floors" in `web/ablate.html` were
  byte-identical GLSL differing only in which texture object they sampled — so any cost difference
  between them was memory placement, not mechanism.
- **The measured gap is below the instrument.** The first real-hardware run of the fixed ablation page
  (a laptop, 2026-11-24, timer tick 63 µs, instrument error bar 2.8%) read **0.25 ms** for the texture
  read and **0.24 ms** for the "FBO→screen blit" at 1.86 Mpx. That is **10 µs — 0.16 of one tick** —
  against spreads of 49% and 17%. It is not a finding, and the old paragraph explained it anyway.

**What changed in the instrument.** `web/ablate.html` now has a row that performs an actual
`blitFramebuffer`, which runs no fragment shader at all, and it says so if the browser refuses. That
makes "texture read vs blit" a question that can be answered instead of one that was assumed.

**The two Reference B numbers, and why the derivation no longer stands.** The table above quotes
**1.25 ms** for "the cached frame" and **1.58 ms** for one texture read at 1.04 Mpx. Those are now known
to be the same operation measured two ways, 33% apart, and the document cannot say which is the shipped
path's cost. That matters, because the bandwidth ceiling was derived *from* 1.25 ms:

| floor used | ms above flat fill per 8 B/px | 32 B/px would cost | R2 ceiling |
| --- | --- | --- | --- |
| 1.25 ms (as previously used) | 0.0725 | 2.99 ms | **~11 B/px** |
| 1.58 ms (a texture read) | 0.1138 | 4.31 ms | **~7 B/px** |

The conclusion *"R3 is not the binding constraint at native resolution"* holds under both, and holds
more strongly under the second. But **the specific figure ~11 B/px is not supported** and is withdrawn
until the shipped per-frame path is measured as itself, by `web/index.html`'s own timer query rather than
by a microbenchmark. Note also that 1.58 ms **exceeds the 1.5 ms budget**, so if the texture read is the
true cached cost at 1.04 Mpx then R2 does *not* pass on Reference B with 17% to spare — it fails.

**Answered, 2026-11-24.** That question was: is the cached cost the 1.25 ms or the 1.58 ms figure? The
answer is **the larger one, and it is worse than 1.58 ms.** A reference-device run measured the shipped
texture-read pass at **1.65 ms** at 1.04 Mpx — against 1.27 ms for a `blitFramebuffer` the renderer does
not perform. The two "identical operations" in this section turn out to have been *not* identical in cost
after all: the blunt floor row (`outColor = vec4(texture(...).rgb, 1.0)`, no other work) reads 1.25 ms,
and the ablator's own texture-read row, run on the same device in the same session, reads 1.65 ms. That
33% is not yet explained and is not being explained away; what is settled is that **R2 fails at native on
the shipped path**, on every number either run produced above 1.5 ms.

**Two things would restore it,** and they are different projects:

1. **Make the blit a real `blitFramebuffer`.** 1.27 ms at native, which passes. `web/index.html` would
   have to cross framebuffer objects instead of sampling the cache as a texture. Both ablator rows now
   measure a genuinely different mechanism, so the before/after would be checkable.
2. **Move R2's resolution clause from 1.15 Mpx to ~0.94 Mpx.** The clause is `MAX_PIXELS = 1_150_000` in
   `web/index.html` and its own comment says it enforces the documented fallback — but the documented
   fallback is **0.68 Mpx**, and the number most defensible from this measurement is **0.94 Mpx**. The
   code and the doc have been disagreeing on this for some time; the measurement picks a side, and it is
   not the one in the code.

`BUILD-PLAN.md`'s "R2 passes at 1.25 ms with 17% margin" is withdrawn for the same reason.

Any future measurement of the per-frame path must measure that path — with the shader the renderer
actually runs, on the buffer the renderer actually reads.

**What 1.5 ms at 0.68 Mpx is for.** It is the *idle* budget — what a settled frame costs, which on a
render-on-demand renderer is zero because nothing is drawn. The cost of a frame that is actually drawn
is a separate quantity and the honest target is the one the ablation matrix measures per scene, not a
single global number. Do not use R2 to justify a per-moving-frame budget; that is R3's job.

**R3 — The single-pass bandwidth ceiling (the Mali test).**
Profile a frame on Reference B. External memory bandwidth must never exceed **32 bytes per screen
pixel** (one colour write + one temporal read/write). Any effect that requires flushing a full-screen
intermediate to RAM and reading it back is forbidden — which is what rules out SSAO, bloom, and a
conventional TAA chain before anyone proposes them.

A fourth, project-level gate lives in `PERFORMANCE.md` and `IMMERSION.md` N9: **no shader compiles
after boot, no synchronous GPU stalls in the loop, no allocation in the frame path.**

**How these are measured, since that is where this document previously misled itself.** The build ships
its own instruments, because the sandbox that compiles the code has no GPU and a timing from it would
be a fabrication:

| instrument | answers | how it stays honest |
| --- | --- | --- |
| `web/index.html` + HUD | what a frame costs on real silicon | labels every number `gpu` / `stalled` / `submit only`; never presents submit time as frame time |
| `web/ablate.html` | which feature costs what, and at what resolution R2 fits | removes one feature at a time from the *real* shader, extracting it at runtime so a copy cannot drift. **Interleaves the variants over 7 rounds, reports a median and a spread, and re-measures the baseline as a control row whose disagreement with the baseline is the error bar.** Refuses to print a resolution verdict when that error bar exceeds 10%. |
| `web/shadow-check.html` | whether the soft shadow agrees with physics | brute-force 256-ray sun as the reference; measures the penumbra rather than deriving it; chooses the tap count from a sweep |

Three findings from those instruments are recorded here because they are the kind of thing this
document got wrong by assertion:

1. The penumbra of a 0.78 m occluder is **23 mm**, about 7 screen pixels — not the 92 mm an earlier
   note claimed, which had multiplied the sun's angular size by the distance from the camera rather
   than from the occluder.
2. The soft shadow's taps were offset in **world XY**, which is only the plane of the solar disc when
   the sun is overhead. The penumbra was therefore sharp along one axis and soft along another, and it
   looked entirely plausible. Measured worst error against the brute force: 0.3177, versus a
   quantisation floor of 0.0833.
3. Two exact closed forms for that shadow were attempted and **withdrawn with numbers**: a
   parallelogram (worst error 1.0000 — the shadow of a resting box is its footprint Minkowski-summed
   with a segment, i.e. a hexagon) and a correct hexagon (0.3038 — the Minkowski identity assumes
   parallel light, and the receiver here is metres away, not at infinity). Neither shipped.
4. **The matrix disagreed with itself by 43%, which made five of its own rows meaningless.** The
   "full shader (as shipped)" row read **3.00 ms** on Reference A and the "resolution 1.00× (native)"
   row read **1.71 ms** — the same shader, same resolution, same device. Rows ran once each in
   sequence, so a row measured late was compared against one measured early and the clock drift between
   them was printed as a feature cost. Five ablations then landed within 2% of each other at ~1.5 ms,
   which is not five features costing the same: it is the instrument reporting its own drift. The rows
   are now **interleaved over 7 rounds with a control row**, and the page prints **no verdict at all**
   when the control disagrees with the baseline by more than 10%. Every delta in the table above under
   ~40% is **unmeasured**, and the large ones (shadow, taps, resolution) survive only because they are
   bigger than the bias.

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
