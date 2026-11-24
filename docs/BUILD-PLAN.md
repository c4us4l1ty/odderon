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
| The shell (WASM + WebGL2) | **renders, measured on real silicon** | Two passes: an expensive shade into a one-target cache, and a blit. Measured on Reference B (Mali-G51, 1.04 Mpx): flat fill 0.67 ms, cached frame **1.25 ms**, full shade 48.71 ms. **R2 passes at 1.25 ms with 17% margin**; R3 is met by construction at 8 B/px. `tools/check-shell.mjs` and `tools/check-cache.mjs` are the standing tests, and **both now run in CI** — `check-cache.mjs` did not until 2026-11-24, so "standing test" meant "runs when somebody remembers", which is a claim rather than a check. |
| The Room (thermal) | **claims 1–4 measured; claim 5 is a judgement and open** | `thermal.rs`: the §4 claim 2 measurement, as a lumped zone and a vapour-compression loop. The bible's own arithmetic reproduces exactly — 3 m² × 800 W/m² × SHGC 0.6 = **1440 W** open, **120 W** with the curtain closed (SHGC 0.05) — and the coupling is measured rather than argued: on a real 12 000 BTU/h unit (EER 11, COP **3.224**, which the Copeland correlation reproduces to **0.9 %**), the draw falls from **713 W to 304 W**, a **57 %** drop visible on a meter. The drop is *not* the load ratio (59 %), because ASHRAE's cyclic degradation coefficient makes a cycling unit worse at part load — the penalty works **against** the drop, which is the conservative direction. The room cools **5.9 K in the first ten minutes** after the curtain closes, against a `C/UA` time constant of **44.4 min**; that 5.9 K is what chose the zone's 120 kJ/K capacity, which is 1.59x the 75 kJ/K of air the room holds — the air alone would cool several times faster, and the three plausible capacities give 17.7 K, 8.9 K and 5.9 K in ten minutes, so the claim holds across the range and 120 kJ/K picks where in it to sit. The integrator is explicit Euler with a **measured** stability bound rather than a trusted one: at `C/UA` it converges, at 3× it diverges, and the bound returned sits between them. 22 tests. This row was rewritten once: the first version reported 464 W → 190 W from a COP of 4.82, which is EER 16.4 — a number no window unit in this class achieves, produced by an `η₂` quoted from a range rather than checked against a nameplate. The omissions are named in the module and are real — no latent heat (so the draw is *understated* on a humid day), one thermal node, no surface-to-surface radiation, and a COP correlation rather than a cycle solved from an equation of state, which is why the real-gas row in `CREDIBILITY.md` §2 still demands REFPROP. **Claim status, checked rather than remembered** (this row previously said "claims 1, 3, 4, 5 not started", which was wrong in three places): **1** passes — `kernel-policy.sh` rule R5 refuses any tracked binary asset, and `git ls-files` finds none. **3** passes *and is now gated* — `tools/check-shell.mjs` measures 0 frames drawn while idle, and a new `shell-claims` CI job runs it on every push. **4** passes on Reference B at 1.25 ms against a 1.5 ms target. **5** is open, because it is the one claim no instrument can settle: the frame is captured by `tools/screenshot.mjs` into `.artifacts/` (uncommitted — R5 forbids the PNG in the tree) and has to be looked at by a person. |
| Kernel in the page | **blocked, deliberately** | `gh run download` fails with a blob-storage EOF and `release-assets.githubusercontent.com` is unreachable from the build sandbox, so a CI-built `.wasm` cannot reach the page. Isolated behind `orbitPosition(t)`. **Not urgent, and here is why:** the shell draws it every frame, so if the kernel is ever slow it is invisible next to a 48.7 ms shade. The transport gets fixed when the shell's wiring is finished and a real mission wants to be drawn — not before, because building it now would optimise a 0.001% term. |

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

  **Still unknown, and this is the part to be careful about.** The rewritten instrument has **not been
  run on Reference A or B**. The only run on record is this sandbox's SwiftShader, where the whole
  shader lands under the browser timer's resolution and the page correctly declines to report rows at
  all ("what is meaningful is that the shader is not the cost"). So there is currently **no number from
  the fixed tool**, for either reference device, and the old numbers must not be read as if there were.
  Until somebody runs `web/ablate.html` on real silicon, the four "surviving" rows above are
  unreplicated, and the per-feature costs of corrugation, aerial perspective and the rest are unknown
  rather than small.
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
| **I cannot compile Rust in this sandbox** | Every Rust deliverable is compile-unverified until it reaches you | Small crates, std/core only, tests shipped, first build treated as a fix-up pass |
| **I cannot measure GPU performance** | SwiftShader is a CPU rasteriser; its timings are meaningless | The instrument ships in the product; you are the only performance measurement |
| **WebGL2 only exists here through a bundled Chromium** | Harness is fiddly (Brotli unpack, `LD_LIBRARY_PATH`) | Keep the harness script in `tools/`, document it, never make the *product* depend on it |
| **The kernel/presenter boundary erodes** | It is the easiest thing to violate under deadline | A CI check that kernel crates do not depend on web/present crates, and the actuator-only lint from `VISION.md` law 4 |
| **Determinism is discovered late** | It is invisible until a hash mismatches | `dmath` + golden hashes in P0, native *and* headless browser |
| **Licence slip on a dataset** | Easy, and only bites at release | `DATA-LICENSING.md` manifest + the two CI gates, from the first dataset |
| **Scope** | A decade-sized vision with a solo dev | The block ladder, a shipped build per phase, and a kill criterion per phase |

The one that matters most is the first two. **You are the only performance measurement I have, and the
only compiler.** The plan is shaped so that neither of us is asked to pretend otherwise.
