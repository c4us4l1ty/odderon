# Onboarding visual storyboard — placement and Lessons 1–7

This is the acceptance storyboard for the shipped, event-driven onboarding. It
uses the production mission controls and physics. No lesson substitutes scripted
outcomes. All distances shown to players are labeled as altitude or center
radius, and every acronym is expanded at first use.

## Global presentation and copy rules

- Initial camera: Earth centered, the complete initial orbit fitted inside the
  planning canvas, north/up unchanged from the mission view. Never move the
  camera while a player is dragging a maneuver handle.
- Highlight one target at a time by scrolling its existing control into view and
  moving keyboard focus without smooth animation. Do not cover that target.
- Pre-action copy is at most 40 words. Four progressive hints are available.
- No unexplained acronyms. “MLI” is rendered as “multi-layer insulation” before
  abbreviation; “GNC” is “guidance, navigation and control”; “Isp” is “specific
  impulse”; “FDR” is “flight data recorder.”
- Success is computed from the same snapshot used by the mission debrief. A
  control click alone is never success.
- Closing records the current threshold as a drop-off only after explicit
  playtest consent. It does not mark the lesson complete.

## Interactive placement diagnostic

The **Find My Starting Point** button opens a two-minute hands-on sequence in the
observatory. The player may instead start Lesson 1 or skip without penalty.

### Check 1 — apsis control

- Frame Contract 101’s full initial orbit and highlight the maneuver canvas.
- Prompt: place one node and raise ideal apoapsis to the 500 km target ring.
- Measure button reads real node count and ideal apoapsis.
- Pass: at least one node and ideal apoapsis altitude >= 500 km.
- Failure recovery: route to Lesson 1; do not reveal the answer first.

### Check 2 — phasing intuition

- Keep the same orbit framing and show a target marker 15 degrees ahead.
- Ask for the first burn direction using two spatially adjacent controls.
- Pass: **Retrograde**, which enters the faster lower phasing orbit.
- Failure recovery: route to Lesson 2, the catch-up/prediction lesson.

### Check 3 — fitted mass and prediction

- Show the Schema-5 mass ledger before/after a 150 kg dry-mass reduction.
- Use a 100 m/s burn and 450 s specific impulse. The rocket-equation change is
  about `150 * (1 - exp(-100 / (450 g0))) = 3.36 kg` less propellant burned.
- Pass: choose “remaining propellant increases by about 3 kg.”
- Failure recovery: route to Lesson 7.
- Passing all checks closes onboarding and unlocks direct ranked-contract entry.
  Completion persistence occurs only when the separate progress checkbox is on.

## Lesson 1 — Your satellite is falling

- Contract/camera: 101, complete 260 km initial orbit and 550 km target ring.
- Highlight: gold maneuver arrow on `#plan-canvas`.
- Causal trigger: atmospheric low-side/periapsis warning if applicable.
- Pass: >=2 nodes, ideal periapsis >=500 km, ideal eccentricity <0.02.
- Recovery hints: opposite apsis response; two pushes; move to the next low
  point; use two prograde pushes near successive low points.

## Lesson 2 — Catch up by falling behind

- Contract/camera: reset Contract 101 and preserve the full-orbit framing.
- Highlight: the tangential maneuver control.
- Prompt: a target is 15 degrees ahead; enter a lower, faster phasing orbit.
- Pass: at least one node, first tangential burn retrograde, and ideal periapsis
  between 120 km inclusive and 240 km exclusive.
- Recovery: explain that lower orbits move faster, lower the opposite side,
  pull the tangential handle backward, then disclose the measured safe band.

## Lesson 3 — Tilting the orbit

- Contract/camera: 102, full orbit with plane-tilt handle visible.
- Highlight: orbit-normal maneuver control.
- Causal trigger: `PlaneUnchangedByProgradeBurn` when the player asks an
  in-plane burn to tilt the plane.
- Pass: flown inclination >=98.50 degrees and each apsis within 40 km of 800 km.
- Recovery: distinguish height from plane, identify orbit-normal control, show
  the purple handle, disclose the measured 366.8 m/s reference.

## Lesson 4 — The shaking engine

- Contract/camera: 103, engine cross-section and plume in frame.
- Highlight: test-stand altitude.
- Causal trigger: separated-flow warning.
- Pass: solved plume attached at altitude >=17 km.
- Recovery: explain ambient pressure, watch separation rather than thrust,
  bracket below 20 km, then identify the 17 km measured boundary.

## Lesson 5 — The long-burn trap

- Contract/camera: 103, Earth and lunar-distance target radius fitted together.
- Highlight: practice-only numerical perigee splitter.
- Causal triggers: long-burn sweep, impact, or propellant deficit.
- Pass: modeled flight survives and apoapsis center radius >=380,000 km.
- Recovery: explain swept angle, preserve total velocity change, disclose the
  practice-only splitter, use three numerical periapsis kicks.

## Lesson 6 — First real contract

- Contract/camera: reload baseline 101; complete orbit in frame.
- Highlight: prediction panel, then debrief.
- Pass: finite score >0 after a real locked prediction and flight.
- Recovery: objective is half the score; prediction and efficiency are each one
  quarter; no lock means no score; compare each debrief row.

## Lesson 7 — The mass budget and Schema-5 prediction

- Contract/camera: 103, stage assembly and live mass ledger in frame before the
  orbit. All four Chief-AI departments begin delegated.
- Ledger: `wet mass = fitted dry mass + initial propellant`; fitted dry mass is
  airframe + solved engine + thermal hardware + power hardware + delegation
  margin. It displays the all-Chief baseline and final prediction error.
- Demonstration: delegated 35% component margins and over-built hardware are
  real dry mass. For a requested velocity change, propellant consumed follows
  `delta_m = m0 * (1 - exp(-delta_v / (Isp * g0)))`; more fitted mass therefore
  consumes more propellant and lengthens fixed-thrust burns.
- Measured action: return Thermal and Propulsion to manual, right-size nozzle,
  blanket, cooler, radiator and array, remove all remaining delegation caps,
  update predicted remaining propellant from fitted—not catalog—mass, lock and
  fly a signed Schema-5 record.
- Pass, all required: schema version 5; signed recorder present; fitted dry mass
  below the all-Chief baseline; absolute remaining-propellant prediction error
  <2.0 kg; objective component >0.65; efficiency component >0.65.
- Recovery: delegation margin is mass; take Thermal/Propulsion manual; remove
  every cap; right-size hardware before lock; use the ledger’s fitted mass.

## Consent-gated playtest panel

Recording defaults off. With it off, no observation object is retained and no
playtest key is written. Opt-in creates a local `PlaytestSessionLog` only.
Exports include lesson, threshold time, retries, hint count, unique causal rule
ids, first phasing direction when available, Schema-5 mass delta, prediction
errors and drop-off step. Export is a user-initiated JSON download. Clear removes
local playtest data. Nothing sends observations over the network.

`cargo run -p odd-app --bin playtest-summary -- session.json [...]` aggregates
exports into a markdown bottleneck table with completion rate, median threshold
time, top causal rule and median Schema-5 prediction-error improvement.
