# Odderon — How we actually build this

**Status: the build plan.** Answers "how are we planning to build this game", given the constraints that
are real rather than aspirational: the hardware, the sandbox, the data rights, and the fact that the last
attempt failed.

Authority: this file decides *how the work is done*. `GAME-DESIGN-BIBLE.md` §17 decides *what is built
and in what order*. `GRAPHICS.md` decides *what a frame costs*. `DATA-LICENSING.md` decides *what data may
ship*. Where this file and another disagree about process, this file wins; where they disagree about
design, they do.

---

## 0. The three facts that shape the plan

**Fact 1 — the hardware is the design brief, not a limitation on it.** Reference A is a 2015 laptop with
an Intel HD 520 and 4 GB of RAM; Reference B is a 2017 mid-range Android (Kirin 710 / Mali-G51). Those
two machines are not "minimum spec". They are the *reason the engine looks like this*: no textures, no
shadow maps, ≤ 8 draw calls, analytical everything. A plan that assumes a good GPU produces the wrong game.

**Fact 2 — the previous attempt died of unmeasured ambition.** It built a renderer, reported a benchmark
it had never taken, and shipped a governor that measured the wrong quantity. The lesson is not "be more
careful"; it is **make every claim a test with a number and a unit, and run it before building the next
thing.** That is why the plan below is a sequence of *measured* increments rather than a sequence of
subsystems.

**Fact 3 — the sandbox and the target are different machines, and the gap is real.** Measured in this
workspace on 2026-11-22:

| | This sandbox | Reference A |
| --- | --- | --- |
| Rust / cargo | **absent**, and no network route to `static.rust-lang.org` / `crates.io` | required |
| Node / npm | present, working | — |
| Chromium | only via `@sparticuz/chromium` (bundle it, unpack with Brotli, `LD_LIBRARY_PATH` to its libs) | — |
| WebGL2 | **works**, but on **SwiftShader** (CPU rasterisation) | real Gen9 GPU |
| GPU profile | not representative; software raster timing means nothing | the only profile that matters |

Consequences, which the plan is built around:

- **Frame timing here is worthless.** SwiftShader can tell me *visually correct* and *what work per pixel*
  — it cannot tell me *how fast*. All performance judgement comes from you, running a build on Reference A
  and B with the frame read-out on screen.
- **My Rust judgement is unverified by a compiler.** Treat compile-clean as unproven until it builds on
  your machine; expect the first pull to need a small fix pass.
- **The frame-cost *instrument* has to ship inside the build**, not live in my sandbox. It is part of the
  product from Block 1, not a dev tool bolted on later.

---

## 1. The shape of the build

### 1.1 One engine, three layers, and the boundary is the whole design

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  odderon-kernel          Rust → WASM.  No DOM. No GL. No clock. No floats    │
│  (deterministic, pure)   taken from the host.  Everything here is a function   │
│                          of time and inputs, and can be re-run bit-identically  │
├──────────────────────────────────────────────────────────────────────────────┤
│  odderon-present         Rust → WebGL2 via `glow` (or hand-written bindings).  │
│  (samples the kernel)    Samples a state ring buffer. May run at 0 Hz.         │
├──────────────────────────────────────────────────────────────────────────────┤
│  odderon-shell           The thin web layer: boot, worker plumbing, tile       │
│  (JS/TS + canvas)        streaming, stroke-font instruments, save/export UI.   │
└──────────────────────────────────────────────────────────────────────────────┘
```

The rule that keeps the whole thing honest: **the kernel cannot see the screen, and the presenter cannot
change the world.** Anything the presenter needs, the kernel must be able to *prove* it is allowed to
skip (I4 in `PERFORMANCE.md`) — which is the same determinism that makes Esse and Proof-of-Flight
possible. One architecture, three payoffs.

### 1.2 Determinism is the first thing built, not the last

`odderon-dmath` — our own `sin`, `cos`, `exp`, `log`, `atan2`, `pow` — is **Block 1 work**, before there is
a rocket to fly. Reasons:

- WASM guarantees IEEE-754 for `+ − × ÷ √` but **not** for transcendentals; they come from the host libm
  and differ between browsers and native builds. A single ulp difference changes a chaotic trajectory's
  phase and breaks every downstream hash.
- Retrofitting determinism into a codebase that has been sloppy about it is a rewrite. Retrofitting it
  into a codebase that has done it from line one is free.
- It is cheap now: a few hundred lines of fixed-coefficient polynomials, plus a lint that forbids
  `f64::sin`, `Instant::now` and `HashMap` iteration in kernel crates.

**The rule that makes it survivable:** determinism is *verified* by golden-hash tests native vs. headless
browser, but it is *practised* by never letting a kernel function read a wall clock. If the build
occasionally drifts, the golden test says so loudly — which is infinitely better than discovering it when
a rating turns out to be unverifiable.

### 1.3 Data enters through one door

An **offline Rust preprocessor**, run by you (or CI), never at runtime:

```
Tier-0 sources ──► odderon-preprocess ──► octree tiles + MANIFEST rows
(JPL DE440, SBDB, Exoplanet Archive, 2MASS, PS1, USGS, CEA)
```

- Every output tile carries its `Provenance` and its manifest row (`DATA-LICENSING.md` §3).
- The runtime only ever does **HTTP range requests** for the tiles the player is observing, under a hard
  resident cap.
- The star catalogue is a `Survey` backend, so Tier-1 data (Gaia, ESA Hipparcos/Tycho) can be *loaded by
  a player who holds the rights* without existing in our distribution — and a `--release` build
  physically cannot load a non-redistributable tile.

This ordering matters: build the door before the first dataset, or you will end up with data baked into
the engine and a licence problem that is now an architecture problem.

### 1.4 Nothing is a "part"

There is no part list, no prefab, no asset. A nozzle is a Rao bell from `(throat_r, ε, length)`; a tank is
a surface of revolution with an aspect ratio; a building is a composition of parametric modules. Geometry
is `f(parameters)`, which is what makes the zero-asset law (`GRAPHICS.md` R1) enforceable by a file search
rather than by vigilance.

---

## 2. The toolchain, decided

| Question | Decision | Why |
| --- | --- | --- |
| Simulation language | **Rust → `wasm32-unknown-unknown`** | Non-negotiable: determinism, `no_std` discipline, and the kernel must be auditable as source (`CREDIBILITY.md` rung 1). |
| GPU access | **`glow`** (thin WebGL2 bindings) | We need ~8 draw calls and full control of shader variants; a scene graph is dead weight. Revisit `wgpu` only if WebGPU-required features appear. |
| Bundler | **Vite**, `?worker` for the sim thread | Not a game engine; a build tool for a WASM module and a handful of TS files. |
| Framework / engine | **None.** No Three.js, no Unity, no Bevy | Every one of them spends the budget on generality we have explicitly refused. |
| Instruments & UI | **Canvas2D + the stroke font**, some DOM for text | The browser's compositor draws it, off our frame budget. Instruments are the HUD (`BIBLE` §5.3). |
| Type sharing | `wasm-bindgen` **only at the boundary** | The kernel is a plain Rust crate, testable with `cargo test` and no browser. |
| Test runner | `cargo test` (native) + a headless-Chromium harness | Determinism must be proven *both* natively and in a browser. |

### 2.1 How I will write Rust you cannot compile

Since this sandbox has no Rust, I use four rules that keep the first build cheap:

1. **Small crates, wide interfaces.** If a crate is wrong, the blast radius is one crate.
2. **Prefer the standard library and `core`**; every dependency is a thing that may not resolve at the
   pinned version on a machine I cannot poll.
3. **No clever generics in the kernel.** It is a simulator, not a library. Concrete types, explicit
   arithmetic, `#[inline]` where it matters.
4. **Ship the tests with the code.** `cargo test` is the contract; if a test does not pass, the number the
   doc claims is not real yet.

---

## 3. The increment loop, and why it is four steps

Every increment is the same shape. Do not skip step 4.

| Step | Who | What | Artefact |
| --- | --- | --- | --- |
| **1. Build** | me | The smallest thing that is *playable or measurable*, on the branch | Code + tests |
| **2. Instrument** | me | The read-out that makes the claim checkable (frame cost, present count, timers, allocations, hash) | On-screen read-out, headless harness |
| **3. Measure** | **you** | Run it on Reference A and B, report the number, including when it is bad | A number, and a screenshot |
| **4. Correct** | me | If the number contradicts `GRAPHICS.md`, fix the *document and the code together*, and write down what was learned | `DECISIONS.md` entry |

The failure mode this prevents is the one that killed the last attempt: building three more subsystems on
top of an unmeasured claim. **A number that has not been measured is a hypothesis, and the plan does not
let a hypothesis carry weight.**

---

## 4. What "done" means for the first increment

Block 1, *The Room* (`BIBLE` §17) — but defined as **five measurable claims**, because a room is not
done when it looks nice:

1. **Zero-bitmap law passes.** A file search finds no `.png/.jpg/.dds/.ktx/.obj/.fbx/.gltf` outside the
   one glyph sheet (`GRAPHICS.md` R1).
2. **The room is a spacecraft.** Closing the curtain drops solar gain by the amount §6.7 of the bible
   says, and the AC's electrical draw visibly falls. This is the *one-solver* claim, and it is the reason
   the room is the first build: it proves the thermal network on Day 1 with no rocket.
3. **Idle is nothing scheduled.** `requestAnimationFrame` calls per second = 0, timers = 0, allocations = 0,
   after 250 ms of no input.
4. **The static-camera test (R2) passes on Reference A**: frame time < 1.5 ms after settling, edge quality
   matching a supersampled render.
5. **The window view is the best-looking thing on your laptop**, shown to you as a screenshot before I
   claim anything else about the art.

If claim 4 fails, the plan says exactly what happens next: **shade at ~0.5 MP and upscale**, which is the
lever `GRAPHICS.md` §3 (Path A, perceptual adaptive shading) already specifies. That is decided in
advance so a bad measurement produces a design change rather than a rationalisation.

---

## 4b. Where the ladder actually is

Kept current, because a plan that says "P0" forever is not a plan.

| Rung | State | Evidence |
| --- | --- | --- |
| CI as the compiler | **done** | Four hard gates run on every push: `cargo test`, wasm32 build, native↔wasm determinism, kernel policy. A Rust compile happens on a machine that is not this one, which is also why native-vs-wasm agreement is testable from Day 1. |
| `odderon-dmath` | **done for now** | Own `sin, cos, atan, atan2, exp, ln, sqrt, cosh, sinh, acos, asin, pow, fabs, fmin, fmax`, `no_std`, no host libm symbol. Error measured, not asserted: 1e-16 to 1e-17 typical. Golden hash armed. |
| `odderon-astro` | **two-body complete, validated** | Universal-variable Kepler propagator. One period returns to 0.17 µm after ten periods; a single jump over 1000 periods conserves specific energy *bit-exactly*; chained steps drift as √N·eps (1.8e-14 over 10000). Hyperbolic, parabolic and circular orbits need no special cases. |
| `odderon-astro` elements | **done** | State ↔ elements both ways, storing `p` rather than `a` so parabolas and hyperbolas are ordinary cases. The degenerate rows (`i = 0`, `e = 0`, both) are folded rather than left NaN, and round-trip to 1.5e-15 across seven orbit shapes including circular-equatorial and `e = 0.999`. Reproduces Vallado Ex 2-5 against the published vector. |
| `odderon-astro` remaining | next | **J2 perturbation** on top of the two-body solution — the first thing that makes orbits decay and precess, which is what makes "where will it be next week" a question with a real answer. Then the body catalogue, because every other part of the game needs to say where something is. |
| The shell (WASM + WebGL2) | not started | `GRAPHICS.md` specifies it; no pixel has been shaded. Needs `odderon-astro` stable first, because the shell exists to display what the kernel computes. |
| The Room (thermal) | not started | The five claims in §4. |

Two things this table is deliberately not allowed to hide:

- **Nothing here is "in progress" without a measurement.** The astro numbers above came from
  `tests/validation.rs` on a real compile, in CI, not from the prototype that preceded it. The prototype
  agreed, and the prototype is not evidence.
- **The `rustfmt` gate is red and stays red (advisory).** No local toolchain can run a mechanical pass,
  and hand-formatting against CI diffs is a losing game that has already cost two commits. It becomes a
  hard gate the day a local `cargo fmt` is possible, and not before. `docs/BUILD-PLAN.md` §2.4 states
  this in the gate table itself so it cannot be quietly forgotten.

---

## 5. The cadence

| Phase | Content | Gate to leave it |
| --- | --- | --- |
| **P0 — Spine** (Block 1) | `dmath` + determinism harness; WASM/WebGL2 shell; the read-out; one parametric room; the thermal solver; art sun, gobos, sky; R1–R3 | Your measured frame cost on A and B |
| **P1 — The Pad** (Block 2) | 1-D cryo/fluid network; the clipboard; the static fire; the wreck that stays examinable | A reproducible failure from the input log |
| **P2 — The Mark** (Block 3) | Grease pencil; ghost twin; one arrow; Akashi export | The Loop of Consequence firing end-to-end |
| **P3 — Flight** (Block 4–6) | Sounding rocket → orbit → light-time comms | Bit-identical replay in a second browser |
| **P4 — The Archive** (Block 7) | Museum, protégé, causal mail | Someone else can verify your work |

Two rules across all five:

- **Nothing is held back for a big reveal.** Each phase produces a public build.
- **Every phase has a kill criterion written before it starts** (`CREDIBILITY.md` §14). If Block 1 cannot
  hold its frame on Reference A with the simple path, we cut the outdoor walk to instruments + interiors
  and say so publicly — rather than quietly lowering the promise and carrying on.

---

## 6. Risks specific to *how* we are building it

| Risk | Why it is real here | Mitigation |
| --- | --- | --- |
| **I cannot compile Rust in this sandbox** | Every Rust deliverable is compile-unverified until it reaches you | Small crates, std/core only, tests shipped, first build treated as a fix-up pass |
| **I cannot measure GPU performance** | SwiftShader is a CPU rasteriser; its timings are meaningless | The instrument ships in the product; you are the only performance measurement |
| **WebGL2 only exists here through a bundled Chromium** | Harness is fiddly (Brotli unpack, `LD_LIBRARY_PATH`) | Keep the harness script in `tools/`, document it, never make the *product* depend on it |
| **The kernel/presenter boundary erodes** | It is the easiest thing to violate under deadline | A CI check that kernel crates do not depend on web/present crates, and the actuator-only lint from `VISION.md` law 4 |
| **Determinism is discovered late** | It is invisible until a hash mismatches | `dmath` + golden hashes in P0, native *and* headless browser |
| **Licence slip on a dataset** | Easy, and only bites at release | `DATA-LICENSING.md` manifest + the two CI gates, from the first dataset |
| **Scope** | A decade-sized vision with a solo dev | The block ladder, a shipped build per phase, and a kill criterion per phase |

The one that matters most is the first two. **You are the only performance measurement I have, and the
only compiler.** The plan is shaped so that neither of us is asked to pretend otherwise.
