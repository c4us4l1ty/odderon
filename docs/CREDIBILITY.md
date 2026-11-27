# Odderon — The Credibility Package

**Specification for the rebuild.** Companion to [`GAME-DESIGN-BIBLE.md`](GAME-DESIGN-BIBLE.md) and
[`VISION.md`](VISION.md).

The vision claims: *NASA/ESA/SpaceX would recommend this to their engineers.* That claim is either
earned by evidence or it is marketing. This document is the plan for earning it — and, more
importantly, the plan for **finding out we haven't** before we say it out loud.

Everything here is written to be attacked. A credibility package that cannot be attacked is not a
credibility package; it is a press release.

---

## 0. The uncomfortable premise

No game has ever been recommended by a space agency for engineering skill, and there is a reason:
games optimise for *felt* competence, agencies require *demonstrated* competence, and the two are
measured differently. The felt-learning literature is explicit that they can even move in opposite
directions (Deslauriers et al., 2019).

So the credibility thesis is not "our physics is accurate". Accurate physics is table stakes and, by
itself, worthless — GMAT has accurate physics and a hostile interface; CEA has accurate chemistry and
no simulation at all.

The credibility thesis is:

> **Odderon produces artifacts a professional can audit, in the formats their own tools already read,
> at a cost of minutes rather than weeks.**

That is the whole wedge. A student's Odderon report is checkable. A student's Kerbal screenshot is
not. A GMAT script is checkable but takes a semester to learn to write. An Odderon export takes a
Sunday afternoon and produces the *same script*, plus the flight that validated it.

The four pillars of the claim, in order of importance:

| # | Pillar | Why an engineer would care | Why a recruiter would care |
| --- | --- | --- | --- |
| 1 | **Auditable replay** (Proof-of-Flight) | They can re-run *your* mission and see if it was real | The credential verifies itself, for free |
| 2 | **Interoperable export** (GMAT/CEA/SPICE/CCSDS) | It drops into their existing workflow | It proves the candidate knows the workflow |
| 3 | **Validated kernel** (published test vectors) | Their V&V people can check our claims | It cannot be dismissed as "a game" |
| 4 | **Honest uncertainty** (provenance types) | It behaves like an engineering tool, not a toy | The candidate's numbers carry error bars |

Pillar 4 is the one nobody expects from a game, and it is the one that will make the other three
believable.

---

## 1. The claim ladder — what we are allowed to say, when

Credibility is a sequence, not a statement. Each rung Unlocks a sentence we may use publicly.

| Rung | Evidence required to stand on it | Sentence we are then allowed to say | Who must be able to verify it |
| --- | --- | --- | --- |
| **0. Now** | none | *"We are building a simulator whose physics is intended to be auditable."* | — |
| **1. Verifiable** | Kernel published as source; conformance suite public; test vectors downloadable | *"Every number in Odderon traces to a cited model, and our models pass the conformance suite below."* | Anyone with a compiler |
| **2. Interoperable** | GMAT/CEA/SPICE export passes the *import* tests of those tools themselves | *"Odderon output runs in GMAT and CEA without hand-editing."* | A user of those tools, in ten minutes |
| **3. Replayable** | Proof-of-Flight bundle verifies on a machine that did not produce it, in a browser with no network | *"You can verify any Odderon mission yourself; it is arithmetic, not testimony."* | A sceptic with a laptop |
| **4. Validated** | Independent third party (a university lab, ideally two) reproduces the V&V report and signs it | *"An independent lab has reproduced our validation suite."* | The lab, on the record |
| **5. Used** | Documented use by real teams: university rocket competitions, CubeSat programmes, coursework | *"N university teams used Odderon in <season>."* | The teams, named, with permission |
| **6. Recommended** | Institution publishes its own recommendation after its own trial | *"<Institution> recommends Odderon to <role>."* | The institution — and only the institution |

**We may never skip a rung, and we may never speak in rung 6 language from rung 1.** That single
rule is the difference between this document and a Kickstarter page.

Practical consequence: **rung 1 is achievable this year; rung 6 is a five-to-ten-year outcome and may
never happen.** Design the product so that it is valuable at rung 2 — a tool people actually use —
because that is the only state we can guarantee.

---

## 2. What the simulator is actually for (and what it is not)

Stated plainly, because overclaiming is the fastest way to lose the audience we want.

A flight director does not need another tool to compute a Hohmann transfer. They need practice at
**judgement under incomplete information with a clock running**. A student does not need another
textbook. They need to *feel* why a nozzle that is optimal in vacuum destroys itself at sea level.

| Odderon is | Odderon is not |
| --- | --- |
| A **judgement trainer**: procedure, sequencing, fault diagnosis, trade-offs | A certified trajectory tool |
| A **bridge** between textbook equations and physical intuition | A replacement for GMAT, STK, CEA, or STK's mission planning |
| A **simulator of consequence** for design decisions | A high-fidelity CFD/FEM solver |
| A **generator of auditable artifacts** | A certification authority |
| A **place to practise being wrong cheaply** | A game that promises you will be right |

And the sentence that must appear in the product itself, not just this document:

> **Odderon is a training simulator. It is not a flight-qualification, navigation-certification, or
> safety-critical tool. Nothing it produces may be used as the sole basis for a real launch decision,
> and no Odderon result should be used to justify a real vehicle's design without independent
> verification by qualified engineers.**

That disclaimer costs us nothing and buys us the right to be taken seriously.

---

## 3. The validation suite (V&V)

This is the heart of the package. The principle: **every model must be checkable against a source
outside the simulation**, and the tolerance must be stated *before* the test is written.

Location in the repo (planned): `tests/validation/`, one file per row, each emitting a machine-readable
result card (`odderon-vv-<case>.json`) so the whole suite renders as a public dashboard.

Tolerances below are **targets to be demonstrated**, not claims of achievement. A row marked
⚑ is one I consider genuinely hard and where I expect to miss first time.

### 3.1 Astrodynamics

| Case | Reference | Metric | Target | Notes |
| --- | --- | --- | --- | --- |
| Two-body Kepler propagation | Analytic Kepler equation | position error after 10 periods | < 1 m at LEO, < 1e-9 relative | Universal-variable formulation; must handle e→0 and e→1 |
| Energy conservation | Vis-viva | relative energy drift | < 1e-12 over 1000 propagations | **Status: implemented and passing** in `odderon-astro` (measured 1.3e-13). Note the theory: a *closed-form* propagator has no accumulation at all — its error is the rounding of the Lagrange f/g reconstruction, and it does not grow with the number of periods. The original target of 1e-14 was written for a numerical stepper. |
| Angular momentum | h = r × v | drift | < 1e-12 relative | **Status: implemented and passing** (measured 4.0e-14 over 500 propagations). |
| Period / mean motion | Kepler's third law | relative | < 1e-12 | |
| J2 secular rates | First-order secular theory (Vallado §9) | Ω̇, ω̇ over 30 days | < 1 % vs analytic first-order ⚑ | **Status: implemented and passing** in `odderon-astro::j2`. Real second-order effects make exact agreement impossible; we compare to the *same* first-order theory and separately to a high-precision numerical run. What is *measured today* is stronger than agreement with the formula: the special-value structure (Ω̇ = 0 at i = 90°, ω̇ = 0 at 63.4349°), linearity in J2, and reproduction of the published ISS-class drift of −5.0°/day and the sun-synchronous inclination. The 30-day comparison and the numerical cross-check are **not yet run** and are listed below as owed. |
| J2 sign convention | Published orbital drift for real satellites | nodal regression sign and magnitude | must be negative for prograde | **Status: implemented and passing** (−5.002°/day vs published −5.0). This row exists because the first implementation had the coefficient negated and produced **+5.002°/day** and a sun-synchronous inclination of 81.8° instead of 98.6°. Both were plausible; neither was flagged by anything until a reference value was checked. The test suite now pins all three symptoms. |
| J2 numerical cross-check | Dormand–Prince 5(4) integration of the real equations of motion | fitted secular rate over 7 days | < 1 % vs first-order theory ⚑ | **Status: implemented and passing.** Measured **0.435 %** on the nodal rate at LEO (400 km, i=51.6°, e=0.001), **0.439 %** at 700 km sun-synchronous inclination, **0.395 %** on a Molniya-class orbit, and **0.21 %/0.26 %** on `Ω̇`/`ω̇` for an eccentric a=12 000 km orbit. This is the check the row above could not provide: a formula cannot validate itself. The residual is physical, not a defect — short-period wobble in the node is 0.026° at 400 km and its second-order interaction with the mean flow is the same magnitude as 0.43 % of 5°/day, which is exactly why averaged theory suits a catalogue and not a rendezvous. The reference integrator is itself checked: `J2 = 0` conserves energy to 1e-9 and angular momentum to 1.7e-10 over 7 days, and under `J2` the **total** energy `v²/2 − μ/r + U_J2` is conserved to 4e-10 while the two-body part alone drifts by 2e-3, as it must. |
| Pass prediction | Closed-form visibility geometry | pass duration, synodic spacing | < 1e-3 s vs `2θ/(n−ω)` ⚑ | **Status: implemented and passing** in `odderon-astro::passes`. Measured agreement with the closed form is **1e-6 s** on a 652 s pass at two masks (0° and 10°), and the synodic spacing reproduces 5936.2388 s against an orbital period of 5553.6243 s. The guards are themselves tested for being *not stricter* than necessary, because a guard that refuses everything passes every refusal test. **The schedule is now J2-aware**: `next_pass` includes the first-order secular perturbation, and `next_pass_exact_geometry` is the two-body search the closed-form checks above are measured against — a distinction that had to become explicit, because the two disagree by 18.19 s in the beat between passes (5936.2388 s two-body, 5918.0468 s perturbed, for 400 km equatorial) and the closed form has no `J2` in it. The figure this row previously carried — "8.4 s on day 0 and 80 s on day 1" — was not reproduced by measurement and is replaced: at 400 km / 51.6° over a 30° N site the pass moves −75 s by the first observation and −445 s by the tenth, fitting **−149.8 s/day** with a 13.8 s residual. The perturbation itself is validated separately (`tests/j2-crosscheck.rs`, RK4 against the real force, 0.435 %), and it was checked against RK4 again at i = 0 while being wired in: real advance 16.2269°/day against 16.1067 from first-order theory, a 0.75 % agreement that is the theory's own truncation. |
| Earth rotation in the observer | Frozen-Earth variant of the same search | change in pass duration | must exceed 30 s ⚑ | **Status: implemented and passing.** Dropping the rotation rate from the *observer* only — the classic beginner bug, and the one that produces a plausible schedule minutes wrong — shortens a 400 km overhead pass from **652.3949 s to 610.3454 s**, a 42.05 s (6.4 %) difference. The test builds the frozen model itself rather than calling a library function, because it is a wrong model and should not have an API. Sign note: overhead, the rotation *lengthens* the pass; a station off the ground track sees the opposite sign. Both are real. |
| Geodetic vs geocentric latitude | Ellipsoid solution vs radius-vector "up" | station position, elevation bias | 21 km / 0.19° at 45° ⚑ | **Status: implemented and passing** in `odderon-astro::observe`. The polar radius reproduces WGS-84 to 6356.7523 km (21.4 km below equatorial), and the geodetic normal — not the radius vector — is what `look_at` measures elevation against. The failure this prevents produces no error and no NaN: every elevation is simply tilted by up to 0.19°, so passes start a few seconds early and the output looks entirely correct. |
| J2 + J3 + J4 propagation | Numerical integration, high tolerance | difference over 7 days | < 50 m at LEO ⚑ | **Status: implemented and passing.** The full zonal field (point mass + J2 + J3 + J4) is integrated through a Dormand–Prince 5(4) adaptive integrator (`odderon-astro::integrate::integrate_zonal`), with the acceleration derived from `U = −μ/r·[1 − J2(R/r)²P₂(s) − J3(R/r)³P₃(s) − J4(R/r)⁴P₄(s)]`. The integrator is self-consistent to **under 50 m over 7 days at LEO** (400 km, 51.6°): the 1e-11 and 1e-13 tolerance runs agree well inside the target. Energy is conserved under the full field (`v²/2 − μ/r + U_J2 + U_J3 + U_J4`) to 1e-9, and the potential's gradient is verified against `accel_zonal` by finite differencing at two test points including high latitude. J3 and J4 each produce a measurable drift from the J2-only integrator over 7 days, confirming the terms are applied and not cancelled. The rendezvous module exposes `propagate_zonal` and `zonal_vs_two_body_drift_m` to quantify the gap the two-body approximation makes — ~539 km after one day at LEO, the measurement that motivated this row. J2 alone remains cross-checked (row above) and in the pass search; the J2-secular approximation is **439 km** off after one day, so the full field is the only model within the < 50 m target. |
| Universal-variable edge cases | Analytic | propagation closure, energy, angular momentum | bitwise stable for e ∈ {0, 1±ε}, i ∈ {0, π} | **Status: implemented and passing** in `odderon-astro` (`tests/edge-cases.rs`, 11 tests). The propagator uses a **single code path** for every conic type and inclination — no branching on `e = 0`, `e = 1`, `i = 0`, or `i = π`. Standard orbits (e < 0.9) close a full period to **1e-12 relative** (7 µm at LEO); near-parabolic orbits (e ≥ 0.9, χ ~ 1e4) close to **1e-4 relative** (2 km at a=20000 km) — the rounding level for double precision, not a formulation defect. Energy is conserved to **1e-12** absolute for standard orbits and **1e-6** for near-parabolic (the near-zero energy makes relative error meaningless). Angular momentum is conserved to **1e-12 relative** at all boundaries. Hyperbolic orbits (e = 1+ε) are checked for convergence, finiteness, energy, and p conservation. True degeneracy (zero radius, zero μ) is still refused. |
| Interplanetary ephemeris | JPL Horizons state vectors, 20 bodies, 12 epochs spanning 1950–2050 | position | < 1 km inner planets, < 10 km outer ⚑ | Whatever the source ephemeris delivers; we report the source's own accuracy rather than pretend to improve it |
| Lambert solver | Published solutions (Vallado examples; Izzo 2015 test set) | arrival velocity error | < 1e-8 relative, multi-rev included | **Status: implemented and passing** in `odderon-astro::lambert`. Uses the y-parameter (Bate-Mueller-White) universal-variable formulation — the same Stumpff functions and Lagrange-coefficient family as the propagator. The Vallado/Curtis Example 5.2 case matches the published solution to **~1e-9 relative** on both departure and arrival velocity. Zero-rev, one-rev, and 3D transfers are verified by propagation self-consistency (the solved departure state, propagated forward by the flight time, reproduces the target position to < 1e-8). The 180° degenerate case is solved directly: for an elliptic 180° transfer the two positions are the apses, so the semi-major axis is determined from the flight time and the velocities come from vis-viva — exact, no perturbation error. Degenerate input (same-point positions, non-positive time) is refused with `unconverged: true`. **Not modelled:** multi-rev transfers — the solver finds the minimum-energy short-way solution only. |
| Interplanetary ephemeris | JPL Horizons state vectors, 20 bodies, 12 epochs spanning 1950–2050 | position | < 1 km inner planets, < 10 km outer ⚑ | Whatever the source ephemeris delivers; we report the source's own accuracy rather than pretend to improve it |
| Frame transformations | ICRF ↔ ecliptic ↔ body-fixed, via SPICE | quaternion round-trip | < 1e-13 | **Status: partially implemented.** Body-fixed frames are done and pinned physically (geostationary longitude, westward sub-satellite drift, IAU rate to 5.12e-6 %), not only algebraically — a zero `W₀` passes every algebraic test. The SPICE comparison and the ecliptic leg are **not done**: there is no ephemeris yet, and no ecliptic frame, so there is nothing to rotate between. Do not read this row as validated. |
| Relativistic corrections | Schwarzschild precession of Mercury | arcsec/century | within 1 % of 43.0″ ⚑ | Includes the known GR precession; tests whether we can *optionally* model it without breaking Newtonian cases |
| Time systems | TDB↔TT↔TAI↔UTC, leap-second table | round-trip, **and an absolute anchor** | < 1 µs round-trip; J2000 anchor < 1 ms ⚑ | **Status: implemented and passing** in `odderon-astro::time`. Round trips close to **1e-7 s** and the 2016-12-31 leap second is measured as three consecutive deltas of exactly 1.000000000 s, at all 28 table rows rather than the one date everyone remembers. But the round-trip target in this row was **not sufficient and is now understood not to be**: two wrong epoch constants passed it. Subtracting 32.184 s from the J2000 constant *and* applying the TT−TAI offset in the conversion leaves every round trip agreeing to 0.1 µs while every date sits 32 s off J2000, because a conversion and its inverse share the constant and cancel. The anchor that catches it is `utc_to_tdb(2000-01-01T11:58:55.816) = −72.60 µs` — TDB at J2000, which is the periodic term alone and cannot be anything else. That assertion is now the first test in the file. Three refusals are deliberate and typed: before 1972 (UTC was a rubber-second system), after 2026-12-31 (leap seconds are announced about six months ahead, so a later date is a guess rather than a computation), and a `23:59:60` on a day the table never recorded. **The upper bound is a published fact, not an extrapolation:** IERS Bulletin C 72 records `UTC − TAI = −37 s` from 2017-01-01 "until further notice", and no leap second has been inserted in the nine years since, so the announced offset covers the whole span to the end of 2026. That span is a real boundary rather than a chosen one — the next possible insertion date is 2027-06-30 and continuous UTC takes effect 2027-05-20 (CGPM Resolution 5, 2026). **Not modelled:** UT1−UTC, which is measured and revised rather than defined — so this module cannot point a telescope, and `CREDIBILITY.md`'s frame rows inherit that limit. The table's own expiry is the other honest boundary: it is correct through 2026-12-31 and *unknown* after, which is why the error type says so instead of extrapolating. |
| Numerical integrators | IAS15, RKF45, symplectic | energy error vs step size | matches published convergence orders | Convergence-order test, not just "it runs" |

### 3.2 Propulsion & thermodynamics

| Case | Reference | Metric | Target |
| --- | --- | --- | --- |
| Gibbs equilibrium solver | NASA CEA (Gordon & McBride) — 30 published cases across CH4/LOX, RP-1/LOX, H2/LOX, N2O4/MMH, solid propellants | T_chamber, Isp_vac, c* | < 1.0 % on Isp, < 0.5 % on T_c ⚑ |
| CEA import/export round-trip | Same 30 cases | our `.inp` read by CEA, CEA's result read by us | identical to within 1 % both directions |
| Ideal rocket relations | Tsiolkovsky, thrust coefficient, mass flow | closed form | < 1e-12 relative (exact to f64) |
| Nozzle flow: over/under-expansion | Method of characteristics reference curves; Rao bell approximations | thrust vs pressure ratio | < 2 % between separation onset points ⚑ |
| Flow separation criterion | Summerfield / Schmucker criteria | separation pressure ratio | within the empirical band, stated as empirical |
| Pump cavitation onset | NPSH definition | onset point | exact by construction (it is the definition) |
| Regenerative cooling | 1-D Bartz correlation vs published engine data (e.g. SSME-class cooling channel studies) | wall temperature | < 10 % ⚑ | Acknowledge: Bartz is an empirical correlation; we say so |
| US Standard Atmosphere 1976 | Published table, 0–86 km | ρ, T, p at 40 altitudes | < 0.1 % |
| Real-gas deviations | NIST/REFPROP data for LOX, LH2, CH4 near saturation | density | < 1 % ⚑ | Where we use an ideal gas, we must *declare* it. **Not met and not claimed.** `thermal.rs` added the first place the question can be asked — a vapour-compression loop — and it answers it the honest way: the COP comes from Copeland's correlation, tested against Carnot × η₂ exactly, and the module header says in as many words that this is a correlation and not a cycle solved from an equation of state. A correlation cannot be *wrong* about real-gas behaviour in the way a target can measure; it can only be unpinned. Pinning it needs REFPROP, which is not here. |
| Two-phase / cryogenic boil-off | Published tank boil-off data | mass loss rate | < 15 % ⚑ | Genuinely hard; the honest deliverable is a stated error bar |
| Radiator sizing | Stefan–Boltzmann + effective emissivity from published data | area for given heat load | < 5 % | **Half done.** `radiative_rejection` and `radiator_temperature` exist and invert each other to **1e-9** — the sizing direction is exact, because it is the same equation rearranged. The `< 5 %` target is **not** met and cannot be from this repo: it is a claim about ε, and this module ships **no ε at all** — it takes emissivity as an argument and validates the range, so there is no `Assumed` value to point at and none to blame. An earlier version of this row said ε was `Assumed` at 0.9 "with the basis stated", which was not true of the code; the 0.9 in the tests is a test fixture. Pinning the row needs a published emissivity table, which is a data licence question (`docs/CLASSES.md`) and not a physics one. Two measured facts are pinned instead, both of which a sign or exponent error would break: at `T_surr = 0` the rejection is `εσAT⁴` and doubling T multiplies it by exactly 16; against a realistic 273 K surroundings the same doubling gives **5.62**, not 16 — the naive ratio is a different equation, and it is the one that gets quoted. |

### 3.3 Entry, structures, environment

| Case | Reference | Metric | Target |
| --- | --- | --- | --- |
| Ballistic entry trajectory | Apollo/Stardust-type published entry profiles | peak deceleration, peak heating altitude | < 5 % ⚑ |
| Convective heat flux | Sutton–Graves `q = k·√(ρ/R_n)·v³` | peak flux | < 10 % vs published empirical fit | Stated as an *engineering correlation*, not physics |
| Radiative heating onset | Published onset velocity | onset | within band, flagged |
| Structural: pressure vessel | Thin-wall hoop/longitudinal stress (closed form) | stress | < 0.1 % ⚑ | The closed form itself assumes thin walls; we flag the validity range |
| Structural: buckling | Published cylinder buckling curves | critical load | within published scatter band ⚑ | Knockdown factors are empirical; we display the factor we used |
| Rotating habitat hoop stress | σ = pr/t + ρω²r² | stress | < 0.5 % | |
| Coriolis cross-coupled stimulus | Published vestibular thresholds (2–3 rpm comfort limit) | threshold | reproduces the published limit | The whole point is that the player feels it |
| Radiation dose | Published GCR/SPE models (e.g. OLTARIS-class, CREME96 lineage) | dose rate, shielding curve | < 20 % ⚑ | Radiation transport is the least certain model in the product. Say so loudly |
| Terramechanics | Bekker–Wong with published soil parameters | sinkage, drawbar pull | < 25 % ⚑ | Empirically parameterised; the number is a model, not a measurement |

### 3.4 Determinism (a validation category in its own right)

| Case | Method | Requirement |
| --- | --- | --- |
| Bit-identical native vs WASM | Golden hash of 1,000 mission replays | identical, all 1,000 |
| Browser matrix | Chromium, Firefox, WebKit × {x86-64, arm64} × {native, wasm} | identical hashes |
| No host libm | `sin/cos/exp/log/atan2/pow` from `odderon-dmath` only; lint forbids `f64::sin` in the kernel | compile-time failure |
| No wall clock in kernel | Lint forbids `Instant::now`, `Date::now` in kernel crates | compile-time failure |
| No hash-order dependence | Lint forbids `HashMap` iteration in kernel; use `BTreeMap`/indexed vecs | compile-time failure |
| Replay after a 10,000-event session | Re-simulate from input log | identical state hash at every checkpoint |
| Replay across epochs | Universe epoch recorded in the bundle | mismatch is *detected and refused*, never silently accepted |

This is not paranoia. A single call to the platform's `sin()` can differ in the last ulp between
Firefox and Chrome, which changes a chaotic trajectory's phase, which breaks every downstream hash.
Owning our transcendentals is the price of an auditable replay, and it is a price worth paying once.

---

## 4. The export bridge

The single highest-value credibility feature, because it is the *observable* proof that we are
serious. Five formats, in priority order.

### 4.1 GMAT script (`.script`) — *first, because it is free to verify*

Generate a complete, runnable GMAT mission from the player's design and trajectory: spacecraft,
force model, propagator settings, burns, targets, and the report file that echoes back the final
state.

```
Create Spacecraft Odderon_S1;
GMAT Odderon_S1.DateFormat = UTCGregorian;
GMAT Odderon_S1.Epoch = '14 Nov 2028 03:22:41.000';
GMAT Odderon_S1.DryMass = 412.6;
...
Create ForceModel Odderon_ForceModel;
GMAT Odderon_ForceModel.CentralBody = Earth;
GMAT Odderon_ForceModel.PrimaryBodies = {Earth};
GMAT Odderon_ForceModel.GravityField.Earth.Degree = 4;
GMAT Odderon_ForceModel.GravityField.Earth.Order  = 4;
...
Create Propagator Odderon_Prop;
GMAT Odderon_Prop.Type = RungeKutta89;
...
BeginMissionSequence;
  Propagate Odderon_Prop(Odderon_S1) {Odderon_S1.ElapsedSecs = 540.0};
  Maneuver(Odderon_Burn1);
...
```

*Acceptance test:* a CI job runs **real GMAT in batch mode** against our generated script and asserts
the final orbital elements agree with our own kernel to within 1e-6 relative over 24 hours. That job
is the proof. If we cannot run GMAT in CI, we ship the generated files with a public "here is how to
verify this yourself" page.

*Why it matters:* a professor can take a student's Odderon mission, run it in the tool they teach
with, and see the same answer. That single moment converts the game into coursework.

### 4.2 NASA CEA input deck (`.inp`)

Every engine designed in the game exports its propellant, mixture ratio, chamber pressure, and
geometry as a CEA input deck, and the game *reads back* CEA's output. This makes the propulsion
solver's accuracy a matter of public record rather than a claim.

*Acceptance:* the 30-case suite in §3.2, run in CI, both directions, against a real CEA build.

### 4.3 SPICE kernels (`.bsp` / SPK)

- Export the player's trajectory as an **SPK** kernel (type 9 Lagrange or type 13 Hermite, both
  well-documented and widely supported) plus a text MJD/TDB time span.
- Export body orientation for habitable megastructures as a **PCK**-style frame kernel.
- *In the other direction:* we consume real NAIF SPK/PCK for the solar system.

*Acceptance:* our generated SPK loads in `spiceypy` and in SPICE Toolkit's `spkcov`/`brief` without
warnings, and positions match our kernel to 1e-6.

*Why it matters:* orbits in SPICE are how the real world shares orbits. A student who has exported
one has used the actual currency of interplanetary navigation.

### 4.4 CCSDS packets (for hardware-in-the-loop)

Telemetry framed as CCSDS Space Packets (primary header, APID, sequence count, secondary header
time), so a real flight computer or a CubeSat team's ground station software can consume it. See §6.

### 4.5 Geometry and drawings

Honest here: **STEP export is a research project, not a feature.** What we will ship:

- **SVG/DXF** parametric drawings generated from the same parametric model as the 3-D view (true
  orthographic projections, dimensions derived from parameters, not hand-placed). This is what a
  report actually needs.
- **glTF** export for the geometry, with a clear note that it is a surface model, not a solid, and
  not suitable for manufacturing.
- No claim of CAD interoperability beyond DXF profiles.

### 4.6 The open schema

A documented, versioned JSON schema for designs, missions, and results. Everything the game knows is
exportable as text. This matters more than any single format: it means a third party can build a
tool we never imagined, and it means our own data cannot be held hostage by our own binary.

**Design rule:** *if a number cannot leave the game in a documented text format, it is not a real
number.*

---

## 5. Proof-of-Flight — making Esse verifiable

### 5.1 The claim

> An Odderon rating is not a number we assert. It is a function of a replay that anyone can run.

### 5.2 The bundle

A `<100 KB` file (plus optional catalog hash reference), containing:

```jsonc
{
  "schema": "odderon.proof-of-flight/1",
  "epoch": { "catalog": "gaia-dr3+horizons-2026.44", "hash": "blake3:…", "t0_tdb": "…" },
  "kernel": { "version": "0.4.1", "hash": "blake3:…", "dmath": "0.1.0" },
  "design": { /* the input parameters, not the mesh */ "hash": "blake3:…" },
  "prediction": { /* the committed card */ "commit": "blake3:…", "committed_at": "…" },
  "inputs": [ { "t_us": 0, "act": ["Valve", 17, 1.0] }, … ],   // actuator stream
  "seeds": { "anomaly": 4471, "material_scatter": 8892 },
  "checkpoints": [ { "t_us": 60000000, "state_hash": "blake3:…" }, … ],
  "result": { "outcome": "orbit-achieved", "perigee_m": 187412.0, "final_hash": "blake3:…" },
  "signature": "ed25519:…"          // optional; authored by the player's key
}
```

That is the entire save file, the entire replay, and the entire credential. **Kilobytes.**

### 5.3 The verifier

Two implementations, because one is not credible:
1. A **WASM verifier** that runs in the browser with the network *off*, so a recruiter needs nothing
   installed and nothing trusted.
2. A **CLI verifier** published as source, pinned to a kernel version, so a verification can be
   reproduced in a decade by someone who does not trust our web page.

Both compare checkpoint hashes and refuse on mismatch, reporting the first divergent tick.

### 5.4 What this solves, and what it does not

| Solved | Not solved |
| --- | --- |
| Retro-fitting a prediction after the fact | Whether the *input stream itself* was generated by a human playing or by a clever automation |
| Editing a result in the bundle | Whether the player copied someone else's input stream (attribution) |
| Divergence between "my" browser and yours | Whether the physical model is good enough (that is §3's job) |
| Silent universe drift breaking a score | Whether a *team* shares one rating (policy, not code) |

The unsolved ones are honestly unsolved by any offline scheme. The mitigations: input streams carry
human-timing statistics (a signature of manual play, disclosed when used); attribution is handled by
opt-in keys and publishing; and the interesting part of Esse (calibration) is inherently
self-scoring, so stealing an input stream gains less than it costs.

**Deliberate policy:** we would rather have 90% verification and zero central authority than 100%
verification that requires us to be trusted.

---

## 6. Hardware-in-the-loop (HIL)

The feature that makes university rocket and CubeSat teams care *years* before anyone calls this a
game.

### 6.1 The interface

```
Transport:   localhost WebSocket (JSON or CCSDS-framed binary)  |  WebSerial (Chrome/Edge, USB CDC)
Direction A: Odderon → hardware  · telemetry: IMU, accel, baro, thermocouples, star-tracker quat,
                                   GPS-like state, separation flags, discrete events
Direction B: hardware → Odderon  · actuators: valve PWM, gimbal commands, ignition discrete,
                                   throttle, pyro channels
Rate:        configurable 10–1000 Hz; the sim can also run in *lockstep* (real-time factor 0)
Latency:     measured and *reported in the flight log*; the physics never pretends it was instant
```

Design commitment: **a non-real-time mode ("lockstep") is the default for HIL.** Lockstep means the
sim waits for the controller each tick, so a student's 50 ms Python loop does not silently corrupt the
dynamics. Real-time mode is available, labelled, and the latency is recorded in the artifact.

### 6.2 The use cases this unlocks

| User | What they get | Why it's a wedge |
| --- | --- | --- |
| University sounding-rocket team (IREC / Spaceport America Cup) | Test their real flight software, state machine, and Kalman filter against a physics model before a launch they only get once a year | Their alternative is expensive; a failure costs an airframe and a season |
| CubeSat team | Run their ADCS in the loop with realistic sensors and reaction-wheel saturation | They already have the hardware and the software; they lack the environment |
| Amateur high-power rocketry | Validate dual-deploy logic and staging timers without a reload | Large, active community |
| GNC engineer / hobbyist | Fly *actual* control code against a real dynamics model | This is the "this game is a tool" moment |
| Student | Learn what sensor noise and latency actually do to a controller | Nobody understands this from a lecture |

### 6.3 Safety and honesty boundaries

- The HIL path drives **simulation outputs only**. There is no code path in Odderon that can command
  real energetic hardware, and there never will be. The USB/serial boundary is an input device.
- The logs record the transport, the rate, and the measured jitter, so a report cannot claim a
  fidelity the transport did not have.
- A prominent notice: *do not use Odderon HIL results as the sole validation for a flight-critical
  system.*

---

## 7. Akashi — the report as the deliverable

### 7.1 Structure (mapped to what a reviewer actually reads)

A generated report follows a conventional technical-report skeleton, so a professional can skim it in
90 seconds and find the four things they check first.

| Section | Content | Generated from |
| --- | --- | --- |
| 1. Summary | vehicle, mission, epoch, outcome, key numbers | bundle |
| 2. Requirements & constraints | the customer's constraint, verbatim | contract |
| 3. Design | parametric dimensions, materials (with property provenance), mass budget with margins | design hash |
| 4. Propulsion | cycle schematic (P&ID), chamber conditions, nozzle, cooling, CEA comparison | thermo kernel |
| 5. Trajectory | elements, porkchop, Δv budget, losses breakdown (gravity, drag, steering) | astro kernel |
| 6. Systems | power, thermal, comms (link budget with real light-time), GNC, structure | subsystem models |
| 7. Predictions vs actual | the committed card, overlaid curves, residuals | prediction + flight |
| 8. Anomalies & RCA | what diverged, the attributed cause, *and the player's own written analysis* | kernel + player |
| 9. Margins & risk | mass/thermal/power margins; sensitivity table (Eidos derivatives) | Eidos |
| 10. Verification | replay hash, verification instructions, export formats | bundle |
| 11. References | the models used, cited | kernel registry |

The **margins table and the sensitivity table** are what will make an engineer sit up, because they
are the two things a student report almost always lacks and a professional report always has.

### 7.2 The part that cannot be generated

Section 8 requires the player to *write* the root-cause analysis. The game will not generate it for
them. It can check it: the attributed cause is known to the kernel, so the game can say "your
explanation does not match the evidence" without saying what the evidence is.

That is the single most valuable sentence in the entire product, and it is also the hardest to build
well. It is also, essentially, an exam that cannot be cheated and cannot be pre-written.

### 7.3 Export

HTML (primary: self-contained, offline, printable, works in ten years) → PDF (print) → and the
underlying JSON so that a lab can machine-read a cohort of student reports. That last one is how
professors will actually use it: *"export all 60 reports from my class as JSON and show me the
distribution of margin errors."*

---

## 8. How a sceptic verifies us in one afternoon

The most important test in this document is not a physics test. It is: **can a busy, sceptical
engineer falsify our claims in an afternoon, without our help?**

The four-step path must exist, and must be documented on a single page:

1. **Read the models.** One page per model, listing the equations, the sources, and the validity
   range. No hidden constants.
2. **Run the conformance suite** (`cargo test --suite validation`, or a web page that runs it in the
   browser) and read the result card for each case, including the failures.
3. **Download a replay bundle** from any public mission and run the CLI verifier. Confirm the hashes.
4. **Take an exported GMAT script / CEA deck** and run it in their own tool. Compare.

If any of those four steps requires an account, an upload, a login, or a phone call, we have failed
at credibility regardless of our physics. **Everything in the verification path is public,
unauthenticated, and offline-capable.**

---

## 9. Independent audit

Self-validation is not validation. The plan, in order of feasibility:

| Stage | What | Cost | When |
| --- | --- | --- | --- |
| **Internal** | The suite in §3, run in CI on every commit, published as a dashboard | free | from Block 1 |
| **Adversarial** | Recruit 3–5 graduate students to try to break the physics; pay them for *finding failures*, publish what they find | small honorarium | after rung 2 |
| **Academic** | A university lab (astrodynamics or propulsion group) reproduces the suite and publishes a short note | a co-authorship, usually | rung 4 |
| **Professional** | One practising flight dynamics engineer runs a real mission's public trajectory through the tool and reports agreement | a favour, or a small contract | rung 4–5 |
| **Institutional** | An agency's own internal trial, at their discretion and on their timeline | theirs | rung 6, maybe never |

**Publication rule:** the audit report is published *whether or not it is flattering*. Including the
failures. Especially the failures. A validation page with zero failures is not credible; it is
advertising.

Also published, and this is unusual: **a list of things we know we get wrong.** Section 10.

---

## 10. Honest limits — the things we will not be able to do

Written now, so that nobody is surprised later, and so that a knowledgeable reader trusts the rest.

| Limit | Why | What we do instead |
| --- | --- | --- |
| **No 3-D CFD.** Ever, at this target. | Cannot run on a 4 GB integrated-GPU machine in a browser | 1-D/quasi-2-D networks + published correlations, with the correlation *named on screen* |
| **No FEM.** | Same | Closed-form + published knockdown factors, displayed as such |
| **Empirical correlations are not physics.** Bartz, Sutton–Graves, Bekker–Wong, Summerfield are fits to data | Honesty | Each is flagged in the UI as `Modeled { model: "Bartz (1957)", σ }` and cannot be displayed as if measured |
| **Radiation transport is approximate** | Full transport is a Monte Carlo problem | Published models with generous, declared error bars (±20 % target, likely worse) |
| **Real-gas and two-phase flow are partial** | Difficult, and the phase-change physics is genuinely unsettled | Declared ideal-gas regions; boil-off with a stated error bar rather than a false precision |
| **Catalog is incomplete beyond a few kpc** | Gaia is magnitude-limited; dust extinction | Model the extinction honestly; the galaxy is dark because it *is* dark |
| **Exoplanet interiors are unknowable** | We have mass and radius and little else | Bayesian hypothesis ensembles, explicitly labelled; *the player's job is to reduce them* |
| **Time-stepping real-time physics at 60 Hz is not how orbits work** | Events, not frames | Analytical propagation; the integrator only runs in the attention bubble |
| **No certification** | Obviously | The disclaimer in §2, in the product |
| **Our Esse anchors are claims** | Any rating scale is a convention | Publish how the anchors were set, and revise them openly |
| **The "recommended by NASA" outcome may never come** | Institutions move slowly and for their own reasons | Build value at rung 2–3, which is achievable and useful regardless |

---

## 11. Where Odderon sits among the real tools

Including the rows where we lose, because the rows where we lose are the interesting ones.

| | GMAT | NASA CEA | STK | KSP | Children of a Dead Earth | **Odderon** |
| --- | --- | --- | --- | --- | --- | --- |
| Physics depth | High (orbits) | High (chemistry) | High (missions, RF) | Low | High | Medium-high, declared |
| Provenance / uncertainty types | Partial | No | Partial | No | No | **First-class** |
| Learning curve | Weeks | Hours | Weeks | Minutes | Days | **Minutes to first consequence** |
| Zero-UI physical interaction | No | No | No | Partial | No | **Yes** |
| Determinism / auditable replay | Partial | Yes | Partial | No | No | **Yes, by design** |
| Economy / company / career | No | No | No | Partial | Partial | **Yes** |
| Multi-POV, light-time causality | No | No | Partial | No | No | **Yes** |
| Runs in a browser on Intel UHD | No | No | No | No | No | **Yes (the constraint)** |
| Independent V&V programme | Yes, decades | Yes, decades | Yes | No | No | **Planned, §3 — currently rung 0** |
| Certification | No | No (reference) | No | No | No | **Never** |

The last two rows are the whole story. We are not claiming to be more validated than tools with
decades of published validation. We are claiming a different product — and stating exactly where we
stand on the ladder while we build it.

---

## 12. The adoption ladder — realistic, in order

Not "NASA picks it up". The actual path, each step being something we can *do* rather than wait for.

| Step | Audience | Why they say yes | What they need from us | Realistic timeline |
| --- | --- | --- | --- | --- |
| 1 | **Individual aerospace students** | It is more fun than a problem set and produces a portfolio piece | The game to work, and to be free | Block 4–5 |
| 2 | **University rocket teams** (IREC / Spaceport America Cup) | HIL for their real flight software, plus export to CEA/GMAT for their reports | WebSerial/WebSocket HIL, CEA export | Block 6 |
| 3 | **CubeSat / student satellite teams** | ADCS-in-the-loop, light-time comms realism, CCSDS | CCSDS framing, SPICE export | Block 7 |
| 4 | **Course instructors** | A semester of astrodynamics with built-in assessment, JSON cohort export, and cheat-proof prediction cards | Instructor dashboard, class bundles, stable kernel versions | after rung 3 |
| 5 | **Amateur high-power rocketry** | Cheap, safe validation of dual-deploy and staging logic | Simple HIL path, no install | after rung 2 |
| 6 | **Professional training groups** (agency academies, new-hire onboarding) | Judgement drills, blind anomaly resolution with a clock, reproducible scenarios | Scenario authoring tools, evaluation rubrics, procurement-friendly licensing | rung 4–6 |
| 7 | **Public / media / education** | The awe, the story, the fact that a teenager in Kathmandu can use it | Performance on old hardware, low bandwidth | continuous |

Note the shape: **each step is a smaller, more concrete group than the one after it**, and each one
requires something we can build. That is deliberate. The only way to fail this ladder is to try to
start at step 6.

Two specific, checkable targets for the first year, stated so they can be judged:

- **T1:** at least three documented university teams using the HIL path and the CEA export in a real
  competition season, with their permission to be named.
- **T2:** at least one instructor teaching a course with Odderon as a supporting tool, with a
  published syllabus.

If T1 and T2 fail, the credibility thesis is wrong and we should say so and re-plan around being a
very good simulator *game*.

---

## 13. Curriculum and competency mapping

An engineer will ask: *"what does playing this actually teach?"* An answer in prose is worthless.
Here is the map, in terms they recognise — textbook chapters, and the tasks in which those chapters
are applied.

| Odderon activity | Canonical text | Real competency |
| --- | --- | --- |
| Orbital mechanics, burns, rendezvous | Vallado, *Fundamentals of Astrodynamics*; Curtis | Orbit determination, Δv budgeting, targeting |
| Propulsion design & static fire | Sutton & Biblarz, *Rocket Propulsion Elements* | Cycle design, Isp/thrust trade, stability, cooling |
| Systems architecture & margins | Wertz & Larson, *SMAD* | Mass/power/thermal budgets, margin policy, redundancy |
| Cryogenic ops, chilldown, ullage | NASA SP-8000-series practice docs; launch-vehicle operations literature | Pad operations, sequencing, GSE, hazard awareness |
| Entry, descent, landing | Published EDL literature (e.g. MSL/Mars-2020 papers) | Heatshield sizing, guidance, terminal descent |
| Failure analysis | NASA/ESA/NTSB mishap reports | Root cause analysis, corrective action, review discipline |
| Company economics | Space industry cost literature | NPV, capex/opex trade, business case |
| Human factors in spaceflight | NASA-STD-3001 human factors standards | Physiology limits, habitat design, workload |

The point of the map is not that Odderon *replaces* those texts. It is that **every hour of play lands
on a specific competency, and the Akashi entry is the evidence for it.** A student who can show three
Akashi entries mapped to three competencies has something a resume cannot fake.

---

## 14. Pre-mortem: how this fails, and what would prove us wrong

Written before we start, so the failure is not a surprise and not a rationalisation.

| Failure mode | Early warning sign | Kill criterion | Pivot |
| --- | --- | --- | --- |
| **Physics is good, game is not fun** | Players admire the demo and do not return | < 25 % of first-week players start a second mission | Rebuild around the debrief and the clipboard; keep the kernel, change the loop |
| **Game is fun, physics not credible** | Nobody outside the community cites it; no team adopts HIL | No T1/T2 in year one | Narrow the claim to *training*, drop the "credible physics" pillar honestly |
| **Zero-UI fails on mobile** | Mobile session length < 90 s; high early exit | Cannot complete a static fire on a 2017 Android in 3 attempts | Mobile becomes design/review only; say so publicly |
| **Performance fails on the reference hardware** | < 30 fps at 648 p on Reference A | Cannot hold 30 fps | Cut the outdoor walking; interiors + instruments only |
| **Determinism fails** | Any hash mismatch across browsers | Any unexplained mismatch after rung 2 | Replays become "advisory", Esse becomes local-only — a big loss, so this is a top-3 engineering risk |
| **Productive failure drives people away** | Retention cliff at first failure | > 40 % who experience the first failure never return | Add scaffolding, not punishment; this is fixable but must be measured, not assumed |
| **Scope** | No playable build for months | No block shipped in 8 weeks | Ship smaller blocks. A block is a promise; keep them small |
| **Claims outrun evidence** | We say rung 6 words from rung 1 | Any public claim above our current rung | Retract publicly. Credibility is a non-renewable resource |

The last row is the one that matters most. **The single greatest risk to this project is not technical.
It is that the vision is so compelling that we start believing our own marketing before the evidence
exists.**

---

## 15. Legal, ethical, and export considerations

Not legal advice; a list of things that must be checked by someone qualified before launch.

| Topic | Concern | Our position |
| --- | --- | --- |
| **Export control (ITAR/EAR)** | Aerospace performance data and technical data can be controlled. Most published textbook data is not. | Use **only publicly available literature and public agency data**. No controlled data. No real propulsion performance from non-public sources. Consult counsel before shipping anything that models a specific real weapon-deliverable or controlled system. |
| **Real companies** | Trademark and defamation risk; also: a game about a real company's failure invites litigation | **Fictional companies**, inspired by the industry. Real agencies appear only as neutral, public facts (e.g. catalog data). |
| **Real people** | Likeness, voice | No likenesses. No synthetic voices of real people. |
| **Mishap reports** | Public domain in most cases; attribution matters | Cite them. Use them. Do not sensationalise casualties. |
| **HIL to real hardware** | A user could injure themselves or damage hardware | Input-device-only boundary; prominent warnings; never command energetics |
| **"Recommended by X"** | Endorsement cannot be invented | Never state an institutional endorsement we do not have in writing. Ever. |
| **Minors** | The target audience includes teenagers | No chat by default. Async messaging is opt-in, moderated, and never required. No purchases without guardian flow if we ever monetise. |
| **Data** | Player telemetry and reports are personal work | Local-first by default. Nothing uploaded without explicit action. Export is a file, not a submission. |
| **Accessibility law** | Education procurement often requires WCAG | Target WCAG 2.2 AA for all instrument and terminal text, and publish a conformance statement |

---

## 16. The ask — what would make this real

Not a request for belief. A request for a **specific, falsifiable next step**, in the order I would
take them.

1. **Build Block 1 and measure it on the reference hardware.** No credibility without the
   performance number, because "runs on a 2015 laptop" is the constraint that makes the rest
   non-obvious. Deliverable: a benchmarked build, published numbers, including the misses.
2. **Publish the kernel and the conformance suite at rung 1**, with the failures visible. Deliverable:
   a public validation page generated from CI.
3. **Ship the CEA and GMAT export and verify with the real tools.** Deliverable: a CI job running a
   real GMAT build, or a documented manual verification with screenshots and scripts in the repo.
4. **Ship Proof-of-Flight and let a stranger break it.** Deliverable: a public page, a bundle, a
   verifier, and an invitation to attack it.
5. **Get one university team to use HIL for one season.** Deliverable: a named team, a public
   write-up, and their honest criticism.

Steps 1 and 2 are within reach of this repository, this year. Steps 3–5 need other people, which is
why the artifacts have to be good enough that other people will spend their time on them.

---

## 17. The one-page summary, for someone with ninety seconds

> **Odderon** is a browser simulator of real aerospace engineering that runs on hardware most
> engineers' students already own, and produces artifacts that can be audited: a replay anyone can
> re-run, a report in the shape of a technical paper, an export that opens in GMAT and NASA CEA, and a
> trajectory in SPICE format. Its physics models are published with their sources and their known
> errors; its validation suite is public, including what fails; its rating system is based on proper
> scoring of your own predictions, verified by deterministic replay rather than by our word.
>
> It is not a certified tool, and it says so in the product. It is not a replacement for GMAT or CEA;
> it is the thing that makes a student *want* to learn them, and gives them a first artifact that
> looks like the real thing.
>
> The claim we are aiming at, and the only one worth making:
> **a simulation honest enough to be checked, in a world warm enough to want to live in.**
>
> What we are asking for now is not belief. It is the next falsifiable test.

---

## Appendix A — the result card format

Every validation case emits this, so the dashboard is generated, not written:

```jsonc
{
  "case": "cea.ssme.chamber",
  "model": "odderon-thermo 0.4.1 (Gibbs, 12 species)",
  "reference": "NASA CEA (Gordon & McBride 1994), case 4371",
  "metric": "isp_vacuum",
  "target": "rel <= 0.010",
  "result": { "ours": 452.1, "reference": 455.9, "rel": 0.00834 },
  "verdict": "pass",
  "provenance": "Modeled{ model: 'Gibbs equilibrium, ideal gas', sigma: 0.01 }",
  "notes": "Discrepancy attributable to finite species set; documented in model page.",
  "date": "2026-11-21",
  "kernel_hash": "blake3:…"
}
```

Two rules: **the target is written before the test**, and **the notes field is mandatory when the
verdict is pass by a narrow margin**. A suite that only records successes is not a suite.

## Appendix B — public data sources and their licences (to be verified)

| Source | Used for | Licence/terms to check |
| --- | --- | --- |
| Gaia DR3 (ESA) | star catalogue | ESA/DPAC terms; attribution |
| JPL Horizons / NAIF SPICE | ephemerides, frames | public, attribution |
| NASA Exoplanet Archive | exoplanet priors | public, attribution |
| NASA CEA + thermo data | equilibrium chemistry reference | public |
| US Standard Atmosphere 1976 | atmosphere | public |
| NIST REFPROP / Webbook | fluid properties | REFPROP is **licensed** — check before shipping; Webbook data is public |
| Published mishap reports (NASA/ESA/NTSB) | anomaly library | public domain in most jurisdictions; cite |
| 3-D dust extinction maps | galactic visibility | per-publication terms — verify each |

*Nothing ships with data whose terms we have not read. This is a shipping gate, not a footnote.*
