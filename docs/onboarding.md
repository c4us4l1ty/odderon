# First 30 Minutes: Onboarding Design

Status: **design, not implementation.** Nothing here ships until this document is
reviewed. Every lesson below is backed by a measured kernel run in
`crates/odd-verify/tests/onboarding_lessons.rs`; where the kernel refused the
story, the story changed.

The target is the **browser** build (WebGL2 + `web/*.js` + `odd_view.wasm`), so
the harness also flies each lesson under the settings `odd_view::mission_bridge`
really passes, including the tank model the native path omits. Every number in
this document is identical under both paths to within 7.5 mm of apsis, and
lesson 6's score is identical to four decimals (§3, §6.2).

---

## 0. What this document replaces, and why

`UI.md` Part 3 already sketches a first 30 minutes. It is built on four beats:

| `UI.md` beat | What it needs | Shipped? |
| --- | --- | --- |
| Launch a 250 kg CubeSat into 300 km | Surface launch, pad ignition, ascent, staging | **No** |
| TWR on the pad (fuel slider) | Launch-site gravity, pad hold-down, sea-level thrust | **No** |
| Nozzle bell at sea level during ascent | An ascent to be during | **Engine lab: yes.** Ascent: no |
| Gravity turn to close the orbit | Pitch-over handle, atmospheric flight, drag | **No** |
| Prediction contract + Astro-Elo + dossier | Locked prediction, debrief, dossier | **Yes** |

Three of the four beats need systems that are not built: surface launch, ascent,
gravity turn, launch-site physics. The fifth thing they need — a reason to care
about TWR — is also the one thing our contracts cannot currently show, because
**every shipped contract starts already in orbit** (measured:
`every_lesson_contract_starts_already_in_orbit`).

This document proposes an arc that needs **none** of that. It teaches the same
physics with the same kernel, starting from a satellite that is already up there
and falling. Ascent becomes a later lesson (see §7), not a prerequisite for the
first 30 minutes.

---

## 1. The arc, and the one rule behind it

| # | Minutes | Lesson | Contract | Concept earned | Discovery |
| --- | --- | --- | --- | --- | --- |
| 1 | 0–5 | **Your satellite is falling** | 101 | Apoapsis, periapsis, prograde | One burn raises one side only; a second burn is needed |
| 2 | 5–10 | **Guess first** | 101 | Prediction, delta-v | Your guess and the truth are two different curves |
| 3 | 10–15 | **Tilting the orbit** | 102 | Inclination, node, plane change | Height and plane are separate; the turn is cheapest high |
| 4 | 15–22 | **The shaking engine** | 103 engine | Flow separation, ambient pressure, Isp | The same bell is a different engine in air |
| 5 | 22–27 | **The long-burn trap** | 103 | Gravity loss, gravity turn | A slow burn near periapsis throws itself away |
| 6 | 27–30 | **First real contract** | 101 scored | Score, prediction term, dossier | Prediction is 25% of your score |

The rule behind the order: **each lesson spends the vocabulary the previous one
earned.** Lesson 2 does not name "prediction baseline" until the player has
already made a prediction and been wrong in lesson 1. Lesson 5 does not say
"gravity loss" until lesson 4 has shown that `Isp` is a *rate of spending fuel*,
not a magic quality score.

### Design rules every lesson obeys

These come from the standing brief and are treated as acceptance criteria:

1. **Act before reading.** First input within 10 s; no pre-action panel over
   40 words. §2 gives the exact copy and its count, taken by counting the words
   rather than by estimating them: **31, 22, 32, 28, 30, 24**. The longest is
   lesson 3 at 32, which leaves 8 words of headroom and no more.
2. **One concept, one lever, one likely failure, one fix** per lesson.
3. **Same physics, no scripts.** Nothing is staged. Every failure below is what
   the kernel actually returns for the player's own input — the harness proves
   it by flying each tempting attempt through `odd_verify::compute`.
4. **Never script a failure; let the player succeed first try.** The tempting
   attempt in each lesson is the *easiest* thing to do, not a trap. If a player
   guesses right immediately, the lesson still runs — it just has less to say.
5. **Escalating hints, never auto-solve.** §4 defines the four-rung ladder.
6. **Vocabulary earned after the deed**, sourced from the existing registry in
   `crates/odd-app/src/lens.rs` — no second glossary.
7. **Guess before reveal.** Every lesson asks for a number before showing one.

---

## 2. Screen-by-screen storyboard

Notation: **[visible]** what is on screen, **[lever]** what the player can touch,
**[success]** the condition that ends the lesson, **[fail]** the likely first
failure, **[copy]** the on-screen text with its word count.

### Lesson 1 — Your satellite is falling (0–5 min)

This is flight 1. The player has touched nothing yet.

- **[visible]** The painted Earth, half in shadow. A gold dot with a short gold
  trail. A dashed cyan oval showing the *contract* target (a 550 km circle). Two
  amber ticks on the current orbit, labelled `AP` and `PE`.
- **[copy]** Pre-action, **31 words**:
  > *This satellite is safe but in the wrong place. It needs to be higher, and
  > rounder. Drag the gold arrow to push it. Press LOCK when you think it is
  > right.*
- **[lever]** One arrow at the current position, draggable along the direction
  of travel only (**prograde, and only prograde**, in this lesson). Drag distance
  maps to delta-v; the arrow shows the numbers as it moves. `[LOCK]` runs the
  flight.
- **[success]** Periapsis above 500 km **and** eccentricity below 0.02: "the low
  side is off the air".
- **[fail — measured]** The player drags to the end of the arrow, spending the
  whole **495 m/s** budget on one burn. Measured result: apoapsis **2 246.9 km**,
  periapsis **273.0 km**. The satellite ends up *much* higher on one side and
  exactly as low on the other. It looks like a spectacular failure and is in
  fact no progress at all.
- **[hint ladder]** see §4, L1.
- **[vocabulary earned]** `AP` and `PE` become hoverable (registered as
  `Projection::Tangential`); the word **prograde** is defined in place, with no
  acronym. **Not yet:** "Hohmann", "transfer orbit", "delta-v".

**Measured facts this lesson stands on**

| Item | Value | Source |
| --- | --- | --- |
| Start orbit | 259.2 × 260.4 km, e 0.00009 | `every_lesson_contract_starts_already_in_orbit` |
| Budget | 495.1 m/s (`ve·ln(m0/mdry)`, Isp 310.6 s) | `budget_m_s` |
| Tempting attempt | 495 m/s → 2 246.9 × 273.0 km, e 0.1292 | `lesson_1_the_tempting_max_burn…` |
| Intended fix | 84.2 + 81.5 m/s → 557.4 × 548.2 km, e 0.0007 | `lesson_1_the_two_burn_fix…` |
| Fix's cost | 165.7 m/s = 33% of budget | `lesson_1_the_fix_stays_inside…` |
| Over-budget attempt | 2 000 m/s → `Err(BurnOutsidePlan)`, refused at compile | `lesson_1_an_over_budget_attempt…` |

The over-budget row is a design constraint, not a detail: **the game must never
present a lever that the kernel will refuse.** The prograde arrow's range is
clamped to the remaining budget, and the refusal is measured here so the clamp
has a number behind it.

### Lesson 2 — Guess first (5–10 min)

- **[visible]** The same orbit, the same arrow, plus a **dashed gold marker** on
  the orbit where the player predicts apoapsis will land. The previous flight's
  trace is still on screen in faint grey.
- **[copy]** Pre-action, **22 words**:
  > *Before you push: where will the far side end up? Drag the gold marker, then
  > make the same burn as last time.*
- **[lever]** The prediction marker (draggable around the orbit), plus the same
  prograde arrow. Both are required before `[LOCK]` enables.
- **[success]** The flight completes and the debrief opens — the player does not
  have to be *right*, only *committed*.
- **[fail — measured]** A plausible naive guess ("three times the burn, most of
  the way there") lands **675 km** off the flown apoapsis (guessed 1 571 km,
  flown 2 247 km). The gap is large enough to read at a glance and small enough
  that a player who understood lesson 1 can beat it.
- **[vocabulary earned]** **delta-v** (the number the arrow shows) and
  **prediction**. The Level-1 vs Level-3 overlay is introduced here: Level 1
  (dashed cyan) is the *contract's* intent, Level 3 (gold) is *what you drew*.
- **[gate]** This lesson is where the 40-word rule bites hardest. The copy above
  is 22 words; the debrief afterwards may be long, because the player has
  already acted.

### Lesson 3 — Tilting the orbit (10–15 min)

- **[visible]** Contract 102's orbit, 250 × 800 km, drawn with a **bright rim**
  on the sunlit side and a **tilted band** across the globe showing the orbit
  plane. The band is visibly not passing over the pole it needs to.
- **[copy]** Pre-action, **32 words**:
  > *This one has to cross the same ground at the same time every day. Its plane
  > is 2.6° short. The arrow has three directions now — one of them turns the
  > plane.*
- **[lever]** The arrow gains a **second, out-of-plane handle** (TNW component 2,
  the orbit normal). Prograde stays. Because lesson 1 taught that height and
  plane are separate, the player can now see two orthogonal levers.
- **[success]** Inclination ≥ 98.50° and apsides within 40 km of 800 km.
- **[fail — measured, and the most important correction in this document]**
  The obvious reading of "tilt the orbit" is to push *sideways in the plane*.
  Measured, a 100 m/s burn along the in-plane normal moves inclination by
  **+0.0089°** — nothing. The same 100 m/s along the orbit normal moves it by
  **+0.5631°**. The two levers are adjacent on screen and differ by a factor of
  63, which is exactly the thing to teach, and exactly the thing a careless
  tutorial would get wrong by drawing the wrong arrow.
- **[second fail]** A prograde-only burn of the reference's own 142 m/s fixes the
  *height* and leaves the plane at **96.008°**, unchanged to three decimals.
- **[vocabulary earned]** **inclination**, **node**, **plane change**. `RAAN`
  becomes hoverable (`Projection::AscendingNode`) *only* after the player has
  seen the node marker go from 3.97° to 4.82° on a tilted burn.

**Measured facts**

| Item | Value | Source |
| --- | --- | --- |
| Start orbit | 238.6 × 767.4 km, e 0.03842, inc 96.00° | `every_lesson_contract_starts…` |
| In-plane 100 m/s | inclination 96.0089° (+0.0089) | `lesson_3_the_orbit_normal_component…` |
| Orbit-normal 100 m/s | inclination 96.5631° (+0.5631) | same |
| Prograde-only reference | 96.008°, 784 × 751 km | `lesson_3_a_prograde_burn_cannot…` |
| Reference burn | 366.8 m/s, 98.595°, 804 × 792 km, e 0.0009 | same |
| Reference vs card | 366.8 of the card's 366.8 m/s; 128 m/s spare | `lesson_3_the_reference_burn_fits…` |
| Plane-change trade | 359 m/s at 250 km vs **331 m/s at 800 km** | `lesson_3_the_plane_change_is…` |

**Constraint discovered:** a *single* plane change at the 250 km periapsis cannot
reach 98.60° inside the budget at all — the harness measured 430.6 m/s buying
98.371° and 538 m/s being refused. So lesson 3's success condition is a
**measurable tilt**, not contract completion. The player is not asked to finish
contract 102 here; that is lesson 6's and later lessons' business.

### Lesson 4 — The shaking engine (15–22 min)

- **[visible]** A cutaway of the tug's bell, drawn as a Summerfield contour, with
  a bright detachment line and a side-load arrow. An altitude slider on the left
  from 0 to 40 km, currently at **0 km**.
- **[copy]** Pre-action, **28 words**:
  > *This is the engine that pushes the tug. It was shaped for space. Drag the
  > altitude slider up and watch the exhaust. Tell me where it stops shaking.*
- **[lever]** One altitude slider. No burn, no orbit, no contract — the engine
  lab is a ground-side solver and needs no flight at all.
- **[success]** The player parks the slider above the altitude where the flow
  reattaches: **17 km**.
- **[fail — measured]** The player runs it at sea level as it opens. Measured:
  the flow detaches at **x/r = 0.146** (area ratio 11.3), the side load is
  **0.09 kN**, thrust falls from **18 087 N to 13 194 N** (−27%) and Isp from
  **449.9 s to 328.2 s**. The "shaking" is a number, not an animation.
- **[the findable threshold]** Measured sweep, 0→40 km: separated at 0, 5, 10,
  15 km; attached from 17 km up. There is a clean, findable transition, which is
  what makes this a lesson rather than a slider to wiggle.
- **[vocabulary earned]** **ambient pressure**, **flow separation**, and `Isp`
  (already registered: `Projection::SpecificImpulse`). **Not yet:** "expansion
  ratio" — the label reads "bell width" until lesson 5 has shown why width is a
  trade rather than a virtue.

### Lesson 5 — The long-burn trap (22–27 min)

- **[visible]** The 18 t tug at 200 km, and — the whole lesson — a **burn-arc
  preview** that draws where the vehicle will be while the engine is still lit.
  At this thrust-to-weight the arc is enormous.
- **[copy]** Pre-action, **30 words**:
  > *This tug is heavy and its engine is patient. One burn is the obvious plan.
  > Draw it, lock it, and watch where the ship is when the engine finally stops.*
- **[lever]** The burn arrow plus a **splitter**: a draggable count of how many
  burns to divide the same total into, from 1 to 3 (the shipped
  `split_tug_for_stage` accepts 2 or 3; 1 is the tempting case). The total
  delta-v is *fixed* by the player's own number, so more burns cannot be "more
  fuel" — only differently timed.
- **[success]** Apoapsis radius ≥ 380 000 km from Earth's centre (contract 103's
  own objective) without re-entry.
- **[fail — measured, and better than expected]** One burn of **3 240 m/s** is
  *legal* — the stage has the propellant — and it is a catastrophe: periapsis
  **−6 365 km**, impacted, apoapsis 2 km. **84%** of the commanded delta-v
  (2 082 m/s gravity + 625 m/s steering = 2 707 of 3 240) is thrown away while
  the engine swings through **157°**. The player gets a crater and a full causal
  chain, not a dialog box.
- **[the fix — measured]** The shipped splitter with **3 kicks** at real
  periapses (1 600 s, 10 303 s, 31 530 s) reaches **387 653 km** altitude with
  only 427 m/s gravity loss. With **2 kicks** it reaches only **158 199 km**.
  The difference is discoverable by dragging the counter.
- **[vocabulary earned]** **gravity loss** (registered:
  `Projection::GravityLoss`), **thrust-to-weight** in the *orbital* regime (a
  second visit to `TWR`, whose flat "cannot lift the vehicle" wording needs a
  variant for "cannot finish before the orbit turns"), and **periapsis kick**.

**Measured facts**

| Item | Value | Source |
| --- | --- | --- |
| TWR at ignition | 0.10, rising as it burns | `lesson_5_measures_the_finite_burn_penalty…` |
| Propellant / engine time for 3 240 m/s | 9 363 kg / **2 284 s** | same |
| One burn 3 240 m/s | −6 365 km periapsis, impacted, 2 707 m/s lost, 157° sweep | `lesson_5_one_burn_is_flown_and_re_enters` |
| Same burn at t=200 s | `Err(BurnOutsidePlan)` — refused | same |
| 2 kicks | 158 199 km altitude, 923 + 185 m/s lost | `lesson_5_the_shipped_splitter_reaches…` |
| 3 kicks | **387 653 km** altitude, 427 + 86 m/s lost | same |

Two warnings for implementation, both measured:

1. **The naive duration formula is wrong by 2.9×.** Delta-v over acceleration
   gives 790 s; the rocket equation gives 2 284 s, because the vehicle lightens
   as it burns. The burn-arc preview must use the rocket equation or the arc it
   draws will be a third of the real arc.
2. **The splitter's guidance is not optional.** `split_tug_for_stage` returns a
   guidance state per node and `Plan::compile_guided` must receive it. Without
   it the same validated 3-kick plan falls into the Earth. This is measured and
   is now a harness comment; it must be a code comment too.

### Lesson 6 — First real contract (27–30 min)

- **[visible]** Contract 101 again, but now with a **numeric prediction panel**
  (five fields), a **score dial**, and the dossier button.
- **[copy]** Pre-action, **24 words**:
  > *Same rescue, real contract. Fill in what you think will happen, lock it, and
  > fly it. The score is not only about the orbit.*
- **[lever]** The five prediction fields and `[LOCK]`. There is no new
  trajectory lever: lesson 6 re-flies lesson 1's fix.
- **[success]** A score above zero, and a debrief that attributes the gap.
- **[measured score]** The lesson-1 fix with a sensible prediction earns
  **0.9238** (objectives 0.8529, prediction 1.0000). That is a real score, from
  the same kernel, in a lesson that took the player four minutes.
- **[fail — measured]** A flight with no locked prediction produces **NaN**, not
  a low score. This is the discontinuity §6 is about.
- **[vocabulary earned]** **score**, **objectives term**, **prediction term**,
  **efficiency term**, and `PDR` (`Projection::DesignReview`) when the dossier
  opens.

---

## 3. Exact initial conditions

All three from `Contract::get`, verified by
`every_lesson_contract_starts_already_in_orbit`:

| Contract | Used by | Start orbit | e | Inclination | Budget | Window |
| --- | --- | --- | --- | --- | --- | --- |
| 101 `CircularRescue` | L1, L2, L6 | 259.2 × 260.4 km | 0.00009 | — | 495.1 m/s | 180 min |
| 102 `SunSynchronous` | L3 | 238.6 × 767.4 km | 0.03842 | 96.00° | 495.1 m/s | 7 200 min |
| 103 `TransLunarApoapsis` | L4, L5 | 199.9 × 200.0 km | 0.00000 | — | 5 478.5 m/s | 10 000 min |

Stages: 101/102 are the same catalog stage (dry 1 020.015 kg, 180 kg propellant,
Isp 310.6 s, 2 397 N). 103 is the 18 000 kg tug (dry 5 200 kg, 12 800 kg
propellant, Isp 449.9 s, 18 087 N, TWR 0.10).

**Starting mass as the game actually reports it.** This is a browser game, and
the game hands the evaluator a tank model that the harness originally omitted
(`odd_view::mission_bridge` always passes `thermal: Some(...)` when the stage
carries propellant). Measured under the game's own settings
(`every_lesson_number_is_the_same_under_the_browsers_own_settings`):

| Contract | Game start mass | No-tank-model mass | Difference |
| --- | --- | --- | --- |
| 101 | 1 200.015 kg | 1 200.015 kg | **0.000 kg** — storable, no tank model |
| 102 | 1 200.015 kg | 1 200.015 kg | **0.000 kg** — storable |
| 103 | 17 994.884 kg | 18 000.017 kg | **5.132 kg** of LH2 boil-off |

The tug really does start 5 kg light, and the design should say so rather than
quote 18 000 kg. Its orbit is unaffected in any way a player can see: the apsides
move by **7.5 mm** (solar radiation pressure divided by a slightly smaller mass),
which is four orders of magnitude below the ~100 km scale every lesson reports.

**Every one starts above 100 km.** This is the fact that makes the whole
first-30-minutes flow possible without ascent physics, and it should be treated
as load-bearing: any future contract added to the intro path must preserve it.

---

## 4. Hint ladders

Four rungs, never auto-solve. Rung *n* only appears after the player has been
idle for 20 s *and* has failed the lesson's success condition at rung *n−1*.
Rung 4 is a ghost the player must execute themselves — the game never flies it.

### L1 — Lesson 1

| Rung | Text (≤ 40 words) | Anchored to |
| --- | --- | --- |
| 1 | *You raised the far tick. Look at the near tick — did it move?* | Causal chain for `PeriapsisBelowAtmosphere` |
| 2 | *(The periapsis tick is highlighted and pulses; the arrow gains a second slot at the far tick.)* | Lever highlight, no text |
| 3 | *You spent 495 m/s and your periapsis is at 273 km. The air ends at about 100 km.* | Governing number: measure vs threshold |
| 4 | *(A ghost second arrow appears at the far tick with the 81.5 m/s value. The player must drag it and press LOCK.)* | `split-kick` demo |

### L2 — Lesson 2

| Rung | Text | Anchored to |
| --- | --- | --- |
| 1 | *Your marker and your orbit can disagree. That is the point of the lesson.* | — |
| 2 | *(The gold marker and the gold trail are drawn in the same colour; the cyan target stays cyan.)* | Lever highlight |
| 3 | *Last time, 495 m/s moved apoapsis from 260 km to 2 247 km. Not three times — eight.* | Governing number |
| 4 | *(The previous flight's trace ghosts in, with its apoapsis where the marker would need to be.)* | Ghost |

### L3 — Lesson 3

| Rung | Text | Anchored to |
| --- | --- | --- |
| 1 | *You changed the height. The plane is a different question.* | `SunSynchronous` objective |
| 2 | *(The out-of-plane handle brightens and the plane band flashes.)* | Lever highlight |
| 3 | *100 m/s sideways in the plane moves the tilt 0.009°. Out of the plane it moves 0.56°.* | Measured numbers |
| 4 | *(A ghost handle pulls along the orbit normal to 98.60°.)* | Ghost |

**New causal rule required.** There is no `Warning` variant for an
inclination/node shortfall. The lesson's rung-1 text asserts a causal claim
("the plane is a different question") that currently has no rule behind it. §8
proposes `PlaneUnchangedByProgradeBurn`.

### L4 — Lesson 4

| Rung | Text | Anchored to |
| --- | --- | --- |
| 1 | *The line on the bell is where the exhaust let go of the wall.* | `FlowSeparationAtAltitude` |
| 2 | *(The altitude slider gains ticks at 15 and 20 km.)* | Lever highlight |
| 3 | *Below about 17 km the flow is detached. Above it, the bell runs full.* | Measured crossing |
| 4 | *(The slider animates to 17 km and stops; the player must release it there.)* | Ghost |

### L5 — Lesson 5

| Rung | Text | Anchored to |
| --- | --- | --- |
| 1 | *Your burn lasted 2 284 seconds. The orbit turned 157° while it was lit.* | `LongFiniteBurnArc` (`burn_sweep_deg > 25`) + `split-kick` |
| 2 | *(The splitter counter brightens; the burn arc redraws as N segments.)* | Lever highlight |
| 3 | *Same 3 240 m/s. Three kicks reach 387 653 km; two reach 158 199 km.* | Measured numbers |
| 4 | *(Three ghost markers appear at the splitter's measured epochs. The player must drag the counter to 3 and press LOCK.)* | Ghost |

### L6 — Lesson 6

| Rung | Text | Anchored to |
| --- | --- | --- |
| 1 | *Half the score is the orbit. A quarter is how well you predicted it.* | Scoring law |
| 2 | *(The prediction panel highlights the two fields with the largest error.)* | Lever highlight |
| 3 | *Your prediction missed by 675 km of apoapsis. That is one fifth of the prediction term.* | Measured gap + scoring law |
| 4 | *(The correct values ghost into the fields; the player must edit and lock.)* | Ghost |

---

## 5. Expert placement test

Goal: a returning player reaches the practice ladder in under 90 seconds without
being taught something they already know, and lands at a *measured* Elo rather
than a guessed one.

**Design.** Three challenges, drawn from the shipped practice path, one per
discipline. No text teaching; the instructions are the one-line objectives.

| # | Challenge | Passes if | Seeds |
| --- | --- | --- | --- |
| 1 | Contract 101, untimed, no hints | score > 0.80 with a locked prediction | trajectory Elo |
| 2 | Contract 102's reference burn, plane change only | inclination within 0.05° of 98.60° | plane-change Elo |
| 3 | Contract 103, single burn | **fails** — the test expects re-entry, and passing means the player found the splitter | gravity-loss Elo |

Challenge 3 is deliberately inverted: an expert who tries one burn and re-enters
has *demonstrated* the lesson's discovery from the wrong side, and the placement
should record "understood loss, chose the fast plan" rather than deduct. This is
consistent with the standing rule that Elo reflects measured outcomes, not
curriculum compliance.

**Scoring.** Placement seeds practice Elo at the *lowest* of the three measured
scores, not the average: one weak discipline is the one worth practising.
Skipping is always available; the button reads *"I have done this before"* and
is on the first screen. Seeds are recorded as a versioned placement result so a
future rubric change can re-seed without rewriting history.

**What the test must not do:** ask a vocabulary question. Vocabulary is earned by
deed everywhere else in this flow; a placement test that quizzes the glossary
would contradict lesson 6.

---

## 6. Scoring dependency: do not ship lesson 6 as-is

Lesson 6 is the first lesson that shows a score. Two measured problems make it
unsafe to ship before a versioned scoring change.

### 6.1 The discontinuity: 0.0 versus NaN versus "a good score"

- A flight **without** a locked prediction reports `score = NaN`
  (`lesson_6_has_no_score_without_a_locked_prediction`).
- Previous-session measurements: a stage that ends with **7.040 kg** aboard
  scores **0.0**, and **1 kg more** scores **> 0**.

So the player crosses from "no number at all" to "a number", and then from
"zero" to "fine" across a **7 kg** step that no screen explains. In a lesson
whose stated point is *prediction is 25% of your score*, the first score a
player sees must not be a cliff.

**Proposal (versioned, not implemented):** add a documented *pre-scoring*
state. Before a prediction is locked, the debrief shows **five measured numbers
and no score**, with the words "lock a prediction to score this flight". After a
lock, the score is always finite — including 0.0 — so the number never appears
or disappears, it only moves. Bump the scoring revision and record the change in
the changelog; existing certificates keep their revision and re-score
identically.

### 6.2 The prediction-term penalty for fitting hardware

Previous-session measurement: a **+264 kg** card fitted with `ThermalDesign`,
with an *exactly correct* prediction, scores the prediction term at **0.9796×**
instead of 1.0. The objective term is mass-independent (bit-identical across
±900 kg); the prediction term is not, because `remaining_propellant_kg` is
compared against `state.mass − contract.dry_mass_kg`, and the fitted dry mass
differs from the catalog dry mass.

This does not hurt lesson 6 **today**, and the reason is measured:
`lesson_6_is_only_honest_because_it_fits_no_hardware` shows catalog dry
**1 020.015 kg** equals fitted dry **1 020.015 kg** — delta **0.000 kg** — because
the catalog stage fits no hardware at all. Lesson 6's scored vehicle is the flown
vehicle by accident of the card, not by design.

**Boil-off does not leak into this term, measured.** The obvious worry is that
the cryogenic tug's 5.132 kg of boil-off would corrupt the
`remaining_propellant_kg` comparison. It cannot: the term compares the
prediction against the *final* state's `mass − dry_mass`, and the efficiency
denominator is `initial − dry`, both of which are card constants that the tank
model does not move. `lesson_6_scores_identically_in_the_browser` recomputes the
published 0.9238 under the game's own settings and gets **0.9238** to four
decimals, so the score in §2 is the score a player sees.

**Proposal (versioned, not implemented):** score the prediction against the
vehicle's *catalog* dry mass, or explicitly document the prediction term as
vehicle-dependent and surface the penalty in the debrief. Choose one *before*
any lesson lets the player fit an engine, or lesson 6's 1.0000 prediction score
becomes unreachable for reasons the player cannot see.

**Sequencing rule:** lesson 6 may not ship in the same version as any change to
the prediction term's definition. Measure the score under the old and new
definition on the same flight and put both numbers in the changelog.

---

## 7. Gap analysis against `UI.md` Part 3

| `UI.md` step | Systems needed | Rough size | Status |
| --- | --- | --- | --- |
| Accept "launch a CubeSat" contract | Contract authoring for a surface-to-orbit objective | S (card only) | Not built |
| Chief AI on thermal/power/avionics, player on staging | Delegation mask exists; per-discipline staging UI does not | M | Partial |
| `[TEST IGNITION]` on the pad | Pad state, hold-down, sea-level thrust, launch-site gravity | M | Not built |
| Fuel slider until TWR > 1.35 | Pad TWR solver (the engine solver exists) | S | Not built |
| Inspect nozzle during ascent | Engine lab | **Built** (lesson 4 uses it) | Done |
| Ascent to 300 km | Ascent propagator with atmosphere and drag | **L** | Not built |
| Pitch-over / gravity-turn handle | Gravity turn guidance, atmospheric flight | **L** | Not built |
| `v_horizontal` vs `v_orbit` gauge | Cheap once ascent exists | S | Blocked on ascent |
| Prediction + debrief + Elo + dossier | Locked missions, scoring, dossier | **Built** | Done |

**Recommendation: ascent is a separate work package, not a later lesson.**

Three reasons, in order of weight:

1. **It is the only item in the table rated L**, and it is actually two Ls that
   must land together — an atmospheric propagator with drag *and* a gravity-turn
   guidance handle. Either one alone produces a lesson that cannot be completed.
2. **It is the only item that changes the kernel's contract surface.** Every
   shipped contract starts in orbit (§3). A surface-launch objective is a new
   `Objective` variant, a new force model (drag at density), a new failure class
   (structural/aero load), and a new causal rule. That is a versioned physics
   change, not a lesson.
3. **Nothing in this document blocks on it.** The first 30 minutes teach
   apoapsis, periapsis, prediction, plane, separation, gravity loss and scoring
   without ever leaving orbit. Ascent should ship when it can be its own
   20-minute lesson with its own measured harness — and it should reuse lesson
   4's engine lab and lesson 5's gravity-loss vocabulary, which will then already
   be earned.

The one thing this arc *loses* relative to `UI.md` is the visceral "rocket rising
off the pad" moment. That is a real loss and should be recovered in the ascent
work package, not faked with a cutscene.

---

## 8. Causal-DAG coverage

Every anticipated failure above maps to an existing rule except one:

| Lesson | Failure | Rule | Exists? |
| --- | --- | --- | --- |
| 1, 3 | Low side in the atmosphere | `PeriapsisBelowAtmosphere` | Yes |
| 1 | Impossible burn refused | `PropellantDeficit` | Yes |
| 3 | **Plane unchanged by a prograde burn** | — | **No — propose `PlaneUnchangedByProgradeBurn`** |
| 4 | Flow separated in air | `FlowSeparationAtAltitude` | Yes |
| 5 | Burn too long for one arc | `LongFiniteBurnArc` (`burn_sweep_deg > 25`), demo `split-kick` | Yes |
| 5 | Re-entry during the burn | `Impact` | Yes |
| 6 | Score cliff | `PropellantDeficit` covers the fuel side; the score side is §6 | Partial |

**Proposed rule, for review — not implemented.**

`Warning::PlaneUnchangedByProgradeBurn`. Symptoms: "your plane did not move".
Mechanism: "a burn along the direction of travel changes speed and therefore the
far side, not the plane". Cause: "the out-of-plane component of the burn was
zero". Demo: `plane-turn`. Trigger: the flight's inclination change is below
0.02° while the requested inclination change (target − start) exceeds 0.5°. Both
numbers are already in the report (fields 26 and the contract's objective), and
the thresholds are the ones this harness measured: the in-plane burn moved
**0.0089°** and the out-of-plane burn moved **0.5631°**, so 0.02° separates them
with a factor of 2 margin and a factor of 28 margin respectively.

---

## 9. Feasibility table

Verdicts are against measured kernel behaviour, not intent.

| # | Lesson | Tempting failure produces the lesson? | Fix produces the correction? | Unbuilt systems needed | Verdict |
| --- | --- | --- | --- | --- | --- |
| 1 | Your satellite is falling | **Yes** — 2 247 km apo, 273 km peri, no progress on the low side | **Yes** — 557 × 548 km, e 0.0007 | none | **ship** |
| 2 | Guess first | **Yes** — 675 km gap, visible and closable | **Yes** — NaN-free once locked (§6) | none | **ship, after §6.1** |
| 3 | Tilting the orbit | **Yes** — prograde leaves 96.008°; in-plane 100 m/s moves 0.009° | **Yes** — 98.595°, 366.8 m/s, inside budget | none | **ship**; needs new DAG rule |
| 4 | The shaking engine | **Yes** — separates at sea level, attached above 17 km | **Yes** — 18 087 → 13 194 N quantified | none (engine lab shipped) | **ship** |
| 5 | The long-burn trap | **Yes, better than planned** — a legal burn that re-enters, 84% loss | **Yes** — 387 653 km via the shipped splitter | none | **ship** |
| 6 | First real contract | **Yes** — NaN without a lock; 0.9238 with one | **Yes** — real score, real debrief | none | **ship, after §6** |

Every lesson stands. None had to be dropped, and none needs unbuilt physics.
The three lessons whose *story* changed because of measurement are lessons 1, 3
and 5, and in each case the measured physics was better than the guess:

- Lesson 1's tempting attempt is the **whole budget**, not a dramatic overspend,
  because 180 kg of propellant buys 495 m/s and no more.
- Lesson 3's tempting attempt is the **wrong lever** 63× away, not the right
  lever badly aimed.
- Lesson 5's tempting attempt needs **no scripting at all**: one burn is a legal
  plan that ends in a crater.

---

## 10. Playtest protocol

**Participants.** 3–5 players. At least one who has played a KSP-like game, at
least two who have not, and at least one who has never used an orbital-mechanics
tool of any kind.

**Method.** Think-aloud, one investigator, one note-taker, screen + voice
recorded locally. The investigator does not help; the observer only records when
a rung appeared and how long the player had been idle.

**Measurement: local only, opt-in, no accounts, no network telemetry.**

- A counter file written to the local browser profile, containing only: timestamps,
  lesson id, rung depth, lever values, `LOCK` presses, whether the plan compiled,
  and the report fields the game already displays.
- A consent screen before the first lesson that names every field, offers a
  "do not measure" toggle, and lets the player export or delete the file at any
  time. Deleting must be possible without a network connection.
- Nothing is transmitted. There is no account requirement anywhere in this flow,
  which is the same reason there is no server-side tutorial state: the tutorial
  must work on a plane.

**Metrics and their pass targets**

| Metric | Definition | Target |
| --- | --- | --- |
| Time to first input | App open → first drag on the arrow | **≤ 10 s** (design rule) |
| Time to first successful orbit fix | L1 start → periapsis > 500 km and e < 0.02 | median ≤ 4 min |
| Retries per lesson | `LOCK` presses before the success condition | median ≤ 3; none above 8 |
| Hint-ladder depth | Deepest rung shown, per lesson | median ≤ 2; rung 4 in ≤ 1 player per lesson |
| Drop-off point | Last lesson with any input | no more than 1 of 5 dropping before L6 |
| Post-session explanation | "Explain why you needed two burns." | 4 of 5 describe one side moving and the other not |

The last metric is the real one. Every other number can be gamed by shortening
the lessons; **"explain why you needed two burns" cannot be**, and it is the only
metric that tests whether the discovery transferred.

**What invalidates a session.** Any rung-4 ghost the investigator executed for
the player; any moment a player was told a number instead of measuring it; any
refusal from the kernel the player could not have predicted from the lever
(such as `BurnOutsidePlan`, which the clamps in §2 are meant to prevent).

---

## 11. Idle-render contract

The acceptance gate is unchanged: **tutorial overlays must settle to zero
animation callbacks and zero worker messages** (`docs/observatory.md` §
"Rendering and idle behavior"; the gate is
`npm run test:browser` → `oracle_playable_loop_and_idle_gate`).

Consequences for this design, stated now so they are not discovered later:

- The hint-ladder's 20 s idle timer must be a **single scheduled timeout per
  lesson**, cancelled on input, not an interval. An interval would fail the gate
  by definition.
- The ghost preview in each rung-4 step must be drawn from a precomputed
  trajectory, not an animation loop. The engine-lab altitude slider (lesson 4)
  may re-solve on input but must issue **no** worker message while idle, which
  it already does not.
- The burn-arc preview (lesson 5) is the one risky element: it redraws on every
  slider frame. It must use the existing governor's coalescing path and must
  schedule no frames after release, exactly as the orbit view does today.
- The prediction marker (lesson 2) drags with the pointer only; no autonomous
  motion, no pulses. The "pulses" in §4's rung-2 text must be a **static**
  highlight change, since a CSS pulse is a keyframe animation and fails `npm
  test`'s keyframe check.

---

## 12. Open questions for review

1. **Is lesson 2's guess scored, or just recorded?** It currently feeds lesson
   6's prediction term, which means a bad lesson-2 guess survives into lesson 6.
   Proposed: recorded, not scored, until lesson 6.
2. **Does lesson 3 end on a tilt or on a contract?** Measured: a single plane
   change cannot finish contract 102 inside the budget. This document ends the
   lesson on the tilt. If review wants the contract completed, the lesson needs
   a second burn and roughly 5 more minutes.
3. **Lesson 5's splitter caps at 3 kicks** (`split_tug_for_stage` rejects
   anything outside `2..=3`). The harness measured 3 kicks at 387 653 km and
   2 at 158 199 km; more kicks might do better, but the shipped helper does not
   allow them. Should the intro expose 4+? Proposed: **no** — the intro shows
   the shipped lever, and widening it is a separate change with its own oracle.
4. **New DAG rule `PlaneUnchangedByProgradeBurn`** (§8) needs a `demo` art key.
   Proposed: `plane-turn`.
