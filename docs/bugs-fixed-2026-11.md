# Bugs fixed during the WP 1.5 end-to-end audit (2026-11)

An end-to-end audit ran after WP 1.5 shipped, checking each requirement against
the code rather than against a green test suite. Every bug below was **passing
the existing suite** when it was found. Each entry names the regression test
that now guards it, so later schema work (Fdr v3, `odd-worker`) cannot silently
reintroduce one.

Line numbers drift; the test names are the stable reference. Run everything
with `./scripts/gate.sh`.

## 1. Ranked-score exploit: contract framing was unverified

**Severity: critical (integrity).** `verify_record` checked the signature, the
engine and the replay, but never checked *what problem was being solved*. A
self-signed record whose `initial_state` already sat on the 550 km target
orbit, coasting with no burns, verified successfully and earned a rating.

The signature was valid because the attacker held their own key — the whole
point of a self-sovereign identity. Nothing tied the flight to the contract it
claimed.

**Fix.** `certify_contract()` in `crates/odd-verify/src/lib.rs` now compares,
bit-exactly, the record's `initial_state` against `contract.initial`, plus
`epoch_s`, `duration_us` and `node_metric`. Legacy (`engine: None`) records
additionally have their baseline thrust, specific impulse, dry mass and
initial mass checked. Failures return the named
`FdrError::ContractMismatch(field)`.

**Guards** — `crates/odd-verify/tests/engine_anti_cheat.rs`:
- `a_ranked_record_cannot_start_somewhere_easier` — the original exploit,
  forged start state, must be rejected.
- `every_contract_framing_condition_is_enforced` — epoch, duration and
  objective metric each rejected independently.

## 2. Applied-engine desync: the lab preview was not the flown engine

**Severity: critical (integrity + correctness).** Two symptoms, one cause.
`configure()` calls `mission_contract()`, which rebuilds the contract's
*baseline* vehicle. So:

1. Clicking **Fit** solved a new engine in the lab but never changed the
   planner's stage — the design had no effect on the flight.
2. The worker payload sent the **live lab spec**, not the committed one. An
   unapplied edit (drag a handle, never press Fit) would fly and be **signed**.

A probe confirmed both: after Fit the main thread still reported 2396.776 N
while the lab reported 30417.87 N; and an unapplied `Pc` edit produced a signed
record carrying 12 MPa / 104412 N that the verifier accepted.

**Fix.** One `appliedEngine` in `web/mission.js` is the only design that flies.
`reapplyEngine(w)` re-commits it after *every* `mission_contract()` reload.
The worker payload and the split-kick payload both read `appliedEngine`;
`mission-worker.js` re-solves `data.engine` for split targeting. Switching
contract clears it. `missionDiagnostics.appliedEngine` exposes it.

**Guards** — `tests/browser/observatory.mjs`:
- fitted thrust / `Isp` / wet mass must equal the lab's vacuum solution
  (`fitting must commit a design`, and the three `assert.equal`s after it);
- an unapplied `#engine-pc` edit plus `#add-node` must leave `appliedEngine`
  null and the thrust at baseline;
- the ranked FDR's engine fields must equal the committed design.

## 3. Apsis targeting silently skipped a revolution

**Severity: correctness.** The turn search discarded any spline segment whose
left endpoint preceded the search window's `after_s`. A periapsis falling
between `after_s` and the next cached keyframe was therefore missed, and the
search returned the *following* revolution — a burn planned roughly one orbit
late.

**Fix.** `crates/odd-sim/src/targeting.rs` clamps the straddling segment's left
endpoint into the window (`path.sample(after_s)`) instead of dropping the
segment.

**Guards** — `crates/odd-sim/tests/targeting.rs`:
- `finds_periapsis_just_after_the_window_start`
- `window_start_exactly_on_a_turn_returns_the_following_turn`
- `apoapsis_search_also_honours_a_straddling_window_start`
- `finds_periapsis_immediately_after_a_burn_end`

## 4. Lens projection detached itself under the pointer

**Severity: usability (infinite loop).** The hover projection renders the
hovered term *inside itself*. Re-entering that node rewrote the projection's
DOM, destroying the element under the pointer, so `mouseover` re-fired
forever and the lens flickered.

**Fix.** Re-entering the active projection no longer rewrites its nodes.

**Guard** — `tests/browser/observatory.mjs`: dispatches `mouseover` twice on
the projection's own term and asserts the node is still connected and is the
same node (`hovering a term inside the projection must not detach it`).

## 5. Close button could not be clicked

**Severity: usability.** Travelling from a hovered term to the Close button
crosses a `mouseleave` boundary. That closed the lens, so the button vanished
mid-click and could never be pressed.

**Fix.** Moving onto the projection or its Close button does not dismiss the
lens.

**Guard** — `tests/browser/observatory.mjs`: hovers a surviving term inside the
projection, hovers `#lens-close`, asserts the lens is still open
(`moving onto Close must not close the lens`) and visible, *then* clicks it.

## 6. Propellant enum order was duplicated with no guard

**Severity: latent integrity.** The ordering `LoxLh2, LoxRp1, LoxLch4, NtoMmh`
exists in four places: the Rust discriminants, `Propellant::all()`, the HTML
`<option value>` list, and the lab's `BANDS`/`OPTIMAL`/`LABELS` maps. A signed
FDR stores the **bare `u8` discriminant**, so reordering the enum would
silently re-label every historical certificate — a hydrolox flight would read
as kerolox.

`odd-report` correctly imports `Propellant` from `odd-propulsion` rather than
redeclaring it, but nothing pinned the numeric ids.

**Fix / guard** — `crates/odd-propulsion/src/thermochem.rs`:
`propellant_wire_ids_are_frozen` pins each discriminant, `from_u8` round trip,
`all()` position and label string, and rejects id 4. **The wire ids are now
part of the frozen schema contract.**

## 7. Stale causal panel after a worker rebuild

**Severity: correctness (display).** The one-shot flight worker rebuilds the
causal panel. Hovering or dragging while it was still running raced a DOM
replacement, reading a panel belonging to the previous flight.

**Fix.** Consumers wait for the worker to settle; engine warnings 7–9 index a
separate `ENGINE_EVIDENCE` table (`warning-7`) rather than overflowing the
7-entry `CAUSAL_EVIDENCE` array.

**Guards** — `tests/browser/observatory.mjs` waits `!missionDiagnostics.pending`
before hovering or measuring lab handles, and asserts `causalValid` (the
combined graph stays acyclic) both before the lab block and after it.

## Also fixed in the same audit

- **Fuel-rich `Isp` optimum pinned to the band floor.** A symmetric `Tc`
  parabola with monotone molar mass put the RP-1/LCH4/MMH argmax at the band
  edge. Fixed with asymmetric rich-side curvature.
  `every_propellant_peaks_isp_inside_its_optimal_band` sweeps 1001 points per
  propellant and requires an **interior** argmax, so a curve merely flattening
  against an edge cannot pass.
- **Vacuum plume stayed cylindrical** instead of opening at the limiting
  Prandtl–Meyer angle (`crates/odd-propulsion/src/plume.rs` unit tests).
- **O/F and `Pc` were not draggable**, and overlapping hit radii let the wrong
  handle steal a drag. Five handles now, nearest wins
  (`tests/browser/observatory.mjs` drag block).
- **Thermal overload reported as a convergence failure.** Added the named
  `PropulsionError::ThermalLimitExceeded`
  (`crates/odd-contracts/tests/thermal.rs`).

## Standing lesson for schema work

Three of these (1, 2, 6) were **integrity** bugs that a green suite hid, and
all three lived at a *boundary*: signature-vs-problem, preview-vs-flown,
Rust-vs-wire. When Fdr v3 adds thermal specs, vent events and a delegation
mask, each new field needs the same three questions:

1. Is it **signed**, and does the verifier **recompute** it rather than trust it?
2. Does the **client commit** path carry the same value the verifier replays?
3. Is its **wire encoding frozen** by a test, not just by convention?

---

# Bugs fixed during the WP 1.6/1.7 acceptance audit (2026-11-14)

A second end-to-end audit ran after the cryogenic and delegation work
shipped. As before, **every bug below was passing the suite when found**. The
full reasoning is in `docs/audit-2026-11-14.md`; this file keeps the
cause/fix/guard record.

## 8. An entire work package was an inert library

**Severity: critical (feature not shipped).** `chief_ai.rs` passed 15 tests
while nothing in production called it: `cap_score`, `apply` and
`hardware_mass` were referenced only from `tests/`, both `Settings`
constructors hardcoded `DelegationMask::MANUAL`, `score()` had no mask
parameter, and there was no UI, badge, mass penalty or cost. The signed mask
was always 0.

**Cause.** The oracle called the library directly, so it proved the
arithmetic and never asked whether a player could reach it.

**Fix.** `vehicle_delegated` charges the margin to the flown dry mass;
`score_delegated` caps Seff; `odd_verify` passes and rebuilds the mask from
the signed byte; the bridge exposes setters, readouts and badges; the
assembly panel renders them.

**Guards.** `crates/odd-verify/tests/delegation_flown.rs` (a *flown* delegated
mission must mass more and score less, and dropping the mask must change the
score) and `tests/browser/assembly.mjs` (clicks the checkboxes, flies the
stage, reads the capped efficiency and the signed mask).

**Lesson.** For every new API, grep for a production caller and add a test
that exercises the shipped path before declaring the package done.

## 9. Active cooling was free

**Severity: critical (integrity).** `Contract::vehicle` charged only the
engine and airframe. A player could fit 400 m² of radiator and sixty blanket
layers, zero the boil-off, and carry none of the mass — the cryogenic
trade-off did not exist.

**Fix.** Fit-out beyond the contract baseline is charged to dry mass, and
`certify_engine` recomputes it from the **signed** thermal spec and
delegation mask. A storable stage has no fit-out and is not charged for one.

**Guard.** `crates/odd-verify/tests/thermal_fitout_anti_cheat.rs` — re-signing
a record that claims 173.8 kg of cooling hardware at the light baseline mass
is refused with `EngineMismatch("dry mass")`.

## 10. The hydrogen tank could never be cooled

**Severity: major (design lever was useless).** The shared bus serves the
oxidizer first. With the baseline 3 kW array the oxidizer's compressor
saturates it, so the fuel tank receives nothing and keeps boiling — and
array output was not a player control, so there was no way to fix it.

**Fix.** Array output is a lever (`mission_thermal_set` field 8) and its mass
is charged like any other hardware. Measured on the flown 6.94-day tug:
155.553 kg → 0.000 kg boil-off for +130 kg of array.

**Guard.** `tests/browser/assembly.mjs` flies both stages and asserts the
loss falls while the dry mass rises.

## 11. The printed QR could not be scanned

**Severity: major.** The dossier QR carried a bare `#verify=…` fragment.
A phone camera cannot resolve a fragment, so the printed "Scan at:" line was
decorative.

**Fix.** The page states its origin through `mission_fdr_origin_reset/byte/commit`;
only `http(s)` URLs with no query or fragment are accepted, bounded at 512
bytes. The QR now carries the absolute page URL plus the fragment.

**Guards.** `crates/odd-view/src/fdr_bridge.rs` tests
(`the_scan_target_is_absolute_when_the_page_states_its_origin`,
`a_non_http_origin_is_refused`) and the observatory gate, which decodes the
printed QR and asserts it is an absolute URL on the serving deployment.

## 12. The propellant ordering existed in four unsynchronised copies

**Severity: major (silent certificate re-labelling).** The Rust wire id, the
`all()` order, the HTML `<option value>` list and the lab's `BANDS`/`LABELS`
maps were four copies of one ordering. A signed record stores the bare
discriminant, so reordering the enum would re-label every historical
certificate with nothing failing.

**Fix.** `mission_propellant_count/label/band/wire_id` expose the table;
`engine-lab.js` builds the selector from it and no longer restates the maps.

**Guard.** `tests/browser/assembly.mjs` asserts selector values equal the
signed wire ids, labels equal the solver labels, and the solver refuses a
mixture ratio just outside each advertised band.

## 13. New bridge tests poisoned the shared statics

**Severity: minor (intermittent gate failure).** The origin tests mutated the
global `ORIGIN`/recorder statics without taking `bridge::test_lock()`, so a
concurrently running dossier test compared HTML that had changed under it,
panicked while holding a lock, and cascaded `PoisonError` into four unrelated
tests. It reproduced only under parallel scheduling.

**Fix.** Both tests take the established `test_lock()` guard.

**Guard.** Four consecutive parallel `cargo test -p odd-view --lib` runs, then
the full gate.
