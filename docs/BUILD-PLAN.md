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

### 2.1 How the Rust gets compiled — corrected 2026-11-24

This section used to begin "since this sandbox has no Rust". **That is no longer true, and what replaced
it took twenty minutes to find**, so it is written down rather than remembered: the registry this sandbox
*can* reach also carries the official toolchain as npm packages —
`@rustbin/rustc-<v>-x86_64-unknown-linux-gnu` (a real `rustc` plus `librustc_driver`),
`@rustbin/rust-std-<v>-{x86_64-unknown-linux-gnu,wasm32-unknown-unknown}` (the `core`/`std` rlibs),
`@rustbin/cargo-<v>-x86_64-unknown-linux-gnu`, and `@rustbin/clippy-<v>-…`. Assembled into one prefix and
pointed at with `RUSTC`/`PATH`/`LD_LIBRARY_PATH`, they compile, test, lint and build for `wasm32` with no
network access to `crates.io` — which is fine, because this workspace has **no external dependencies at
all**. `@scalar/rust-fmt` is rustfmt compiled to wasm and runs on plain Node, providing the formatter.

So the four rules below are now belt-and-braces rather than the only defence, and the risk table's
"every Rust deliverable is compile-unverified" line was too pessimistic. What is still true, and is the
reason CI remains the authority:

* the toolchain here is **1.85.0**, while `rust-toolchain`/CI pins **1.83.0**, and a different compiler
  can accept code the pinned one rejects (the undocumented `#[unsafe(no_mangle)]` idiom, for one, is a
  2024-edition-ism used here deliberately);
* the formatter is rustfmt **1.85.0's** wasm build, not 1.83.0's, so a formatting match is strong
  evidence and not proof.

Both were used to check the orbit-boundary change: 6 native tests, `cargo test --workspace` (16 binaries,
0 failures), `cargo clippy -- -D warnings` on both the workspace and the wasm32 target, and a real
`wasm32-unknown-unknown` release build whose exports were then run by `tools/check-orbit.mjs --wasm`.
CI is still the only place the *pinned* toolchain compiles it, and this section says so rather than
pretending otherwise.

The rules, which still keep the first build cheap:

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
| CI as the compiler | **done, seven jobs, all armed** | Seven jobs run on every push and **every one of them can fail a branch**. `test (native)` runs the `cargo test` suite *plus* the determinism golden check as a step rather than a job of its own — that is why there are seven jobs and not eight. The other six are `build (wasm32)`, `kernel policy lint`, `class certification lint`, `shell claims (1, 3, 5-artifact)`, `clippy` and `rustfmt`. `clippy` and `rustfmt` were `continue-on-error` until 2026-11-23 and are armed now that both are green on `main`. A Rust compile happens on a machine that is not this one, which is also why native-vs-wasm agreement is testable from Day 1. **This row was wrong until 2026-11-24**, claiming six jobs: `shell-claims` was added when claim 3 was gated and the row was never updated, so the plan under-described its own CI. It now names all seven, and `shell-claims` carries five steps of its own — the static path resolver, the shell instrument, the temporal cache, the ablation report judgement, and the frame capture — so "seven jobs" is a count of jobs and not of checks. |
| `odderon-dmath` | **done for now** | Own `sin, cos, atan, atan2, exp, ln, sqrt, cosh, sinh, acos, asin, pow, fabs, fmin, fmax`, `no_std`, no host libm symbol. Error measured, not asserted: 1e-16 to 1e-17 typical. Golden hash armed. |
| `odderon-astro` | **two-body complete, validated** | Universal-variable Kepler propagator. One period returns to 0.17 µm after ten periods; a single jump over 1000 periods conserves specific energy *bit-exactly*; chained steps drift as √N·eps (1.8e-14 over 10000). Hyperbolic, parabolic and circular orbits need no special cases. |
| `odderon-astro` elements | **done** | State ↔ elements both ways, storing `p` rather than `a` so parabolas and hyperbolas are ordinary cases. The degenerate rows (`i = 0`, `e = 0`, both) are folded rather than left NaN, and round-trip to 1.5e-15 across seven orbit shapes including circular-equatorial and `e = 0.999`. Reproduces Vallado Ex 2-5 against the published vector. |
| `odderon-astro` J2 | **secular done, validated** | First-order J2 drift (`j2::secular_rates`, `j2::propagate_j2_secular`). Pinned by structure, not by arithmetic: the node rate is zero at `i = 90°`, the periapsis rate is zero at the critical inclination `63.4349°`, the rates scale linearly in `J2`, `J2 = 0` reproduces two-body, and the sun-synchronous solution lands at **98.19°** (retrograde) rather than the 81.8° a sign error produces. **Reproduces the ISS-class nodal regression of −5.0°/day** and periapsis advance of +3.74°/day. Short-period wobble is *not* modelled and is a separate `CREDIBILITY.md` row. |
| `odderon-astro` integrator | **reference only, validated** | `integrate.rs`: adaptive Dormand–Prince 5(4) on the real J2 equations of motion. **Never called from a frame** — it exists to make the secular theory falsifiable, because a formula comparing itself to its own algebra proves nothing. `J2=0` conserves energy to 1e-9 and angular momentum to 1.7e-10 over 7 days; under `J2` total energy is conserved to 4e-10. Used by `tests/j2-crosscheck.rs`, which measures **0.435 %** agreement on the nodal rate at LEO. |
| `Provenance` as a type | **done, first surface** | `provenance.rs`: `Prov` is a sum type over epistemic status — `Measured{sigma}`, `Defined`, `Derived{method}`, `Assumed{basis}`, `Unknown` — because the distinction that matters is *known vs invented*, not *how wide is the bar*. `Quantity::unknown()` carries `NaN` so arithmetic on it fails loudly; `Prov::measured(0.0)` is refused. `Lookup<T>` keeps "no such body" distinct from "body exists, property unknown". |
| The body catalogue | **done, opaque bodies only** | Earth, Moon, Mars, Sun: `mu`, equatorial radius, `J2`, rotation rate, every one carrying its provenance. **Star positions, planetary masses and orbital elements are deliberately absent** — they belong behind the `Survey` seam and carry a licensing consequence (`DATA-LICENSING.md` §1). Rotation phase / prime meridian also absent: IAU 2009 and 2015 disagree by tens of degrees and a guess is invisible. The Moon's `J2` and the Sun's oblateness are `NotKnown` with reasons, not estimates. 14 tests, mostly about the type discipline. |
| `odderon-astro` frames | **done** | `frames.rs`: `BodyFixed{w0, rate}` with `W(t) = W₀ + Ẇt` wrapped into `[0, 2π)`, both directions, and deliberate `NoPrimeMeridianConstant` errors when a body publishes no prime meridian. Deliberately **not** `Default` — a default `W₀ = 0` produces a perfectly self-consistent frame half a world from the truth, and every algebraic test passes. 11 tests, split into *algebraic* (round trip, periodicity) and *physical* (geostationary longitude, westward sub-satellite drift, agreement with the IAU rate to 5.12e-6 %): only the physical ones catch a zero `W₀`. The full-rotation residual from the stored 8-figure rate is **4.71 m at the equator**, documented rather than hidden. |
| `odderon-astro` ground stations | **done** | `observe.rs`: `Site::from_geodetic` solves the ellipsoid properly, with `up` as the **ellipsoid normal**, not the radius vector — the difference is 0.19° / 21 km at 45° latitude and tilts every elevation by that much. `look_at` → elevation, azimuth, range; `horizon_range` → 2294 km for a 400 km orbit. `Physical` gained `flattening`, with the Sun's `NotKnown{"solar-oblateness-unsettled"}` rather than an estimate. 18 tests. |
| `odderon-astro` pass prediction | **done** | `passes.rs`: `next_pass` → rise, set, culmination, maximum elevation, both azimuths, minimum range. Both search bounds are **derived, not tuned**, and checked against a bisection on the exact elevation function: minimum pass `2θ/(n_max − ω)` with `θ = acos((R/r)cos ε) − ε` (652.3949 s at the horizon, 398.2311 s at 10°, for 400 km), guaranteed window `2π/(n_min − ω)` = 5936.24 s. The guards **refuse** rather than answer "no pass" — a 90-minute window is short by 8.9 minutes, and a 120 s step steps clean over a 64.5 s pass. Culmination is golden-section refined, so it is step-independent (same 90.0000° at 10 s and 5 s). 17 tests, each keyed to a measured number: synodic spacing 5936.2388 s vs orbital 5553.6243 s; 51.6° orbit → 390.35 s pass with 76.4221° maximum at 30° N and **nothing** at 60° N (`Ok(None)`); a frozen Earth shortens the overhead pass by 42.05 s. |
| `odderon-astro` time systems | **done, UTC 1972–2026** | `time.rs`: `Civil` ↔ TDB seconds past J2000, through `TAI − UTC` from a 28-row published table, the 32.184 s TT−TAI definition, and the leading `TDB − TT` periodic term (1.6570 ms, extremes at day 92.85 / 277.42). Round trips close to **1e-7 s**, and the 2016-12-31 leap second is three consecutive deltas of exactly 1.000000000 s at **all 28 rows**, not just that one. The inverse is a bijection because it indexes candidate days by TAI day-start: `23:59:60` and the following midnight are the same TAI instant, and searching earliest-first keeps them filed under their own days. Three typed refusals — before 1972, after 2026-12-31, and a `23:59:60` the table never recorded — and the inverse refuses the same regions rather than exposing a one-way gate. The range's upper bound is **coverage of the IERS announcement, not the last leap second**, which is a distinction this row originally got wrong: the table's last *row* is 2017-01-01, and reading that as the end of the table's authority made the module refuse every date in the present — the game is set in 2026. A table of changes cannot state an absence of changes, so the zero-leap-second span from 2017-01-01 to 2026-12-31 is its own declared constant, and a compile-time check fails the build if a leap second is ever inserted inside it. **The lesson worth keeping is about the test, not the module:** the round-trip target that `CREDIBILITY.md` set for this row is satisfied by two wrong epoch constants. Only the absolute anchor `utc_to_tdb(J2000 UTC) = −72.60 µs` finds them, and it now exists. The table's ordering is also asserted in a `const` block, so an out-of-order row is a compile error rather than a wrong second years later. |
| The licence gate | **done, mechanically** | `src/classes.json` + `tools/check-classes.mjs`, a hard CI job (`docs/CLASSES.md`). Every dataset is OPEN or GATED; the checker fails both ways (a promised class that is absent, and a class that appears without being promised) and refuses a GATED record in the kernel. `tools/check-classes-selftest.mjs` injects ten violations and requires each to be caught. It cannot verify a licence — §3 of that document says so — but it makes an unclassified record unwritable, which is the part a program can do. |
| `odderon-astro` remaining | next | **The `Survey` seam** for stars, which is where the class gate starts earning its keep: Tier-0 backends ship, Tier-1 arrive only through a player-installed implementation. J2 is now **in the pass search** (`next_pass`), with `next_pass_exact_geometry` kept for the closed-form checks that must stay two-body — the two differ by 18.19 s in the beat. The drift the old row quoted ("8.4 s on day 0, 80 s on day 1") did not survive measurement: at 400 km / 51.6° the pass moves −75 s by the first observation and −445 s by the tenth, **−149.8 s/day**. Remaining in this row: J2+J3+J4 for the rendezvous case, which the same measurement now quantifies — first-order secular theory is 439 km from the real motion after a day, so a rendezvous is not an approximation away. |
| The shell (WASM + WebGL2) | **renders, measured on real silicon** | Two passes: an expensive shade into a one-target cache, and a blit. Measured on Reference B (Mali-G51, 1.04 Mpx), **re-measured 2026-11-24**: flat fill 0.67 ms, `blitFramebuffer` **1.27 ms**, texture read — **the pass `web/index.html` actually runs — 1.65 ms**, full shade 26.63 ms (the earlier 48.71 ms was under thermal load). **R2 is supported on the shipped path but not yet directly measured.** The texture-read path measured 1.65 ms against the 1.5 ms budget. The per-frame pass is now a real `blitFramebuffer`, and Reference B's two instruments agree at **≤ 1.20 ms** (the renderer's amortised bound) and **1.27 ms** (the ablator's floor row) — a ~15% margin on the larger. A direct per-frame measurement needs a GPU timer, which this device does not have. The previously recorded **"R2 passes at 1.25 ms with 17% margin" is withdrawn** — 1.25 ms does not reproduce on the device and path it described, and the row it came from was one of two identical operations. R3 is met by construction at 8 B/px. `tools/check-shell.mjs` and `tools/check-cache.mjs` are the standing tests, and **both now run in CI** — `check-cache.mjs` did not until 2026-11-24, so "standing test" meant "runs when somebody remembers", which is a claim rather than a check. |
| The Room (thermal) | **claims 1–4 measured; claim 5 is a judgement and open** | `thermal.rs`: the §4 claim 2 measurement, as a lumped zone and a vapour-compression loop. The bible's own arithmetic reproduces exactly — 3 m² × 800 W/m² × SHGC 0.6 = **1440 W** open, **120 W** with the curtain closed (SHGC 0.05) — and the coupling is measured rather than argued: on a real 12 000 BTU/h unit (EER 11, COP **3.224**, which the Copeland correlation reproduces to **0.9 %**), the draw falls from **713 W to 304 W**, a **57 %** drop visible on a meter. The drop is *not* the load ratio (59 %), because ASHRAE's cyclic degradation coefficient makes a cycling unit worse at part load — the penalty works **against** the drop, which is the conservative direction. The room cools **5.9 K in the first ten minutes** after the curtain closes, against a `C/UA` time constant of **44.4 min**; that 5.9 K is what chose the zone's 120 kJ/K capacity, which is 1.59x the 75 kJ/K of air the room holds — the air alone would cool several times faster, and the three plausible capacities give 17.7 K, 8.9 K and 5.9 K in ten minutes, so the claim holds across the range and 120 kJ/K picks where in it to sit. The integrator is explicit Euler with a **measured** stability bound rather than a trusted one: at `C/UA` it converges, at 3× it diverges, and the bound returned sits between them. 22 tests. This row was rewritten once: the first version reported 464 W → 190 W from a COP of 4.82, which is EER 16.4 — a number no window unit in this class achieves, produced by an `η₂` quoted from a range rather than checked against a nameplate. The omissions are named in the module and are real — no latent heat (so the draw is *understated* on a humid day), one thermal node, no surface-to-surface radiation, and a COP correlation rather than a cycle solved from an equation of state, which is why the real-gas row in `CREDIBILITY.md` §2 still demands REFPROP. **Claim 2 IS NOW IN THE PICTURE, which is the change of 2026-11-25.** Everything in this row was measured and none of it was *visible*: `thermal.rs` had reproduced the bible's arithmetic for weeks while `web/index.html` rendered a wall, a crate and a sphere, so the game had no room in it and claim 2 was a test rather than a thing you could see. The kernel now exports `room_solar_w`, `room_light_fraction`, `room_cooling_w` and `room_draw_w`, and the page draws a window in the wall whose **light is the network's output**: the shader multiplies its sun radiance by `room_light_fraction`, so closing the curtain changes the picture because SHGC changed, not because a brightness constant did. Measured through the page, with the module loaded by the browser: solar gain **1440 W → 120 W**, AC draw **713.4 W → 303.8 W (a 57.42 % drop)**, and the shader's input **1.0000 → 0.0833**, which is exactly 0.05/0.60. The 713/304 pair only reproduces when the fixture is the test suite's own (§6.7's 35 °C outdoors and its 300 W occupant); the first version of the ABI invented 32 °C and no occupant and got 582 → 161.5 W, a 72.25 % drop, which is internally consistent and is not the number this plan ships. That is the second copy of a claim problem again, and it is why the exports now use `tests/thermal.rs`'s `summer()` and `occupant()` rather than their own constants. `tools/check-room.mjs` gates it in CI and is the point of the exercise: it asserts the panel's numbers equal the kernel's numbers computed independently, that the page holds **no** thermal constant of its own, and that toggling the curtain moves at least 500 pixels. It was shown failing against the pre-fix page (no room at all), against a page that derives SHGC itself, and against a **decorative** room hardcoded to `0.083` that produced the same image, the same text and the same pixel change -- which is the failure this file exists to catch, because a plausible picture is not evidence. **AND THEN IT STILL WAS NOT A ROOM, which is the change of 2026-11-25 (second pass).** The panel reported the right numbers beside a scene whose owner described it as *"nothing like a room"* -- and they were right. What the first version of this work had built was an EXTERIOR: a corrugated metal wall filling 40 % of the frame, a crate on grey concrete, a sphere, and a 1.15 m square of dark blue in the wall that was labelled a window and was really a hole. Measured, the 'room' covered **0.9 % of the frame** and the sun patch was a rounded rectangle placed by a constant rather than cast by an aperture. Four things were wrong and only one of them was cosmetic:

1. **The camera was outside the building.** It is now inside an interior box (5.2 x 2.64 x 6.0 m) with a 2.30 x 1.44 m **hole** in one wall -- not a decal, the same rectangle subtracted from the wall's own bounds. The room went from 0.9 % of the frame to **58 %**.
2. **The sun was on the wrong side.** `sunDirection()` returns **(0.585, 0.395, -0.708)** for a late-November declination, so daylight travels toward +z and -x. The first room put its window in the +z wall, which means every sunbeam test asked for light coming from the wrong direction and the patch could not exist. The room is now turned so its window wall is -z; it faces the sun.
3. **The patch was drawn, not cast.** It is now `apertureVis()`: a surface point is sunlit when the ray from it toward the sun passes through the aperture, with the penumbra from the solar half-angle. The trapezoid is then a consequence of the geometry, and moving the sun moves it.
4. **The curtain had to be a thing.** It is a surface *in* the opening now, backlit, with folds and a rim of light leaking past its edges. Before, the cloth was shaded from the floor's own position -- so closing the curtain darkened the room and never showed a curtain. *"The sun is blocked"* and *"you can see what is blocking it"* are two claims, and only the first was true.

**The check now looks at the frame, and that is the part that matters.** Every assertion from the previous pass passed on the scene that read as a hole, because the check asked the page what it thought rather than asking the picture. `tools/check-room.mjs` now measures the pixels: floor mean luma **115.8** (the scene that read as a hole was **6.1**), sun-patch peak **164.5**, warm-toned share of the lit floor **100 %**, and closing the curtain darkens the floor **22.9 %** (115.8 -> 89.3). Every threshold is set from those numbers with its headroom stated in the file, because a threshold chosen to make the right answer pass is not a guard. It was shown failing on **the exact scene that was rejected** (three assertions: no warm light, 0.0 % darkening, 1062 changed pixels), on a **hand-placed patch** instead of a cast one, and on a **missing curtain surface** -- that last one passed at first and is why the curtain-visibility assertion exists.

**A separate defect, and the reason there is a new check in CI:** `web/index.html` keeps its fragment shader in a JavaScript template literal, so ONE backtick in a GLSL comment closes the literal early and the module never runs -- blank canvas, no WebGL error at all. Four edits to this scene hit it in one sitting. `tools/check-script-syntax.mjs` asks Node to parse the script, which is the same question the browser asks, in a tenth of a second with no GPU; counting backticks does not work, because the count stays even as the literal closes and re-opens.

**And one measurement claim was withdrawn.** `check-cache`'s assertion that the blit and the texture-read fallback render identical pixels had been passing on a comparison of **sensor noise**: `emit()` mixes grain as a hash of `gl_FragCoord + uFrame`, and two page loads settle after a different number of frames. It only surfaced when routing every path through one output stage made the grain reachable from the interior -- where it had previously been dead code, hiding the difference under the threshold. Measured on a nine-colour flat probe: **149 085 pixels differing by up to 5 levels**, both passes exact. The grain is now a function of position only (**0 pixels differ**), because a static image that changes state is the same defect as a render loop spinning on an idle page.

**AND THE PREVIEW SAID `— no kernel`, WHICH IS THE FINDING OF 2026-11-25 (third pass).** Every screenshot in the previous two passes came from a machine where I had built the module. A reader served the page from a clone got something different: `orbit source: unavailable`, `room solar: — no kernel`, and a hole onto a flat horizon -- and asked, reasonably, *"is this it"*.

**It is not a defect in the room. It is `dist/` being git-ignored**, which is correct: `/dist/` is build output and `**/*.wasm` is a binary. R5 guards *image and mesh* assets -- its own pattern is `.png|.jpe?g|.webp|...|.gltf|.glb`, and its own message is *"Every surface must be a function S(u,v,t)"*, i.e. GRAPHICS.md R1, which is about textures and models, not about compiled code. So nothing in the rule forbids the kernel; the module is simply never in a checkout, and a sandboxed preview with neither `gh` credentials nor a Rust toolchain cannot produce it.

**What was wrong was the report.** The panel said `— no kernel`, which names neither the artifact nor a remedy, and says nothing at all in the frame -- and a hole onto a bright horizon is a picture that cannot be argued with. That is the same class of failure as the 43 % self-disagreement and the panel that scored IoU 1.000 over a black scene: `a number the panel reports but the frame does not show` has moved the claim into the HUD rather than onto the screen, and here the absence had moved there too.

Now, when the module is missing: a banner across the bottom of the frame says **the thermal kernel is not built, so there is no room in this picture**, names the artifact and the reason it is absent from a clone, gives both routes to build it, and says plainly that a sandbox with neither `gh` nor cargo can only show the geometry. The room rows distinguish **`— kernel not built`** (a checkout fact) from **`— waiting for kernel`** (a transient), because only one of those is worth reporting as a defect. `tools/check-missing-kernel.mjs` takes the module away and asserts every one of those things; shown failing **nine ways against the page the reader was actually looking at**, including `banner occupies the frame 0 px²`. It runs in `build-wasm`, the only job with both the module and a browser.

**Claim status, checked rather than remembered** (this row previously said "claims 1, 3, 4, 5 not started", which was wrong in three places): **1** passes — `kernel-policy.sh` rule R5 refuses any tracked binary asset, and `git ls-files` finds none. **3** passes *and is now gated* — `tools/check-shell.mjs` measures 0 frames drawn while idle, and a new `shell-claims` CI job runs it on every push. **4** **SUPPORTED BY TWO AGREEING ESTIMATES, awaiting one direct measurement.** History, because it matters: the texture-read pass measured **1.65 ms** at 1.04 Mpx against a 1.5 ms target, and the original "passes at 1.25 ms with 17% margin" was withdrawn. The per-frame pass is now a real `blitFramebuffer`, and on Reference B (2026-11-24) **two independent instruments agree**: the renderer's own amortised bound reads **≤ 1.20 ms**, and the ablation matrix's separate `blitFramebuffer` floor row reads **1.27 ms** — 5.5% apart, on a budget of 1.5 ms, leaving **~15% margin on the larger of the two**. The blit also renders a pixel-identical image to the fallback (0 of 313,600 sampled pixels differ, `tools/check-cache.mjs`). **The pixel clause is now derived rather than declared**, and it was the one part of claim 4 that no instrument had ever checked: `MAX_PIXELS` was a bare `1_150_000` with no derivation, above the **1.228 Mpx** a 1.5 ms budget buys on the 1.27 ms blit and disagreeing with `GRAPHICS.md`'s **0.68 Mpx** — a figure that was itself `1.5 ms / (2.29 ms at 1.04 Mpx)`, resting on a texture-read row this project **retracted**. So the code's number came from nothing and the doc's came from a withdrawn measurement. The clause is now **1.068 Mpx** = `1.5 / 1.2211 / 1.15`, declared as `CLAUSE_MPX` with `MAX_PIXELS` computed from it, printed on the panel (`1.07 Mpx at clause 1.07`), and held by `tools/check-pixel-budget.mjs`, which recomputes the derivation and fails on the retracted 0.68 or on any clause above the no-margin reach. It was shown failing on the original `1_150_000`, on `0.68`, and on `1.5`. What is still missing is a **single direct measurement of one frame**: one side is an amortised upper estimate and the other a median of an emulated call, and Reference B has no `EXT_disjoint_timer_query_webgl2` to produce a direct number. So claim 4 is **supported, not passed** — and this row will say passed only when a direct measurement exists. **A direct measurement was then attempted, 2026-11-25, and the attempt is the finding.** Fence-based per-frame timing (`fenceSync` + `clientWaitSync`, offscreen target so presentation is excluded) was built and measured. It resolves the frame cost by subtraction, and it works — but the subtraction is of two samples of a **scheduler-bound** clock, and the floor of that clock is the answer: one `setTimeout(0)` tick costs **4.1 ms** on this sandbox (median of 16, minimum 0, so it is the clamp and not the work), against a budget of **1.5 ms**. Measured three ways on one page: a tick with no GPU work at all **4.1 ms**, the same tick with a fence and no blit **4.2 ms**, a blit plus that tick **4.0–4.2 ms**. The blit does not move the number, because the number *is* the yield. So a naive version of this reports "4.00 ms direct" wherever it is run and calls the scheduler a renderer. **The code now refuses instead**: when the method's own floor is above the budget, `directBlitMs()` returns no figure and states the floor. On this sandbox it resolves to **~0.1 ms** by subtraction with an uncertainty of one tick, which is *not* a claim — it is a difference of two 4 ms numbers — and the panel shows it in tenths, in warning colour, with the figure clamped at zero because the raw subtraction once printed **"~-0.1 ms frame"**, a negative time. **What this establishes about claim 4, and what it does not:** it establishes that the missing measurement is not blocked by the GPU but by the clock, and that on a device where a tick costs more than the budget there is no wall-clock route to a per-frame number at all. It does not produce the number. The remaining route is a GPU timer, and Reference B does not have one — so claim 4's last step is a hardware question, not an engineering one, and this row now says that instead of implying somebody has not got round to it. **A second finding, about the existing bound:** `upperBoundBlit()` reports T/K — K blits queued back to back over T ms. That bounds **throughput**, not the latency of one frame, and a pipelined GPU answers throughput with a smaller number: measured here, **0.014 ms** T/K against **0.2 ms** for one fenced frame, an order of magnitude apart. The panel used to print that figure as "upper bound", which overstated what had been measured — the same class of error as calling submit time a frame time. It now reads **"per frame k=64"** and `check-cache.mjs` asserts the row states a per-frame quantity rather than attesting to the old phrasing. **5** is open, because it is the one claim no instrument can settle: the frame is captured by `tools/screenshot.mjs` into `.artifacts/` (uncommitted — R5 forbids the PNG in the tree) and has to be looked at by a person. Two things about that artifact are now known and fixed rather than assumed, because *a panel nobody can read is a judgement nobody can make*: `capturePicture()` rasterised the HUD into the saved PNG **without the document's stylesheet**, so rows collapsed and values printed on top of their own labels — in the PNG, while the live page was correct and every check therefore passed; and it composited the panel in **CSS pixels into a device-pixel buffer**, so on a phone (dpr 2) the panel landed at half size in the corner. Both were found by capturing a dpr-2 viewport after the first was fixed. `tools/check-hud-capture.mjs` now compares the panel in the picture against the panel on the page at **dpr 1 and dpr 2**, and scores the pre-fix page **0.000–0.025** against **0.728–1.000** for the fixed one. That check is the reason claim 5 can be judged at all: before it, the picture being judged was not the picture the renderer drew. **A third defect, found 2026-11-25, and the one that made the other two moot:** the scene in the saved picture was **black at every device ratio**, while the live page rendered correctly. `capturePicture()` copied the frame with `drawImage(canvas)` — a readback of the **default framebuffer**, which on this backend returns black after a `blitFramebuffer`. The module already documented that quirk on `captureNow()` and had moved the *pixel probe* onto the cache; the *picture* was left behind. Measured on one page load, 200 sampled pixels each: cache mean luma **146.3**, 200/200 non-black; default framebuffer **0.0**, 0/200. The picture is now read from the cache, which holds the image on both the blit and texture-read paths. The existing panel check **could not see this** — its mask is built from the panel region, so a perfect panel over an empty frame scored **IoU 1.000**. The check therefore now also compares the **whole frame** against the live page on a 16x9 grid: correct **0.960 / 0.985** of cells agree, black scene **0.170 / 0.607**, threshold **0.80**. Two weaker scene metrics were tried and rejected first: absolute brightness read **8.7% lit against an 8% floor** (the pre-fix phone frame is only partly black), and the picture/page brightness ratio was **0.071 at dpr 1 but 0.314 at dpr 2** — the populations were closest exactly where the bug was worst. |
| Kernel in the page | **wired and gated; the build is verified in CI, not in the sandbox** | `web/index.html`'s `orbitPosition(t)` now calls `odderon-astro` through wasm: `orbit_state_{x,y,z}` and `orbit_velocity_{x,y,z}` over a reference orbit built *inside* the kernel from `Elements::to_state`, plus `orbit_iterations` and `orbit_unconverged` so a failed solve cannot pass as a position. The page keeps **no** local `mu`, no solver, and no circular-orbit speed — `tools/check-orbit.mjs` fails the build if any of those reappear, and it **fails 6 of 7 checks against the previous commit's page**, which is the only evidence that a guard is a guard. **Why this was worth doing, correcting the row it replaces:** the old row argued the transport could wait because a kernel call is a 0.001% term next to a 26–56 ms shade. The cost argument was right and irrelevant. The stub returned a **position and no velocity**, so the page held no state it could advance — and a render-on-demand renderer that resumes after idle has to answer "where is it *now*" without re-deriving from elements and re-solving. The defect was capability, not milliseconds. **What is verified, and where:** a real `rustc`/`cargo` assembled from the registry (§2.1) builds this for `wasm32-unknown-unknown` — **12,783 bytes, zero imports** — and `tools/check-orbit.mjs --wasm` then reads `r(0) = 6860.000 km`, `v(0) = 8.691175 km/s` and closure after one period from that binary. `cargo test --workspace` is 16 binaries / 0 failures and `cargo clippy -D warnings` is clean for both the host workspace and the `wasm32` target. **The pinned 1.83.0 toolchain still only runs in CI**, which is why the `build (wasm32)` job re-runs the same boundary check against the binary it produces, demanding `r(0)` = periapsis, `v(0)` = vis-viva (not `sqrt(mu/r)`), and closure after one full period. The native half of that arithmetic is `crates/odderon-astro/tests/reference-orbit.rs` (energy and angular momentum conserved through a period, apoapsis at half a period, backward propagation inverting forward). **When the artifact is absent** — the in-app preview, which is sandboxed with no network, and any checkout that has not run `npm run fetch:wasm` — the page says `orbit source: unavailable: HTTP 404`, places the body far outside the frame where it cannot be seen, and shades every other pixel as usual. It does **not** fall back to a JavaScript orbit: a substitute orbit is exactly the claim this change removes. |

Two things this table is deliberately not allowed to hide:

- **Nothing here is "in progress" without a measurement.** The astro numbers above came from
  `tests/validation.rs` on a real compile, in CI, not from the prototype that preceded it. The prototype
  agreed, and the prototype is not evidence.
- **The ablation instrument was rewritten because it disagreed with itself by 43%, and the rewrite has
  never been run on real hardware.** This bullet said "and is therefore not yet a measuring
  instrument" until 2026-11-24. That sentence is now out of date: the instrument *was* fixed, and saying
  otherwise is as wrong as claiming it works. What follows is split into what is history, what is
  fixed, and what is still unknown.

  **History, and these numbers are from the old tool.** On Reference A the row "full shader (as
  shipped)" read **3.00 ms** and the row "resolution 1.00× (native)" read **1.71 ms** — the same
  shader at the same resolution on the same device. Rows ran once each, in sequence, so every row
  carried whatever clock state the row before it left behind. Consequence: the five small feature
  ablations (corrugation 1.58, aerial 1.50, sphere 1.50, tonemap 1.50, sky glow 1.54 ms) were **all
  within 2% of each other and all smaller than the ordering bias**. They were never findings and still
  must not be quoted as such. The four rows that cleared the bias — shadow (3.00 → 0.83), tap count
  (8 → 4 saves 1.67 ms), resolution (linear), and the floor share (6% on A vs 45%-equivalent on B) —
  cleared *that run's* bias, which is not the same as surviving a better instrument.

  **Fixed, in `web/ablate.html`.** Three mechanisms, all visible in the output rather than assumed:
  every variant is measured **once per round** across 7 interleaved rounds, so clock drift is spread
  across all rows instead of assigned to whichever row ran at the wrong moment; each row reports the
  **median with its min–max spread**, and a delta smaller than its own spread is printed as
  "indistinguishable" instead of as a number; and a **CONTROL** row re-measures the baseline in a
  different slot, so its disagreement *is* the instrument's error bar, published at the top of the page.
  If the control disagrees by more than a few percent the page reports no verdict at all. `summarise()`
  is covered by `tools/check-ablation.mjs`, which feeds it matrices with known answers — including the
  exact shape of the 43% failure — and it is now a CI step, so the judgement cannot regress silently.

  **Run on real hardware, 2026-11-24 — and it found two more bugs in itself.** The first laptop run
  printed an instrument error bar of **2.8%** (control 0.771 ms vs baseline 0.750 ms, same shader
  twice), which is the rewrite working: the old page could not have told you what its own error was.
  It then made two mistakes, both now fixed and both now covered by `tools/check-ablation.mjs`:

  1. **It printed a resolution floor and then ignored it.** The page reported *"resolution floor: 63 us
     per frame — deltas below 63 us are invisible to this instrument, not zero"* and, three lines
     later, listed a **50 us** delta (`− body sphere test`) as a finding, judged against a 5% error bar
     of 37.5 us. A delta of 0.8 of one timer tick was being reported as a measurement. Each row is now
     judged against **its own** floor (`tick / max(MIN_FRAMES, that row's frame count)`) as well as the
     relative bar, and the rejected row says which bar rejected it.
  2. **The verdict contradicted the floors.** It said *"A CACHED STATIC FRAME costs 0.24 ms … That
     PASSES R2 at native"* and then extrapolated the **full shader** sweep and printed *"VERDICT: R2 IS
     MET … 1.5 ms buys 3.64 Mpx"*. Both were true and only one was about R2. R2 is a budget for a drawn
     frame, and on a render-on-demand renderer that is the **blit**, not the shade. The verdict now
     names the blit first and labels the sweep explicitly as the full-shade question.

  **Second run, same laptop, same day — the numbers below supersede the first run's.** The first run's
  blit figures were two readings of the same shader (see `GRAPHICS.md`), so this run is the one that
  measured the operations separately:

  | row | ms at 1.86 Mpx | note |
  | --- | --- | --- |
  | flat fill, no memory access | **0.13** | 1.0x |
  | one texture read | **0.26** | 2.1x — **the shipped per-frame path** |
  | `blitFramebuffer`, no shader | **0.23** | 1.9x — a hardware blit, which the renderer does **not** use |
  | full shader (as shipped) | **0.77** | only when the world moves |
  | resolution sweep | **0.41 ms/Mpx** | |
  | *run 3, same device* | | |
  | flat fill | 0.13 | |
  | one texture read | 0.25 | |
  | `blitFramebuffer` | 0.23 | again within 20 us of the texture read |
  | full shader | 0.78 | spread **205%**, positional |
  | control | 0.77 | spread **5%**, same program |

  The two floors are now **3 readings apart at 240-frame batches, i.e. genuinely separate** — and they
  come out **within 30 us of each other**. That is the answer to the question `GRAPHICS.md` had been
  answering by assumption: **a hardware blit and a shader that samples the cache cost the same on this
  device.** So the retracted sentence ("the blit is cheaper because it samples a framebuffer attachment")
  was not merely unexplained — the effect it claimed does not appear at this resolution.

  A cached static frame is therefore **0.23–0.26 ms at 1.86 Mpx against a 1.5 ms budget**, comfortably
  met on either number, and the full shader also fits at 1.86 Mpx. **These are laptop numbers, not
  Reference B**; the Mali-G51 has not been run through this instrument.

  **Two more instrument bugs, both found by this run and both now covered by cases 9 and 10 of
  `tools/check-ablation.mjs`:**

  3. **It printed a threshold that governed nothing.** The header read *"vs the larger of the 5% error
     bar and the timer floor (63 us)"* and then listed a **50 us** delta (`− body sphere test`) as a
     finding. Not a contradiction in the code — each row is judged against **its own** floor, and a row
     that ran ~100 frames has a 10 us floor, not the 63 us the widest row had — but a single headline
     number standing in for a per-row rule reads as the page arguing with itself. The header now names
     both bars, says each row is judged against its own, and gives the widest floor as a bound rather
     than as the rule.
  4. **A 0.0% error bar was reported as excellence.** Control and baseline both printed 0.767 ms, so the
     page said *"Good: the instrument agrees with itself to within 0.0%"*. With a 1 ms timer, two
     readings less than a tick apart are indistinguishable: 0.0% was the **display hitting the floor**,
     not the instrument being precise. It now says *"agrees to better than one tick, which is all this
     instrument can claim"* and states that one tick is 0.1% of the baseline, so nothing finer is
     resolvable. This is the same species of error as the refresh-period "blit" — a number at the
     resolution limit presented as a measurement.

  **Third run, same laptop — the `spread` column was decoration.** The page computed every row's
  min-to-max spread, printed it, **coloured anything over 10% red**, and then used it in **zero
  decisions**; `summarise()` mentioned it once, in a sentence telling the *reader* to check it. The
  source comment still claimed *"a delta smaller than the spread is printed as 'indistinguishable'"* —
  which the code did not do. A computed column that decides nothing is the same failure as the
  refresh-period blit: a number presented as if it were doing work. (`check-ablation` case 11 now
  asserts `spread` appears in executable code, and verifies the case fails against the old page.)

  **What that run actually showed, and it is more interesting than a bug.** `baseline` had a **205%**
  spread and `control` **5%** — the *same program, same code path, different slots of the round*. The
  page announced *"Good: the instrument agrees with itself to within 2.1%"*, a true sentence about the
  medians printed directly above a red 205%. The difference is **position in the schedule**, not the
  shader: whichever row goes first each round is stalled on something the others are not. The median
  survives it (0.783 vs 0.767 while the samples span 205%), which is exactly why the median is the
  reported statistic — but the page now says both things, and says **POSITIONAL effect** by name when
  the medians agree while one row is unsteady (case 12).

  The spread is now a **third bar** on every delta, alongside the relative error bar and the row's own
  timer floor, and a rejected row names which of the three rejected it.

  **What survives as a finding, and what does not.** Against the larger of the error bar and the timer
  floor: the **solar-disc shadow is real at 0.15 ms** (20% of the frame, and 1.2 ticks — the one row
  that clears both bars). The other six are **not findings**: wall corrugation 0.02, aerial perspective
  0.03, body sphere 0.05, tonemap 0.01, sky glow 0.00, 8→4 taps 0.00 ms. The sphere test's 50 us clears
  the error bar and dies on the 63 us tick, so it is **unresolved rather than absent** — a finer clock
  would settle it.

  **Still unknown.** This is one device, one run, and it is not Reference A or B — it is a laptop whose
  GPU the page does not name in the copy returned. The four rows the old 43% run "survived" are still
  not replicated on a named reference device. What has changed is that the instrument now agrees with
  itself to 2.8% and reports its own resolution, so the next run is worth reading.
- **The style gates are armed (2026-11-23), and getting there cost more than it should have.** Both
  `rustfmt` and `clippy` were `continue-on-error` until they went green on `main`, which is the
  condition §3 set. `clippy` went green first. `rustfmt` took eleven files and about as many rounds,
  for two reasons worth keeping:
  - **The annotation caps at six diffs.** A run showing six is a *truncated list*: clearing them
    reveals six more. Chasing hunks is the wrong shape; clear the file the job names, whole.
  - **The sandbox still has no Rust toolchain**, so a formatting edit cannot be checked before it is
    pushed. What broke the loop was `@scalar/rust-fmt` — rustfmt compiled to wasm, running on plain
    Node. Its bundled rustfmt is an *older release* and disagrees with the pinned 1.83.0 on at least
    one construct, so it is a good oracle for a red file and not a safe one for a green file. Every
    change it proposed was verified as whitespace-only (strip whitespace, normalise trailing commas,
    require an identical token stream) before being applied.

  The earlier version of this bullet said the gate "stays red until a local `cargo fmt` is possible".
  That was too pessimistic by one npm package, and the cost of being wrong was a gate that could not
  fail a branch — a red result carrying no information at all.

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
| ~~I cannot compile Rust in this sandbox~~ **Corrected 2026-11-24** | The toolchain is installable from the reachable npm registry as `@rustbin/*` packages (§2.1), so Rust *is* compiled and tested here — on **1.85.0**, not the pinned **1.83.0** | Small crates, std/core only, tests shipped, and CI still treated as the authority: a local pass is evidence, a pinned-toolchain pass is the proof |
| **I cannot measure GPU performance** | SwiftShader is a CPU rasteriser; its timings are meaningless | The instrument ships in the product; you are the only performance measurement |
| **WebGL2 only exists here through a bundled Chromium** | Harness is fiddly (Brotli unpack, `LD_LIBRARY_PATH`) | Keep the harness script in `tools/`, document it, never make the *product* depend on it |
| **The kernel/presenter boundary erodes** | It is the easiest thing to violate under deadline | A CI check that kernel crates do not depend on web/present crates, and the actuator-only lint from `VISION.md` law 4 |
| **Determinism is discovered late** | It is invisible until a hash mismatches | `dmath` + golden hashes in P0, native *and* headless browser |
| **Licence slip on a dataset** | Easy, and only bites at release | `DATA-LICENSING.md` manifest + the two CI gates, from the first dataset |
| **Scope** | A decade-sized vision with a solo dev | The block ladder, a shipped build per phase, and a kill criterion per phase |

The one that matters most is the first two. **You are the only performance measurement I have, and the
only compiler.** The plan is shaped so that neither of us is asked to pretend otherwise.
