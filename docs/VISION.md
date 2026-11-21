# Odderon — Vision Synthesis & First-Principles Architecture

Status: **proposal for review**, not a commitment. Written from `IMPORTANT-1.md` and `IMPORTANT-2.md`.
Purpose: state back what Odderon is, convert the philosophy into mechanisms that can be enforced by a
compiler, a test suite, or a CI check, and name the places where the promises fight each other.

---

## 0. One sentence

> **Odderon is a real aerospace workstation and a real universe, disguised as a summer afternoon.**

The disguise matters as much as the physics. The simulator is the product; the warm sunlit
world is the reason anyone stays long enough to *become* an engineer inside it.

---

## 1. The thesis and the flywheel

Everything in the design docs reduces to one loop that no game has closed:

```
   physics gives you an advantage  ->  you learn the physics to win
            ^                                   |
            |                                   v
   a stranger trusts the proof  <-  your work product is real
```

Most "educational" games break at step 3: the skill is real but the artifact is a game score.
Odderon's whole argument is that the artifact can be **real** — a prediction card, a Pareto point,
a debrief, a report, an exported GMAT script or CEA input deck. That is the only reason a NASA
flight director would ever tell a new hire to play it.

So the real product is not "a space sim". It is:

1. a **physics kernel** that is honest enough to be trusted,
2. an **interface** that teaches by consequence instead of by tooltip,
3. an **artifact pipeline** (Eidos → Akashi → Esse) that produces things a professional can read.

### The emotional engine (not decoration)

The art direction is not a coat of paint on a simulator; it is the simulator's pacing device.

- **Warm vs. cold.** A sunlit corrugated workshop and cicadas on one side; 2.7 K background and
  silent razor physics on the other. The contrast only exists if both ends are rendered with
  conviction. The return to Earth must feel like relief — that is what stops two hours of Hohmann
  transfers from being fatiguing.
- **The cramped start.** You wake up in one small room. This is a psychological trick with an
  engineering payoff: *a small, fully-simulated room is also the perfect first build target.*
  Every watt of fidelity we can afford goes into 25 m² of space, and the universe feels enormous
  precisely because that room felt small.
- **The first five minutes.** No tutorial popup. A humming window AC unit, a broken visor, a
  tram you will miss if you don't leave now. The game teaches its input model with physical objects.
  By minute 5 the player has seen a static fire. By minute 30 they have caused a failure and
  diagnosed it. If those two numbers slip, the design has failed regardless of physics accuracy.

### The name

"Odderon" is a real object in physics: the C-odd partner of the Pomeron, the odd-parity Regge
trajectory, still being pinned down experimentally at the LHC. Fitting for a game whose subject is
the *odd* state of things — the failure, the asymmetry, the design that shouldn't work but does.

---

## 2. What Odderon refuses to be

| Not this | Because |
| --- | --- |
| KSP with better graphics | Lego parts + scaled solar system = wrong instincts. Our parts are parameters, not prefabs. |
| Children of a Dead Earth | Beautiful physics, no economy, no life, no reason to come home. |
| A NASA tool with a skin | GMAT/CEA/STK have no loop. We borrow their *math*, not their UX. |
| Star Citizen's fantasy physics | 200 m/s dogfights are a lie. 7 km/s orbital intercepts are a puzzle. |
| No Man's Sky's procedural galaxy | If we invent a planet, we have taught the player something false. |
| A tech tree | Technology is a maturity level + explicit assumptions, not an unlock token. |

---

## 3. The Eight Laws → enforcement mechanisms

A design principle that isn't enforced is a wish. Each law gets a mechanism, most of them cheap:

| # | Law (from your notes) | Enforcement mechanism |
| --- | --- | --- |
| 1 | **Real physics, no shortcuts.** Conservation of energy / momentum / mass absolute; no FTL; relativity where it matters. | One kernel; all state changes routed through conserved quantities. No "cinematic" branch anywhere. A published validation suite: engine results compared against NASA CEA decks, JPL Horizons states, and analytic limits (Kepler, vis-viva, rocket equation) as CI tests. |
| 2 | **Real universe. No fantasy procgen; unknown is never shown as fact.** | A `Provenance` type: every number carries `Measured { σ } / Modeled { model, σ } / Assumed { basis } / Speculative { assumptions }`. The renderer and the UI **cannot** display a value without provenance. World data comes only from catalog octree tiles; the only randomness permitted in world data is sampling from a *declared hypothesis distribution*, which is labeled as such on screen. |
| 3 | **Transferable skill is priority #1.** | Artifacts export to formats real tools read: GMAT script, NASA CEA `.inp`, SPICE SPK/PCK, CCSDS packets, STEP. Predictions are committed (hashed) *before* flight, never after — so calibration can't be back-filled. |
| 4 | **Zero UI.** No "add 100 kg propellant" buttons; hands-on, physical controls and real instruments only. | The player→world API surface is a **control-panel schema**, not a command API: `Actuator::{ValveOpen, Throttle, Gimbal, Switch, Hand}`. There is no verb in the kernel for "set fuel mass" or "execute burn node". A CI lint rejects any new public kernel entry point that isn't an actuator. |
| 5 | **Zero 3D assets.** Every nozzle, tank, truss, cabin pipe, and cockpit instrument generated from math. | CI budget: repo contains no binary asset files above a small threshold; a test asserts every renderable has a parametric constructor. Geometry is `f(parameters)`, so a design change is a uniform update, not a mesh download. |
| 6 | **Near-zero CPU, integrated GPU, browser, scales to 100k ly.** | Event-driven kernel: the universe is a **queryable function of time**, never a per-frame loop. Presentation freezes entirely when nothing changes. Measured acceptance criteria, not adjectives (see §8). Hierarchical frames: `i128` sectors + `f64` local frames. |
| 7 | **Open-ended.** Pad tech, CEO, backyard inventor, freelancer, pirate, terraformer, megastructure captain — none required. | No quest gates. Contracts, clients, and customers are **data generated from real catalogs and real economics**; the career ladder is an emergent consequence of reputation and balance sheet, not a state machine. Systems first, content second. |
| 8 | **Learning must be addictive; no jargon barrier, no googling "retrograde".** | Language lives in the world, not in a wiki: the Eidos visor shows cause and effect before it shows a term. Terms are introduced *after* the player has already succeeded at the thing. A scripted "first 30 minutes" acceptance test run in CI as a design requirement, with telemetry on where players quit. |

---

## 4. Where the promises fight each other

Being honest about the collisions is more valuable than pretending they don't exist.

| Tension | Why it bites | Resolution |
| --- | --- | --- |
| **Zero CPU vs. first-person rendering** | A walking human is a continuous 60 Hz workload. "0.0% CPU" cannot be literally true while moving. | **Resolved by architecture, not concession** (`PERFORMANCE.md` I1/I3/I8). There is no game loop: an event queue drives a presenter, and the renderer *samples* a state ring at whatever rate it likes — including zero. Kernel and presentation are decoupled, so idle is literally "nothing scheduled". While moving, the honest claim is not 0% but *bounded*: ≤ 11 ms worst case, measured and gated in CI, and input→audio latency held at ≤ 8 ms independent of frame rate. |
| **Zero assets vs. text, sound, and human faces** | Fonts are assets. Voice is assets. Parametric human faces are the hardest problem in this document. | Whitelist, stated openly: a procedural **stroke font** (~4 KB of coordinates) for instrument faces and labels; the OS font stack for dialogue; **all** sound synthesized in WebAudio from kernel numbers; humans stylized, cel-shaded, and rarely close-up (Shinkai's own compositions favor backs, distance, and silhouette). No pre-rendered anything. |
| **Real universe vs. 4000 ly of data** | Gaia is magnitude-limited and dust-extincted; completeness collapses beyond ~1–3 kpc. The catalog *has holes*. | That is the scientifically correct outcome, so we lean into it: model 3D dust extinction explicitly. The galactic interior is dim and hidden — as it truly is. Unexplored space is dark, and funding a survey is a real activity. Nothing is invented to fill the gap. |
| **Determinism vs. live NASA sync** | Verifiable replays require a frozen universe; live updates change it. | **Universe epochs.** A catalog version is part of the epoch ID. Replays, leaderboards, and Esse are pinned to their epoch. A NASA delta-sync creates a new epoch; comparisons across epochs are marked as such, never silently averaged. |
| **1:1 real life vs. hundreds of hours of tedium** | Doing a 45-minute cryo load by hand is thrilling three times and unbearable the fiftieth. | Automation is a **physical, player-authored subsystem** (a logic rack wired to sensors), not a menu setting. Time compression is only available in diegetic states: in the bunk, in cryo, or during certified autonomous fleet operations. Never mid-manual-flight. |
| **Deep simulation vs. an integrated GPU** | CFD, FEM, and 10,000 agents are off the table by definition. | 1-D/quasi-2-D networks instead of 3-D fields; analytic models (closed form, Chebyshev, Jacobians) instead of solvers wherever possible; the active bubble is the only thing that integrates numerically. |
| **Esse as a real credential vs. a client-side game** | A browser-side score is cheatable, therefore worthless to a recruiter. | Commitment + re-simulation: see **Proof-of-Flight** in §7. The credential is a deterministic replay anyone can verify, not a number a server asked you to trust. |
| **Professional credibility vs. mass-market fun** | JPL-grade rigor and a 60-second onboarding are opposites. | Two rules on the same world: the **Eidos visor down** experience (guided, causal, jargon-free) and **visor up** (raw instruments, no annotations, ranked). Same physics, two difficulty curves. |

---

## 5. Architecture strawman

### 5.1 The single most important idea: *time is a query, not a loop*

```
trait Trajectory { fn state_at(&self, t: Epoch) -> State; }
```

- The universe stores **functions of time**: Keplerian + J2/J4 secular drift for coasting bodies,
  Chebyshev tables for real ephemerides, dense-output tapes for anything that thrusted, and a
  live integrator only for the *attention bubble* around the active observer.
- Nothing "ticks". Mars does not move at 60 Hz. It is a closed form that gets evaluated when
  something looks at it.
- **Signals are queued, not shared.** A telemetry packet is `Signal { origin, t_emit, payload }`
  and arrives at `t_emit + |r_observer − r_origin| / c`. That *is* the Light-Cone Ledger — it falls
  out of the architecture rather than being bolted on.
- **Save files are kilobytes.** The universe is regenerated from (epoch, catalog hash, seed), so a
  save is an epoch + input log + deltas. No state dumps, no serialization of 50,000 asteroids.

This one decision is what makes "0% idle CPU", "scales to 100k ly", "no global god clock", and
"async multiplayer" all true at once.

### 5.2 Determinism (required for Esse, Akashi, and replays)

WASM guarantees IEEE-754 semantics for `+ − × ÷ √`, but **not** for `sin`, `cos`, `exp`, `log`,
`atan2`, or `pow` — those come from the host libm and differ between browsers and native builds.
Therefore:

- `odderon-dmath`: our own polynomial-approximation transcendentals with fixed coefficients. Same
  bits natively and in every browser.
- No `f64` NaN-dependent branching, no `HashMap` iteration order in the kernel, no wall-clock reads.
- Fixed integer tick: `t` in µs since J2000 (TDB), `i64` (±292,000 years) with an `i128` deep-time
  variant; positions are `i128` sector index + `f64` local frame.
- CI runs golden-hash tests **natively and in a headless browser**; a mismatch is a build failure.

### 5.3 Crates

```
odderon-dmath      deterministic math, no_std, bit-reproducible transcendentals
odderon-astro      Kepler/universal-variable solvers, Chebyshev ephemerides, Lambert,
                   J2/J4 secular, IAS15 + RKF45, relativistic corrections
odderon-thermo     1-D fluid/thermal network, Gibbs equilibrium (CEA-compatible thermo data),
                   materials from a real properties table, structural stress
odderon-sim        entities, event queue, light-cone signals, actuators, contracts, economy
odderon-geom       parametric geometry: surfaces of revolution, splines, corrugations, CSG-lite
odderon-render     WebGL2 via `glow`; analytic shaders, gobos, matte layers, render-on-demand
odderon-ui         instrument faces (Canvas2D vector), stroke font, diegetic panels, Eidos overlay
odderon-verify     input logs, hashing, replay bundles, PDF/HTML report generation
odderon-preprocess NATIVE: Gaia DR3 → octree tiles, Horizons/SPICE → ephemerides,
                   Exoplanet Archive → priors, CEA thermo db, 3D dust map
web                wasm-bindgen glue, worker, tile streaming (range requests)
tests/validation   comparisons vs NASA CEA, Horizons, and analytic limits
```

No game engine. No Three.js. No Unity. `wasm-bindgen` + `wasm-pack`/Vite (worker support matters
more than convenience); decided at M1, not now.

### 5.4 Rendering: how the summer noon is actually cheap

The reference image is expensive-looking for four reasons, and all four are per-pixel math:

1. **Warm sun vs. cool sky-shadow.** One stylized PBR shader: direct term = solar blackbody
   (~5500 K) shifted by atmospheric path length; ambient term = Rayleigh sky blue from above +
   grass bounce green from below. Zero texture lookups.
2. **Procedural surface detail.** Corrugation = a sine perturbation of the normal in the fragment
   shader; roof tiles, weld beads, AC grille, concrete pitting = the same trick. This is where
   "0 asset textures" pays off: infinite detail, one draw call.
3. **Analytic gobos.** Dappled tree/power-line shadows are computed in-shader as projected
   analytic silhouettes along the solar angle — no shadow map, no tree geometry.
4. **Sky as a full-screen shader.** Rayleigh/Mie gradient + cumulonimbus billows via SDF, at half
   resolution with temporal reuse. This is the cheapest convincing sky ever made and it is *the*
   Shinkai signature.

**Fidelity-as-attention (matte layers).** Distant city, hills, and far launch pads render as
painted parallax layers — analytic layers, not textures — that resolve into real parametric
geometry when the player travels there. This is LOD, but framed honestly: the far world is
attention, the near world is simulation. It is also how a 2015 laptop gets to render a coastline.

**Render-on-demand.** When input is idle and no sim event is pending, rAF is not scheduled. Move
nothing, and the frame is a static painting at 0% GPU. This is not a trick; it is the correct
architecture for an instrument panel.

### 5.5 Input model (how Zero-UI is actually enforced)

```
Actuator::ValveOpen(id) | Throttle(f32) | Gimbal(x, y) | Switch(id, bool) | Hand { pos, grip }
```

Loading propellant is a **mass-flow simulation**: you open a valve, hear the line groan, watch the
thermocouple, manage ullage with helium, and chill the pump. There is no code path that adds
propellant, because there is no such verb. The same is true of burns, docking, and repairs.

Automation is a **physical logic rack** you wire to sensors — `IF pump_housing_temp > 92 K THEN hold
ignition` — compiled into the same deterministic tick loop. It is exportable text, so it can be
shared, reviewed, sold on an in-game market, or pasted into a real PLC discussion.

### 5.6 Data pipeline

- Offline Rust preprocessor (never at runtime): Gaia DR3 → quantized octree tiles (per-tile origin,
  u32 deltas, proper motion, colour index), JPL SPICE → Chebyshev subsets, NASA Exoplanet Archive →
  mass/radius/orbit priors with real error bars, 3D dust extinction map, CEA thermo coefficients.
- Runtime streams only the tiles being observed, via HTTP range requests, with a hard resident-memory
  cap. Expected download for M1 is a few MB; the full 4000 ly catalog is a streaming resource we
  meter and validate — not a claim we make before measuring.
- Daily delta-sync against the Exoplanet Archive TAP API; each sync mints a new **epoch**.

---

## 6. The pedagogical machine, implemented

| Concept | Concrete implementation |
| --- | --- |
| **Predict → Execute → Debrief** | A typed Prediction Card, hashed and committed *before* approval. After flight, the debrief overlays predicted vs. actual curves and names the divergence point. |
| **Eidos ("visual partial derivative")** | Because every model is analytic, the kernel can produce exact or finite-difference **sensitivities** on demand in milliseconds. Hover the nozzle expansion ratio and the sim returns ∂Δv/∂ε, ∂T/∂ε — real numbers, drawn as flow lines. Eidos is nearly free *because* we refused to make the sim a black box. |
| **Debrief root cause** | The divergence is attributed automatically via the same sensitivities: "T+42 s: you used vacuum Isp 348 s; at sea level the wide nozzle was over-expanded, actual Isp 265 s, flow separation." No hand-written string required. |
| **Ghost Run** | One counterfactual replay, side by side with the real flight. The fastest intuition-building mechanic in the game. |
| **Akashi** | Deterministic report from (design hash, epoch, input log): CAD cross-sections, P&ID, porkchop, telemetry, margins, player-written RCA. HTML/PDF export. |
| **Esse** | Three pillars — predictive calibration (proper scoring rules, not accuracy vibes), Pareto contribution on weekly global contracts, blind anomaly resolution under a clock. Deterministic scoring so it can be recomputed and audited. |
| **Beginner's ladder** | Visor down: cause-and-effect language, no jargon. Visor up: raw instruments, sextant, audio only. There is no separate "easy mode" — the same physics, annotated or not. |

---

## 7. Innovations I would add (beyond the docs)

Each one is chosen because it serves law #3 (transfer) or law #6 (cost), not because it is clever.

1. **Proof-of-Flight — offline-verifiable Esse.** A mission records a <100 KB input log (actuator
   deltas + seeds + epoch). Any machine can re-simulate it in seconds and match a state hash.
   The credential becomes *re-executable*, like a proof, instead of a number a leaderboard asserts.
   A recruiter clicking the link in an Akashi PDF runs the mission themselves. Anti-cheat stops
   being moderation and becomes arithmetic.
2. **The Ledger of Ignorance.** Provenance + uncertainty as first-class types, so it is
   *impossible* to render an assumption as an observation. Telescope views blur by real angular
   error bars; a planet's surface stays a probability cloud until an instrument narrows it. Your
   law #2 becomes a compiler-enforced invariant rather than a promise.
3. **Causal Mail — multiplayer without netcode.** Since the universe is a function of time and the
   kernel is deterministic, two players can share an epoch and exchange **signed event logs**.
   Your friend's ship appears in your sky; their message arrives delayed by real light-time because
   it genuinely travels at c. No state sync, no servers per session, no tick reconciliation.
   Asynchronous co-op that is *thematically* correct is essentially free here.
4. **The Export Bridge.** Every artifact leaves the game in a format a real tool consumes: GMAT
   scripting, CEA input decks, SPICE kernels, CCSDS packets for hardware-in-the-loop, STEP-ish CAD.
   This is the adoption wedge: a university rocket team will adopt Odderon as a *front end* months
   before anyone calls it a game. "NASA recommends it" is earned this way, not by marketing.
5. **Inverse-design lens.** Because the model is differentiable, the CAD terminal can show the
   *feasible manifold* — the surface of designs that meet a constraint — without ever highlighting
   "the answer". You teach the shape of the problem, not the solution.
6. **Anomaly library from real accidents.** Ranked blind-anomaly missions seeded from public mishap
   reports (NTSB/NASA/ESA/commission findings), graded against the actual investigation. Real
   failure modes are stranger and better than invented ones, and the player learns real forensics.
7. **Temporal ledger.** Proper time accrues per POV. Your CEO ages; your 0.8c astronaut doesn't.
   Crew, relationships, and company state belong to their own clock. No game has made relativity
   *personally* expensive before, and it costs us nothing extra — the clocks already exist.
8. **Kilobyte saves.** A consequence of §5.1 worth stating as a feature: your entire universe history
   is a text file you can email.

---

## 8. Milestone ladder — a vertical slice must contain all five promises at 1% scale

The failure mode for a vision this large is building one subsystem perfectly for two years. The
antidote is a slice that is *small in scope but complete in promise*.

**M1 — "The Room."** WebGL2+WASM canvas; one parametric apartment (corrugated wall, wooden desk,
window AC, power lines, swaying grass); analytic gobos; sky + cumulus shader; real solar ephemeris
for the chosen town and date; a distant pad plume with shockwave arrival at 343 m/s; walking and
object interaction; render-on-demand.
*Acceptance:* 0 scheduled frames/s and 0 timers when idle; **60 fps at 1080p output** on an Intel
UHD 620-class iGPU at ≤ 11 ms worst-case frame time and ≤ 8 draw calls (see `PERFORMANCE.md`); < 3 MB
download; zero binary assets. The frame budget is a CI gate, not an aspiration — a build that
exceeds it fails.

**M2 — "The Pad."** 1-D cryo/fluid network (chilldown, boil-off, ullage, cavitation), a static
fire with a real failure mode, visor diagnosis, a hashed Prediction Card, Akashi HTML export.
*Acceptance:* the failure is reproducible from the input log; the report explains it from
sensitivities, not from a script.

**M3 — "The Sounding Rocket."** Parametric CAD build (tank + hybrid motor + nozzle), manual
launch, apogee telemetry, Ghost Run debrief, first Esse event, Proof-of-Flight bundle.
*Acceptance:* an exported bundle re-simulates bit-identically in a second browser.

**M4 — "The Orbit."** Keplerian coast + J2, rendezvous, light-time comms, first orbital contract,
the first module of the light-cone ledger.

Each milestone ships a public web build. Nothing is held back for a "big reveal".

---

## 9. Risk register

| Risk | Severity | Mitigation |
| --- | --- | --- |
| Scope: this is a 100+ person-year vision | **Fatal if unmanaged** | Milestone ladder above; each milestone is demoable and independently valuable; never start a subsystem without a slice that consumes it. |
| "Zero assets" produces ugly humans and UI | High | Art direction avoids the uncanny valley by composition (distance, backlight, silhouette, cel shading); stroke font for instruments; OS font for prose. Accept a small, declared whitelist. |
| Integrated-GPU fill-rate limit at 1080p | Medium | Half-res sky with temporal reuse, analytic shadows instead of shadow maps, matte layers, render-on-demand. Benchmark in CI on a weak GPU profile. |
| Determinism drift across browsers | High (breaks Esse) | Own `dmath`, no host transcendentals, golden-hash CI in native + headless browser. |
| Catalog incompleteness at 4000 ly | Medium (it's a feature if handled honestly) | 3D dust extinction, explicit "unknown" rendering, survey missions as gameplay. Never invent. |
| Browser memory ceiling for star tiles | Medium | Hard resident cap, LRU eviction, coarse tiles with on-demand refinement. |
| Credibility ("why would NASA care?") | High (it's the whole thesis) | Publish the validation suite (CEA/Horizons agreement), the export bridge, HIL via WebSerial, and open the physics kernel. Adoption by university teams is the realistic first proof. |
| Real companies/brands, real people | Low–medium | Fictional analogs for corporations; no likenesses; public-domain literature only for performance data; keep an eye on export-control-adjacent material. |
| Economy/politics of "Esse as a resume" | Low | Start as self-published signed bundles; any hosted ranking is opt-in and clearly separate from the game. |
| Solo-dev burnout on an infinite vision | **Fatal if unmanaged** | Same as scope: ship M1 as a thing people can use. Small rooms, real sunlight, finished. |

---

## 10. Open questions for you

1. **What should I produce next?** (a) start M1 "The Room" as working code; (b) a deeper
   architecture spec for one subsystem (determinism / time-as-query / data pipeline); (c) a game
   design bible (loops, progression, economy, UI flows); (d) the credibility package (validation
   suite + Esse/Akashi + export bridge).
2. **Who writes the code?** You and me together, or me with you reviewing, or spec-first because
   you plan to bring in people?
3. **Which wedge gets built first** — ground ops (pad technician), cockpit flight, the CAD
   terminal, or the orbital freelancer?
4. **Online from day one, or local-first with verifiable bundles and a server later?**
5. **Data purity vs. download size** — stream real catalogs (hundreds of MB on demand), cap
   everything under ~50 MB, or real solar system + real nearby stars with statistical priors beyond?
6. **What is your actual machine?** "No dedicated GPU" covers a 2015 dual-core with 4 GB RAM and a
   modern i5 with Iris Xe, and those are different games. I will write the acceptance numbers to
   your hardware.
