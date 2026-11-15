# WP 1.3 / 1.4: mission planning, physics lens and debrief

This document records the implemented experimental mission loop and the
numeric conventions behind its acceptance gates. It is not an operational
flight-dynamics product. The three contracts are playable local scenarios and
challenge ratings are fixed catalog labels. The player rating and flight
license now persist in the browser, and a ranked flight carries an Ed25519
signed recorder that a reader re-flies bit for bit; what does not exist is a
server that adjudicates the rating. See
[Flight identity and signed records](#flight-identity-and-signed-records).

## Work-package status

- **WP 1.3 — Visual Physics Lens & causal graph:** implemented in
  `crates/odd-app/src/lens.rs`, `crates/odd-app/src/causal.rs`, the scalar
  bridge in `crates/odd-view/src/mission_bridge.rs`, and `web/mission.js`.
  The browser loads the acronym registry from Rust, renders live handle
  projections, and asks the Rust causal engine for deterministic warning
  chains. The browser gate asserts zero unexplained rendered registry
  acronyms, hovers a forbidden term, checks the live projection, and verifies
  the causal DAG.
- **WP 1.4 — Prediction vs. reality debrief:** implemented in
  `crates/odd-astro/src/finite_burn.rs`, `crates/odd-astro/src/shapley.rs`,
  `crates/odd-app/src/debrief.rs`, the mission bridge and the debrief UI.
  The finite-burn loss oracle passes within 1% of the independent integral
  reference. Exact four-player Shapley contributions reconstruct the
  full-minus-ideal metric error; the browser checks share normalization and
  efficiency residual for Contracts 102 and 103.

These are prototype work-package gates, not flight-operations certification.
Since they were written, `crates/odd-identity` (persisted flight license) and
`crates/odd-report` (signed recorder, deterministic dossier, verification QR)
have shipped, and the browser keeps one ranked certificate per contract.
`crates/odd-propulsion` (WP 1.5) has also shipped: the engine is now a
physical design solved from chamber conditions and nozzle geometry, the
browser exposes it as a draggable CAD and Schlieren lab, and the signed
recorder carries the design rather than a claimed thrust.

`crates/odd-systems` (WP 1.6) has shipped: cryogenic tanks are integrated by
the same adaptive Dormand-Prince stepper as the trajectory, driven by actual
sun/planet vectors and eclipse state, with active cooling drawing from the
electrical bus ahead of electric propulsion. The 12.8 t cislunar tug loses
582 kg (4.55%) over its 6.94-day window with a 20-layer blanket. Three numbers
in that work-package card were physically impossible as written; the
corrections and their derivations are in `docs/thermal.md`.

`odd-contracts::chief_ai` (WP 1.7) has shipped: four deterministic department
heads with a per-discipline `[AUTO-DELEGATED]`/`[MANUAL]` toggle, +35% mass and
+30% cost margins, a visible badge, and an efficiency cap of 0.65.

FDR **schema v3 is frozen** (`docs/fdr-schema-changelog.md`): it signs the
per-stage thermal spec, the delegation bitmask, the exact vent history and the
integrated boil-off magnitude. Schema 1 and 2 records still verify.

A server-authoritative rating (WP 1.9) remains deferred, deliberately: it will
be written against the frozen v3 schema so no certificate is invalidated.

## Apsides, directions and time quantization

The requested T–N–W frame takes T along velocity, W along orbital angular
momentum, and N = W × T. N points inward at the periapsis of a prograde orbit;
it is generally **not radial** away from an apsis, because velocity has a
radial component. An increase in prograde speed raises the *opposite apsis*
most cleanly when the burn is near an apsis; a general burn changes energy and
both osculating apsides. A normal burn changes orbital angular momentum; torque
is r × F, generally not along the line of nodes. Explanations and diagrams must
not claim these approximations hold everywhere.

The compiler uses integer microseconds and a normalized, quantized quaternion;
it reports both the unquantized rocket-equation mass and commanded mass. For
example, the 18,000 kg tug, 18,000 N, 450 s, 3,120 m/s burn has an ideal
2,236.858757254 s duration; rounding that duration to microseconds changes
consumed mass by roughly 1.04 micrograms. Thus exact rocket-equation mass and
<1e-9 kg after executing an integer-microsecond, fixed-thrust command cannot
both be guaranteed. In the mathematical F → infinity limit a quantized burn
also becomes shorter than one microsecond and is no longer representable; the
finite-limit regression uses a large but representable thrust.

Every browser node-time entry path — typed numeric input, clicking the preview
path and dragging a node marker — goes through one `clampNodeTime` helper over
`[1, DURATION - 1]` seconds, and the numeric input's `min`/`max` attributes are
generated from that same range. Previously the pointer paths used a separate
60 s margin, so the same intent produced different epochs depending on how it
was entered. The clamp bounds the *epoch* only: a node one second before the
flight end still fails plan validation because its finite burn overruns the
window, which is a separate check.

`Plan::compile_guided` accepts optional pre-burn reference states. A guided
node freezes its TNW command in inertial axes from that reference, and the
Level-1 preview applies the **same inertial impulse** at the node. This is how
the perigee splitter targets an actual numerically propagated periapsis rather
than the periapsis of an earlier ideal conic. Empty guidance preserves the
original ideal-frame behavior.

`next_radial_turn` searches the half-open window `(after_s, end_s)`. The spline
segment containing `after_s` is part of that window: its left endpoint is
clamped to `after_s` and sampled there, because a turn can fall between the
window start and the next cached keyframe. Discarding the whole straddling
segment would silently return the *following* revolution's apsis — a full
period late — whenever a burn ends just before a periapsis, which is exactly
how `split_tug` calls it (`after = burns.last().end`). A turn lying exactly on
`after_s` is not returned; the search is strictly for the next one.
`crates/odd-sim/tests/targeting.rs` pins both behaviors.

## Impact and entry events

The Dormand–Prince integrator supports scalar state-crossing events in
addition to time events. Crossings are bracketed on accepted steps and
localized by bisection with fresh Runge–Kutta trial steps. The force model
exposes a terminal central-body **impact** event at the reference ellipsoid and
a non-terminal **atmospheric entry** event at 120 km geometric altitude.
Propagation stops at impact; it never continues underground or invents a
late-epoch state. Trial stages just below the surface are kept finite by
clamping drag to the reference surface and treating Earth as fully occulting
the Sun; accepted flight is truncated at the crossing.

Event detection is step-bounded: a crossing that leaves and re-enters within
one accepted step without an endpoint sign change is not seen. Callers must
bound `max_step` for the geometry they need to resolve. The mission model uses
120 s maximum steps.

## Cislunar fixture and the playable Contract 103 budget

For the tug's stated 200 km circular Earth orbit, the ideal, instantaneous,
two-body 3,120 m/s prograde impulse reaches approximately **342,072 km from
Earth's center** (about 335,694 km altitude). Even the lower edge of a
365,000 km *radius* target needs about 3,126.47 m/s under that ideal model.
With finite burns, rotating atmosphere, J2, SRP and Sun/Moon gravity, a single
centered 3,120 m/s burn enters the atmosphere and impacts. A guided two-kick
3,120 m/s design reaches only about 109,286 km apoapsis radius in the full
model. The 3,120 m/s value is therefore retained as the **gravity-loss
regression budget**, not as a claim that it reaches the lunar-distance target.

The playable Contract 103 reference uses **3,240 m/s in three equal kicks**,
with later nodes targeted to full-force numerical periapses. The reproducible
fixture (`crates/odd-contracts/tests/tug.rs`) gives:

- node epochs: 1,600.000000 s, 10,300.419216 s, 31,505.642847 s;
- first apoapsis after the final burn: 472,983.798253 s;
- apoapsis radius: 389,522.542502 km (target 380,000 ± 15,000 km **radius**);
- periapsis altitude: 365.436021 km;
- final mass: 8,638.043279 kg (dry mass 5,200 kg);
- exact finite-burn Shapley share of apoapsis error at the common post-burn
  epoch: 0.993689 (> 0.70), with absolute shares summing to 1.

The catalog convention is explicitly **apoapsis radius**, not altitude. The
browser prediction fields remain altitude fields; the debrief also reports
radius and radius error. A radius crossing alone is not a lunar encounter.

`tools/astro-oracle/perigee_kicks_scipy.py` remains an independent SciPy DOP853
experiment: in a point-Earth, constant-inertial-direction, centered-burn model,
one 3,120 m/s burn gives an osculating apoapsis radius near 41,044 km, versus
about 109,315 km after two equal burns at consecutive predicted periapses.
Both consume the same mass. The 2-kick gain exceeds 40,000 km for those exact
assumptions but does not reach the stated target. The long single burn dives to
about 41 km altitude even in the point-Earth vacuum model; with the rotating
atmosphere it impacts, so late-epoch full-model attribution for that fixture is
undefined and the debrief says so.

## Lunar encounter modeling

`odd-sim::encounter` provides a bounded sphere-of-influence search and a
Level-1 Earth→Moon→Earth patched preview. The SOI is the Laplace approximation
`a (m/M)^(2/5)`; it is a preview boundary, not a physical gravity discontinuity.
The full-force flight always keeps continuous Sun/Moon third-body gravity.
`find_entry` brackets a downward SOI crossing on an accepted numerical path;
`LunarPatch::find` searches the ideal plan at a caller-supplied step and then
propagates a Moon-centered conic after entry, translating it back to Earth for
display. It does not target a B-plane, guarantee capture, or optimize a launch
window.

At the Contract 103 reference epoch (2026-01-01 TDB start), the 3,240 m/s
three-kick trajectory reaches the requested apoapsis radius but **does not enter
the lunar SOI** within the 600,000 s mission. The Moon is on the opposite side
of Earth near apoapsis; the closest approach is hundreds of thousands of
kilometers away. The browser therefore reports no lunar entry for that fixture
rather than fabricating one. The encounter finder and patch are exercised by a
controlled-geometry regression (`crates/odd-contracts/tests/encounter.rs`).

## WP 1.3: Visual Physics Lens and causal DAG

`odd_app::lens` owns the acronym registry and the deterministic projection
quantities. The scalar bridge exposes the registry as UTF-8 bytes
(`mission_acronym_*`) and live node quantities (`mission_lens`):

- tangential handle: kinetic-energy change and before/after osculating apsides;
- orbit-normal handle: torque r × F and plane tilt;
- in-plane handle: eccentricity-vector before/after and rotation, with the
  circular zero-eccentricity direction rejected rather than invented.

Each projection also owns a stable browser art key via `Projection::key`,
exported as `mission_acronym_key_len`/`_byte`. The match is exhaustive, so a new
variant must declare its demonstration; the browser no longer restates the
enum's declaration order in a JavaScript array, where an added variant would
have silently fallen back to the tangential drawing.

`web/mission.js` builds its hover glossary from that Rust registry, wraps
rendered terms in focusable `abbr.term` elements, and drives a demand-scheduled
canvas projection through `governor.js`. Hovering/focusing a term or handle
starts the projection; leaving, blurring, Escape or the close button suspends
it. Leaving *into the lens panel itself* does not suspend it, because the
projection text and the close button live inside that panel — an unconditional
suspend hid the button while the pointer was still travelling toward it, so it
could never be clicked. Re-entering the projection that is already showing is
a no-op: the lens
renders the hovered term inside its own `#lens-projection`, so rewriting that
text on every `mouseover` would destroy the node under the pointer and
re-fire the event forever (and steal focus from a keyboard user). The same
guard keeps `pointermove` from resetting the animation phase on every mouse
sample, which otherwise froze the handle demo while the cursor moved. The
idle gate still observes zero animation callbacks, worker messages and
mission redraws after completion.

`odd_app::causal` maps warning inputs to deterministic symptom → mechanism →
cause → demo chains and validates them with a Kahn topological check. The
bridge (`mission_diagnose`, `mission_causal_*`) stores the chains for the UI.
The browser renders the Rust nodes and adds live numeric evidence. Warning
coverage includes impact, propellant deficit, thrust-to-weight below one,
periapsis below the atmosphere, finite-burn gravity loss above 15%, long burn
arc, and apoapsis shortfall. Each chain has a one-click micro-demonstration.

## WP 1.4: finite-burn loss isolation

`odd_astro::finite_burn` isolates the burn penalty in the plain two-body model
so J2, third bodies, drag and SRP cannot contaminate the integrals. During a
constant-inertial-thrust arc,

```text
Δv_ideal - (|v_end| - |v_start|) = L_steer + L_gravity
L_steer   = ∫ (F/m) (1 - u·v̂) dt
L_gravity = ∫ -(g·v̂) dt
```

`Δv_ideal` is `Isp g0 ln(m0/m1)`, not `mass_rate × duration`. The integrals are
evaluated with five-point Gauss–Legendre quadrature on every adaptive spline
segment inside each burn. The module also reports powered duration and swept
central angle. The identity residual is a numerical-quality gauge.

The swept angle is the exact polar-angle integral

```text
Δθ_sweep = ∫ |r(t) × v(t)| / |r(t)|² dt
```

evaluated at the same Gauss–Legendre nodes as the loss integrals, accumulated
per spline segment and summed as absolute turn. It replaces a sum of endpoint
chord angles. For a *planar* arc the two agree — the angle between successive
radius vectors already is the swept polar angle — but they diverge once thrust
tilts the plane, because the 3D angle between two radius vectors is not the
polar angle swept in the instantaneous orbital plane. Measured on a 1800 s
burn with a dominant orbit-normal component, 15× keyframe decimation costs the
chord sum 1.4e-6 rad while the integral stays at 7.3e-8 rad. Note this is the
arc actually flown, not the change in osculating true anomaly: thrust also
rotates the apsidal line. `crates/odd-astro/tests/sweep_exactness.rs` pins both
the analytic planar limit (<1e-6 rad against the endpoint radius angle under
decimation) and the 3D improvement over the retired formulation.

The independent oracle is `tools/astro-oracle/finite_burn_loss_scipy.py`
(SciPy DOP853, rtol 2e-13, atol 1e-11, max step 5 s), which carries the two loss
integrals as extra ODE states. The checked fixture is
`data/oracles/finite-burn-loss.csv`; the gate is
`crates/odd-astro/tests/finite_burn_loss.rs`.

- `low_twr_tli`: 200 km parking orbit, m0 = 1000 kg, F = 800 N
  (a0 = 0.8 m/s²), Isp = 450 s, ideal Hohmann departure Δv = 3131.345013718 m/s,
  duration = 2803.057528170 s. The fixed-inertial burn sweeps about 190°.
  Reference steering loss = 2708.005995873 m/s, gravity loss =
  3421.860187383 m/s, speed deficit = 6129.866183256 m/s.
- `short_perigee`: the same vehicle, a 60 m/s burn centered on periapsis.
  Reference steering loss = 0.019276053510 m/s. The card's small-burn estimate
  `Δv μ T² / (24 r³)` gives 0.019426452360 m/s, about 0.77% high and inside the
  1% gate.

The cubic card formula is a **short centered-burn estimate**, not the exact
long-arc TLI integral. The gate therefore checks the exact integrals against
the independent ODE reference for both cases and checks the cubic estimate only
in its valid short-centered regime.

## WP 1.4: exact Shapley attribution

`odd_astro::shapley` toggles four physical players independently:

0. finite-burn arc (absent = instantaneous impulses with the same propellant);
1. Earth J2 zonal gravity;
2. Sun/Moon third-body gravity;
3. atmospheric drag + solar radiation pressure.

The empty coalition is the impulsive, unperturbed reference. Exact four-player
Shapley values average each player's marginal contribution over all 16
coalitions. Signed contributions sum to `full - reference`; displayed shares
are absolute contributions normalized to sum to one. This is deliberately not
the older leave-one-out sensitivity recipe in `odd_astro::attribution`.

All coalitions use the same integrator options, force assembly, environment
callback and flight limits. If any coalition impacts before the requested
epoch, every outcome is resampled at the earliest impact and `truncated` is
set; the browser suppresses late-epoch shares for a truncated or impacted
debrief. For Contract 103, the mission outcome is the first apoapsis after the
final burn, but attribution is measured just after the final burn
(last burn end + 1 s). Comparing each model at its own later apoapsis would mix
timing differences into the metric, and the ideal multi-impulse preview can
impact on a later return even when the finite flight survives.

The original card's phrase "4 rapid shadow-propagations (<5 ms total) ... exact
Shapley" is internally inconsistent for four interacting players: exact
Shapley needs 16 coalition evaluations, not four leave-one-out runs. This
implementation keeps the exact decomposition and reports the measured worker
compute time instead of claiming the impossible four-run budget. The older
normalized leave-one-out sensitivities remain available for comparison but are
not labeled as exact additive causes.

## Contracts, scoring and browser loop

`crates/odd-contracts` is the single source of truth for the three cards:

- **101 · LEO Drag Rescue · 800**: 260 km → 550 ± 10 km circular, e < 0.005,
  ≤ 3 h, m0 = 1,200 kg, mprop = 180 kg, F = 2,400 N, Isp = 310 s.
- **102 · Sun-Synchronous J2 Precession · 1250**: 250 × 800 km, i = 96° →
  800 km circular, i ≈ 98.60°, RAAN +4.93 ± 0.15° over five days. The card did
  not specify mass/engine, so it explicitly borrows Contract 101's 1,200 kg,
  180 kg propellant, 2,400 N and 310 s.
- **103 · Cislunar TLI: The Perigee-Kick Trap · 1650**: 200 km LEO tug,
  m0 = 18,000 kg, mprop = 12,800 kg, F = 18,000 N, Isp = 450 s, TWR ≈ 0.10,
  apoapsis radius 380,000 ± 15,000 km via two or three perigee kicks. The
  demonstrated reference budget is 3,240 m/s in three kicks; the 3,120 m/s
  oracle is a separate gravity-loss regression.

The local score is transparent: 50% objective quality, 25% locked-prediction
closeness, 25% delta-v efficiency against the contract's reference geometry.
Hard impact, fuel or deadline failures score zero. Attribution shares never
enter the score. The debrief updates a local practice Astro-Elo with the
standard expected-score formula (Kp = 40 for the first 20 local missions, then
20; Km = 8). The rating is persisted locally and every ranked update is backed
by a signed, replay-verified recorder, but no server adjudicates it: it is a
local ladder, not a global one.

The mission page includes a ranked selector, canvas node and handle
interaction, keyboard-accessible numerical controls, the Rust-backed Physics
Lens and causal checks, a perigee-kick splitter, locked predictions,
worker-based flight, impact-aware debriefs, a patched lunar preview legend,
prediction-vs-reality table, exact Shapley list, finite-burn loss line, practice
Elo update and a print-styled PDR export button. That button is the quick
print-to-PDF summary. The full dossier is a separate export owned by
`crates/odd-report`: it is issued only after a successful verify-and-replay and
carries the canonical digest, the metric tables and the vector verification
QR. Boil-off is explicitly marked not modeled.

Heavy propagation, targeting and counterfactuals run in one-shot workers; the
idle gate remains zero animation callbacks and zero worker messages over two
seconds after completion. The browser E2E (`tests/browser/observatory.mjs`)
exercises all three contracts, including Contract 103's single-burn impact,
numerical three-kick split, surviving apoapsis, finite-burn-dominated exact
Shapley attribution, acronym snapshot/hover gate and post-mission idle gate.

## Scalar bridge report layout

`mission_report(i)` after `mission_execute` (length `mission_report_len()` = 58):

- 0 final mass kg; 1 apoapsis altitude m; 2 periapsis altitude m;
  3 eccentricity; 4 local mission score; 5 commanded delta-v m/s.
- 6–9 exact Shapley absolute shares (finite, J2, third body, drag+SRP), only
  when not truncated and not impacted; 10–13 signed Shapley contributions;
  14 total metric error; 15 reference metric; 16 full metric; 17 efficiency
  residual; 18 common attribution epoch s; 19 truncated flag.
- 20 isolated ideal delta-v m/s; 21 steering loss m/s; 22 gravity loss m/s;
  23 speed deficit m/s; 24 identity residual m/s; 25 powered duration s.
- 26 inclination rad; 27 ascending node rad; 28 node change rad (node metric);
  29 evaluation time s; 30 impact flag; 31 atmospheric entry time s;
  32 numerical lunar SOI entry time s; 33 SOI radius m; 34 prediction subscore;
  35 objective subscore; 36 efficiency subscore; 37 common attribution epoch
  duplicate; 38 truncated duplicate; 39 reserved; 40 Contract 103 apoapsis
  radius error m.
- 41–56 the 16 coalition metrics; 57 isolated finite-burn swept angle rad.

Undefined values are NaN, not zero. Infinities are refused by name at signing
time: the JSON transport prints every non-finite value as null and reads null
back as NaN, so an infinity could not survive its own round trip. The worker also
returns `computeMs`
(measured around `mission_execute`) outside the deterministic physics report.

## Acceptance gate map

- `oracle_tnw_maneuver_compilation`: `crates/odd-sim/tests/finite_limit.rs`
  (0.01% element match in a large representable thrust limit; <1e-9 kg
  commanded mass bookkeeping) and `tests/guidance.rs` (shared inertial impulse).
- `oracle_perigee_kick_gravity_loss_reduction`:
  `crates/odd-sim/tests/perigee_kicks.rs` and
  `tools/astro-oracle/perigee_kicks_scipy.py` (>40,000 km gain at the same
  3,120 m/s point-Earth budget).
- WP 1.3 acronym/DAG gate: `tests/browser/observatory.mjs` (zero unexplained
  rendered registry acronyms, live TWR hover projection, valid causal DAG) and
  `crates/odd-app` unit tests.
- WP 1.4 finite-burn gate: `crates/odd-astro/tests/finite_burn_loss.rs`
  (exact loss integrals within 1% of the independent reference; short centered
  cubic estimate within 1%).
- `oracle_counterfactual_attribution`:
  `crates/odd-sim/tests/contract_attribution.rs` (exact J2 Shapley share >85%
  nodal for 102) and `crates/odd-contracts/tests/tug.rs` (exact finite-burn
  Shapley share >70% apoapsis for a surviving 103; shares sum to 1; efficiency
  residual small; impacted 3,120 m/s fixture reports no late shares).
- `oracle_playable_loop_and_idle_gate`: `tests/browser/observatory.mjs`
  (Contract 101 score >0.80, all three loops, 2 s idle counters).
- `oracle_hardening_clamp_and_exact_sweep`:
  `crates/odd-astro/tests/sweep_exactness.rs` (swept angle matches the analytic
  planar arc within 1e-6 rad under keyframe decimation, and beats the retired
  chord sum by >10x once the plane tilts) plus the clamp block in
  `tests/browser/observatory.mjs` (typed and dragged node times share one
  boundary, and the input's min/max are generated from it).

- `oracle_identity_roundtrip_and_rfc8032`:
  `crates/odd-identity/src/lib.rs` unit tests (2048 words, sorted, lowercase
  ASCII, 4–10 characters, unique four-letter prefixes; entropy ↔ 12 words ↔
  `ODD-` key lossless; a substituted word fails the 4-bit checksum),
  `crates/odd-identity/tests/rfc8032.rs` (the three published RFC 8032 §7.1
  vectors reproduce byte for byte, and the derived seed signs identically to a
  plain Ed25519 key), and the identity block in
  `tests/browser/observatory.mjs`, which calls `mission_ed25519_selftest()`
  inside the compiled wasm32 module and requires all three vectors to pass
  there too, then round-trips both encodings and rejects a flipped message bit.
- `oracle_tamper_proof_fdr_verification`:
  `crates/odd-report/tests/tamper_proof.rs` (one ULP in every finite metric,
  one microsecond on every command, and nineteen other field mutations each
  rejected with `SignatureMismatch`; transport round trips; malformed and
  future schemas named) and `crates/odd-view/src/fdr_bridge.rs` tests, which
  fly Contract 101, replay it bit for bit, and then **re-sign** a false metric
  and a shifted command so the signature check passes and
  `PhysicsReplayMismatch` / `FinalStateMismatch` must catch them. The browser
  gate repeats all of it in wasm32, including the re-signed forgery.
- `oracle_odd_report_deterministic_dossier`:
  `crates/odd-report/tests/tamper_proof.rs` and the odd-view export test (two
  runs byte-identical including the QR matrix, all four sections plus the
  annex present, both figures vector, no raster assets), and
  `tests/browser/observatory.mjs`, which rebuilds the QR matrix from the
  emitted SVG rectangles, decodes it with `jsqr`, and verifies the decoded
  payload again.
- `oracle_browser_persistence_and_idle_gate`:
  `tests/browser/observatory.mjs` (the first execution of a contract certifies
  it and moves Astro-Elo; license, public key, mnemonic, rating and all three
  certificates survive a full document reload; the profile dialog exports a
  backup that re-imports to the same identity and rating; a corrupted
  mnemonic is rejected while the real one validates; replaying a certified
  contract stays practice with the rating and certificate unchanged; the
  profile dialog and the verification dialog each settle to 0 animation
  callbacks and 0 worker messages over 2 s; a shared `#verify=` link verifies
  in a fresh document).

## Flight identity and signed records

`crates/odd-identity` owns one 128-bit secret with three derived faces: a
12-word mnemonic (128 bits plus a 4-bit SHA-256 checksum, packed as 12 × 11-bit
indices into the compile-time 2048-word table), an `ODD-` hex flight key carrying
the bare entropy, and an Ed25519 keypair whose seed is
`SHA-512("odderon-flight-license-ed25519-v1" || entropy)[..32]`. The domain
separator keeps a license key from ever being confused with another purpose.
Randomness is supplied by the caller (`crypto.getRandomValues` in the browser),
which is why the crate needs no `getrandom` and builds for wasm32 without a shim.
The public identifier is `ODD-ID-` plus base32 of the first ten public-key bytes.

The license, callsign, player rating, per-contract ratings and certified
certificates persist in one versioned `localStorage` record, which also archives
up to eight identities so switching away and back resumes a pilot's own progress.
Without that archive a switch would erase the certificates of the identity being
left, which would let the same contract be certified twice. **This is a local
practice identity, not key custody**: `localStorage` is readable by any script on
the origin, and the profile dialog and the exported backup both say so. A ranked
certificate is issued once per contract; later attempts are practice and cannot
move the rating. The signed recorder is stored with the certificate, because a
locked rating would otherwise leave the official record unrecoverable except by
flying again, which would be practice; the profile dialog can export and
re-verify it after a reload.

`crates/odd-report` signs a canonical fixed-layout byte string, not JSON. Every
field a reader acts on is inside it, including the callsign and the Elo change.
Floats are raw IEEE-754 bits so a decimal round trip cannot round a metric; all
NaNs collapse to one canonical quiet NaN because undefined fields are common and
NaN payloads are not platform-stable, while signed zero is preserved because the
replay compares bits. Verification is two independent checks: the signature over
bytes rebuilt from the typed fields, then a Level-3 replay through the *same*
`compute` function the live flight used, requiring all 58 fields and the
terminal-state hash to reproduce bit for bit.

## Remaining limitations

This is an experimental prototype. There is no launch-window optimizer,
B-plane targeting, lunar orbit insertion, terrain/landing model, space-weather
atmosphere, thrust steering law, cryogenic boil-off model, or server-backed
multiplayer rating. Astro-Elo and the flight license are local to one browser;
there is no account recovery beyond the exported backup, no revocation, and no
defence against someone who reads the origin's storage. The perigee splitter is a bounded forward search for two or
three equal kicks, not a global optimizer. The lunar patch is a display/preview
aid; continuous third-body gravity remains the execution model. The debrief print button is a plain print-to-PDF export; the signed,
QR-bearing dossier is a separate export that is only issued after a successful
verify-and-replay, either as a self-contained HTML download or printed to PDF
from a sandboxed frame at the document's own `@page` geometry. No server-side PDF
renderer is involved. The dossier QR carries an origin-independent `#verify=`
fragment, so a reader must prepend the deployment address; it is not a scannable
absolute URL. It is encoded at error correction M, falling back deterministically
to L for a maximal twelve-command guided flight whose payload exceeds M's byte
capacity; the level actually used is stated in both the SVG and the printed
verification block. Exact
Shapley is computationally heavier than the original four-run/<5 ms card; the
implementation chooses mathematical exactness and reports measured cost.
