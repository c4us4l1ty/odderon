# Odderon — Game Design Bible

**Living specification for the rebuild.** This is not final. It is the document Block 1 is built
from, so the most useful thing you can do is disagree with it in specific places.

Companion: [`VISION.md`](VISION.md) (thesis, architecture, tensions), [`CREDIBILITY.md`](CREDIBILITY.md)
(how the claims get earned) and [`GRAPHICS.md`](GRAPHICS.md) (**authoritative for rendering and frame
cost** — where this file and that one disagree about pixels, that one wins).

Hardware target, fixed: **Reference A** — 2015-era laptop, Intel HD 520 / UHD 620, 2 cores, 4 GB RAM,
1366×768. **Reference B** — Android 8+, HiSilicon **Kirin 710** (Mali-G51 MP4), 2–3 GB RAM, Chrome.
Every number below is written against those two machines, not against a gaming PC.

---

## 0. The thesis, in one page

Every mechanic in this bible is the same mechanic wearing different clothes.

```
   ┌────────────────────────────────────────────────────────────────────────┐
   │                        THE LOOP OF CONSEQUENCE                         │
   │                                                                        │
   │   GREASE PENCIL ──►  THE HAND  ──►  PHYSICS  ──►  GHOST DIFF           │
   │    (Expectation)      (Act)        (Reality)     (Attribution)         │
   │          ▲                                             │               │
   │          │                                             ▼               │
   │          └────────────  UPDATED INTUITION  ◄───────────┘               │
   │                              │                                         │
   │                              ▼                                         │
   │            PERSISTENT ARTIFACT  (Akashi / Esse)                        │
   │     "a stranger can click any number and verify my work"               │
   └────────────────────────────────────────────────────────────────────────┘

   Stage   The player does            The kernel does                    What usually fails
   ─────   ──────────────────────     ───────────────────────────────    ─────────────────────
   1 EXPECT  a 0.5 s grease mark      captures the player's implicit     tedious forms, or no
             on a gauge / scope /     model M_expected                   prediction at all
             blueprint
   2 ACT     pulls a valve, throttles  runs the unified solver           a button that says
             an engine, commits a     M_actual                           "Execute burn"
             design
   3 CONSEQ  watches and *hears* the  enforces conservation laws,        fake RNG urgency or
             machine answer; sees the  propagates Provenance<T>           arbitrary explosions
             gap δ from their mark
   4 ATTRIB  flips the visor down     highlights argmax|∂δ/∂xᵢ| on the  a red "MISSION FAILED"
             to see the ghost diff     physical hardware itself           screen, or a wall of text
   5 ARTIFACT signs the log, turns    hashes the telemetry into a        ephemeral points that
             off the lamp at golden     verifiable Akashi report           mean nothing outside
             hour                                                         the game
```

Call it **the Loop of Consequence**. It is the reward-prediction-error circuit that Schultz, Dayan
and Montague mapped in dopamine neurons, and it is also precisely how an engineer is actually made.
Odderon's entire design problem is: *make this loop run in a browser on a 2015 laptop, thirty times
an hour, without ever lying.*

The loop needs four things to fire, and each one is a design pillar:

| The loop needs | Which is pillar | Which is where games usually fail |
| --- | --- | --- |
| A **real expectation** to violate | **Honesty** | Fake urgency instead of real consequence |
| **Agency** in the act | **The hand** | A menu button that says "Execute burn" |
| **Attributable** consequence | **Eidos** | A red "MISSION FAILED" screen |
| A reason to **update the model willingly** | **Felt learning** | Punishment, or condescension |

And then there is the fifth thing, which no simulator has ever had, and which is why this could be a
once-in-a-generation game:

| **The consequence persists as an artifact.** |
| --- |
| The prediction you committed to, the flight you actually flew, the failure you diagnosed, and the report you wrote **outlive the session and leave the game as something a stranger can verify.** That is what turns a loop into a career, and a game into a credential. |

**The one-sentence design law I will enforce in code review:**
> **The Law of Odderon.** Never tell the player in words what the physics could have done to their
> hands, and never show them a number the kernel cannot defend.

### The five mechanisms that make the loop cheap

The loop is only worth defending if it is cheap enough to fire 30 times an hour. Each of the
following is a **reduction** in code, not an addition: they replace a system we would otherwise have
had to write.

| | Mechanism | What it replaces | Spec |
| --- | --- | --- | --- |
| **A** | **The grease-pencil mark** — expectation is a 0.5-second hand gesture, never a form | The Prediction Card's typing, the calculator, the "Submit" button, and the entire prediction UI | §6.1 |
| **B** | **The ghost twin** — attribution is a translucent overlay of *your assumed model* against reality, at the divergence point, with one arrow on the part responsible | The debrief paragraph, the tutorial text, the "why did this fail" screen | §6.2 |
| **C** | **Provenance as a type** — every observable is a lineage node; uncertainty propagates by construction | Ten thousand hand-written tooltips, and the entire "explain this number" UI | §6.6 |
| **D** | **The apartment *is* a spacecraft** — the Day-1 room runs on the same vapor-compression, thermal-network and DC-bus solver as the lunar base | Every "domestic" system we would otherwise have had to write, and the whole prologue | §6.7 |
| **E** | **The epistemic peak** — the game finds *your* moment of maximum δ-resolution and turns it into the ending | Difficulty tuning, session-length heuristics, and the "when should we let them stop" problem | §9.5 |

All five are derived, not invented: A and B are the same sensitivity machinery Eidos already needs
(`GRAPHICS.md`; VISION.md §6); C is the `Provenance` type already required by Pillar 1; D is a
constraint on how the solver is written on day one; E is a detector over data we already record.

---

## 1. The five pillars, with falsifiable tests

A pillar you cannot fail is decoration. Each of these has a test that can be run against a build.

### P1 — Honesty over comfort
The game never manufactures stakes it hasn't simulated, never rations information the player has
earned, and never presents an assumption as an observation.
*Test:* pick any number on any screen. One click must unfold its lineage: the equation, the sensor
noise bar, and — highlighted in yellow — every input the player *guessed* rather than measured. If
any number cannot pass this, it is a bug. Enforced by the `Provenance` type in the kernel (§6.6), not
by discipline: no display-facing system may accept a raw `f64`.

### P2 — Consequence over instruction
Nothing is taught before it is needed. The player's own design, not a tutorial, produces the lesson.
Every failure must be *cheap, informative, and reversible*, and the information must arrive in the
debrief, not in a wall of text before the mission.
*Test:* read the mission-start screen. If it contains a paragraph explaining a concept, delete the
paragraph and make the concept observable instead.

*The wreckage is always examinable.* Never cut to black on failure. The simulation continues through
the aftermath: the smoke clears, you can walk out of the blockhouse, put your hand on the ruptured
metal, and rewind the last 500 ms of thermal and pressure stress **on the scorched hardware itself**.
A failure screen destroys attribution; the wreck preserves it — and the wreck is content we never
have to author.

### P3 — Authorship over ownership
The player should feel they *built* their vehicle, not that they *bought* it. Labor creates
attachment — but only labor that completes, and only labor above a triviality threshold (the IKEA
effect has four preconditions, and failure to complete *inverts* the effect).
*Test:* every design the player flies must contain at least one choice with no default and at least
one consequence they can trace to their own hands.

*Enforcement:* **no working defaults, only broken primitives.** Every new domain hands the player a
physically complete machine with one fatal, glaring flaw — an inherited sounding rocket whose throat
is too narrow and overpressures the chamber; a life-support loop with no scrubber regeneration heat.
Never a blank CAD screen (completion fails, and the IKEA effect inverts into frustration), and never a
working default (zero authorship). One fatal bottleneck, diagnosable in about three minutes: the
effort crosses the non-triviality threshold, completion is guaranteed, and the player claims the whole
machine. We call it **the almost-working inherited machine**, and it is the cheapest onboarding
mechanism in the document.

### P4 — Small room, big sky
The opening space is deliberately cramped, ordinary and *completely* simulated. The universe is
vast not because the map is big but because the first room was small and real.
*Test:* measure the ratio of fidelity to floor area. It should be absurd. Twenty-five square metres
of room should have more simulation detail than a city block.

*Enforcement:* **the apartment is a spacecraft.** We do not write "apartment mechanics". The window AC
unit is a closed-loop vapour-compression heat pump (compressor → condenser → expansion valve →
evaporator) rejecting heat to the summer air. The roof collector is a radiative thermal collector. The
fuse box is a DC bus with breakers. The room is a 25 m² pressurised atmospheric volume exchanging
solar radiative heat through glazing and convective heat through its walls.

The same ~500 lines of network solver then run the capsule, the lunar base, and the O'Neill cylinder.
The player can test this on Day 1: close the curtain and the solar gain through 3 m² of glazing falls
from ~1440 W to ~120 W, the AC's electrical draw drops by roughly 440 W · h⁻¹ of cooling, and the room
cools by several kelvin. **On Day 1, in a cramped room on Earth, they are already living inside the
spacecraft physics engine** — and we have written one solver instead of two games.

### P5 — The game ends well
Players remember the peak and the ending, not the average (duration neglect). Sessions, missions,
and careers all get designed endings. The game will end a session for you, politely, at a high point,
with one loop left open.
*Test:* instrument a session. If the last two minutes are the low point of the session, the design
has failed, and no amount of good middle will save the memory.

---

## 2. The psychological machine

This section exists because you asked me to research the tricks rather than invent them. Each
mechanism below is a real, replicated finding with a citation in Appendix B, followed by the exact
Odderon mechanic it justifies, followed by the way it *backfires* if used lazily. The failure-mode
column matters more than the mechanism column.

### 2.1 The thirteen mechanisms

| # | Mechanism | Research | Odderon mechanic it justifies | How it backfires |
| --- | --- | --- | --- | --- |
| 1 | **Productive failure** — struggling with a problem before instruction beats instruction-then-practice, especially for transfer | Kapur 2008, 2016; Kapur & Rummel 2012 | The player's first cryo load is under-specified on purpose. It froths, the pump cavitates, the test article dies cheaply on the stand. *Then* the debrief explains chilldown. | Unstructured struggle → frustration, not learning. It needs a *consolidation* step immediately after, and the problem must be within reach. Our debrief is that step. |
| 2 | **Desirable difficulties** — spacing, interleaving, retrieval, generation all hurt immediate performance and help long-term retention | Bjork & Bjork 2011; Soderstrom & Bjork 2015; Roediger & Karpicke 2006 | The Academy never gives a lecture twice. It gives you a *new situation* that requires the old knowledge, spaced days apart. | Difficulty without feedback is just noise. Every difficulty must emit a legible result. |
| 3 | **Felt-learning paradox** — students learn more in active conditions but *feel* they learned less | Deslauriers et al. 2019, PNAS | The single most important risk to this game. Productive failure feels bad. We must engineer an honest competence signal into every debrief: "you predicted 412 kg, you had 418 kg — that is a 1.4% error, better than 84% of first attempts." | Without it, players quit believing the game is unfair. This is the #1 churn cause I expect, and it is a *design* problem, not a difficulty problem. |
| 4 | **Reward prediction error** — learning is driven by the *difference* between expected and actual, not the reward | Schultz, Dayan & Montague 1997 | The grease-pencil mark (§6.1). A 0.5 s physical commitment to an expectation, visible on the gauge as a yellow tick, so δ is perceived as *a gap on glass* rather than a number in a table. | A perfectly predicted outcome gives zero learning signal. Do not flatten the variance; make the prediction harder instead. |
| 5 | **Curiosity gap** — curiosity is deprivation caused by an awareness of a *specific, near* gap in knowledge | Loewenstein 1994; Zeigarnik 1927 | Open anomalies: a 3% thrust discrepancy you cannot explain yet. It sits in a drawer. It is small enough to almost solve. | Gaps too big → indifference. Too small → nothing. Also: unlimited open loops become anxiety. We cap it (see 7.3). |
| 6 | **Peak–end rule / duration neglect** — remembered experience is (peak + ending), not the average | Kahneman et al. 1993; Redelmeier & Kahneman 1996 | Session architecture (Section 9). Missions end with a landing, a report, and a warm walk home — never with a failure screen. | Manufacturing fake peaks destroys trust. The peak must be *earned by the sim*. |
| 7 | **Autonomy, competence, relatedness** — intrinsic motivation needs all three; external rewards can *undermine* it | Deci & Ryan 1985; Ryan, Rigby & Przybylski 2006 | No XP bars, no daily streaks, no loot. Progression is *access and trust*, which reads as autonomy. The protégé and the async world supply relatedness. | Over-rewarding a task the player already enjoys reduces intrinsic motivation. Never pay the player for something they'd do anyway. |
| 8 | **IKEA effect / effort justification** — you overvalue what you built, given completion, non-trivial effort, identifiability, self-valuation | Norton, Mochon & Ariely 2012; Kahneman, Knetsch & Thaler 1990 | Every vessel is parameter-authored. The first sounding rocket is a stack of decisions, not a purchase. | Incomplete builds invert the effect. This is why the first vehicle must *fly*, even badly. And why we never let the player start a 40-hour build before their first success. |
| 9 | **Overview effect / awe and the small self** — seeing Earth from space produces self-transcendence and prosocial shift; awe is reproducible via VR | Yaden et al. 2016; van Limpt-Broers et al. 2024; Piff et al. 2015 | The orbital dawn, the first cupola, the shockwave arriving seconds after the plume. Also the *cramped room*, which supplies the contrast. | Awe is a non-renewable currency per playthrough. Rationed by design (Section 10). Also: never fake it with a cutscene. |
| 10 | **Nostalgia as a psychological resource** — bittersweet longing increases meaning, self-continuity, social connectedness; triggered by transition, loneliness, music, scent | Sedikides et al. 2008; Wildschut et al. 2006; Reid et al. 2015 | Cicadas at 3 pm. The AC hum. The specific warm light of the first room. Later, returning to that room after years of game-time. The *wreck of your first rocket still on the pad*. | Pure nostalgia-bait is kitsch. It only works if the thing remembered was genuinely lived, so the first room must be real enough to miss. |
| 11 | **Situated learning / legitimate peripheral participation** — newcomers enter a community of practice at the periphery with a *legitimate* task, and move inward | Lave & Wenger 1991; Brown, Collins & Duguid 1989 | Day 1 you are handed a clipboard and a checklist. It is real work, it matters, and it is boring in the way real entry-level work is boring. Rank 1 → 4 is literally LPP. | Periphery without legitimacy is busywork. The clipboard must actually catch a fault that would have killed the launch. |
| 12 | **Embodied cognition / gesture as retrieval cue** — physical manipulation improves retention and transfer, especially where the gesture matches the concept | Pouw, van Gog & Paas 2014; Skulmowski et al. 2016; Goldin-Meadow et al. 2001 | Canonical gestures (Section 5.2): the LOX fill hand-turn is the *same* gesture every time, in every rocket, for the whole game. The hand becomes the index. | High interactivity can *increase* cognitive load on first contact. So: one canonical gesture at a time, never two new ones in a scene. |
| 13 | **Learning by teaching (protégé effect)** — preparing to teach, and teaching, improves the teacher's own understanding | Fiorella & Mayer 2013; Chi et al. 1989 (self-explanation) | The protégé (5.6). You are given a junior the day you are promoted; you must brief them; they will repeat your mistakes back to you at the worst possible time. | A protégé who is purely a burden is resented. They must also *give* you something — they notice what you missed. |

### 2.2 The three nobody does well, and how I want to do them

These are the ones I believe can make Odderon feel unprecedented. Everything else in the table is
table stakes for a good educational game; these three are the game-changers.

#### (a) Productive failure, *with* the felt-learning fix

The literature is unambiguous and it is a trap: active, struggle-first learning produces better
transfer *and* lower felt learning (Deslauriers et al. 2019). If Odderon is built purely on
productive failure, players will quit while learning the most.

So the design has to include a **competence mirror** that is honest and immediate. After every
prediction, the debrief does not say "great job" — it says:

> You predicted a 42.0 s burn. Actual: 43.1 s. Error 2.6%. Across your last nine predictions, your
> mean error is 3.1% and your bias is +0.8% (you consistently overestimate nozzle efficiency).
> That bias is your next lesson.

That is a *specific, numeric, personal* statement of growing competence. It is not praise. It cannot
be faked and it cannot be ground out. It is the antidote to the felt-learning paradox, and it is also
the seed of Esse. It costs us nothing to compute, because we already have both curves.

#### (b) Legitimate peripheral participation, taken literally

Most games simulate a career ladder with ranks and unlocks. Odderon should simulate the *social
structure* of an apprenticeship, because that is what actually transfers.

Concretely, Day 1 at the company:
- You are given a **clipboard** (a physical object, with real paper on it).
- The clipboard has a **checklist written by a tired human**, with one item that is wrong. The wrong
  item is not a trick: it is a real, common mistake, and it is the first thing you can catch.
- Nobody explains anything. A colleague points at a hose and says "that one" and leaves.
- You are *allowed* to be at the pad. That is the legitimacy. You are not allowed to touch the engine.
  That is the periphery.

By Rank 4 you are the one writing the clipboard, and — this is the part that makes it land — **the
next rookie reads your checklist, including your mistakes.** The community of practice goes all the
way around. That single mechanic converts the entire career ladder from a progression system into a
*closed social loop*, and no space game has it.

#### (c) Peak–end plus Zeigarnik: the designed ending

Sessions and missions are shaped deliberately:

- **The peak is scheduled but not scripted.** Every act gets one designed high-consequence moment
  (the first static fire; the first anomaly in flight; the first time the shockwave arrives late).
  The sim produces it; the game merely arranges the player to be present.
- **The ending is warm.** A mission that ends with a landing ends with an actual walk, in the actual
  light, and a ritual (Section 9.2). Never a modal.
- **One loop stays open.** You close the session with exactly one unresolved question — small,
  specific, and the thing you will do first next time. This is the Zeigarnik effect used *honestly*:
  it is a real question the game will really answer.

### 2.3 What we refuse — the dark patterns, named

Credibility with NASA depends as much on what we refuse as what we build. These are design
constraints, and they are non-negotiable unless you overrule me.

| Refused | Why |
| --- | --- |
| **Daily streaks, FOMO, timed events** | Manufactured scarcity for a game about the *real* scarcity of delta-v. It would make the physics feel cheap, and it punishes adult players with jobs. |
| **Variable-ratio reward loops (loot, gacha)** | Yes, they work. That's why they're unethical here. A game that wants to be recommended by flight directors cannot run a slot machine. |
| **Pay-to-progress; anything purchasable that touches Esse** | Esse must be the one number in the industry that cannot be bought. Monetization is cosmetic or institutional (see 12.3), and never touchable by a simulator-level advantage. |
| **Fake difficulty (bullet sponges, grind)** | The sim is the difficulty. If the difficulty needs padding, the mission design is wrong. |
| **Punishment for absence** | The universe runs on proper time. It waits. Your company may decay, but it never punishes you for closing the tab. |
| **Lying about uncertainty to make the UI cleaner** | The single fastest way to lose the entire premise. `Provenance` is a type, not a policy. |
| **Cutscene awe** | The overview effect only lands if the player caused it, paid for it, and can see their own design in the frame. |
| **An "easy mode" that hides the physics** | Instead, the Eidos visor annotates the *same* physics. Same world, two lenses. |
| **Score inflation to retain players** | Esse is epoch-scoped and published with its confidence interval. |

---

## 3. The core loops

### 3.1 The hand (seconds)
You turn a physical knob. Something in the world responds with a sound and a number. The number
moves an instrument needle. Nothing else happens until you decide.
*Design rule:* **10 seconds.** Every input must produce an observable consequence within 10 seconds.
If a realistic process takes 45 minutes, it must produce *intermediate* consequences (a hiss, a
frost line, a temperature falling) every few seconds.

### 3.2 The mission (30–90 minutes)
```
BRIEF (constraint, not answer)
  → DESIGN / PREPARE (authoring; Eidos available)
    → PREDICT (commit + hash; scored)
      → EXECUTE (hands; real time; anomalies)
        → DEBRIEF (predicted vs actual; root cause; Ghost Run)
          → RECORD (Akashi entry; Esse update; Proof-of-Flight bundle)
```
Every mission, however small, contains all six beats. A cargo launch is not skipped — it is
*compressed*, because you solved it by automating it (Section 5.5).

### 3.3 The session (60–120 min)
Two or three missions, or one large one. Rises: first contact with a new system → struggle → the
moment it works → a quiet return. Ends on a designed ending and one open loop. Never ends on a
failure screen (Section 9).

### 3.4 The career (50–500 hours)
Rank ladder on one side, *self-directed* arc on the other. The player can leave the company at any
point with no penalty other than consequence: the ladder is faster and safer; the independent path
is slower and yours. Neither is the "real" game.

### 3.5 The life (the archive)
This one is new. The save file is not a number. It is:
- a **museum** (Section 11.4) containing every vehicle you built, every report you wrote, every
  wreck you left,
- a **calendar** of proper time, with people in it who age,
- a **verification bundle** anyone can replay.

The endgame of Odderon is not a final boss. It is: *someone else can look at what you did and believe
it.*

---

## 4. Progression without XP

### 4.1 What actually gates you

There is no XP bar. Five real currencies, all diegetic:

| Currency | Earned by | Spent/consumed by | Felt as |
| --- | --- | --- | --- |
| **Money** | contracts, sales, salvage | hardware, propellant, salaries, leases | freedom (Path B/C/D) |
| **Access** | trust: not blowing up the pad | promotion, pad time, security clearance | being let near the thing |
| **Knowledge** | the Loop of Consequence | nothing — it cannot be taken | confidence |
| **Reputation** | delivered on prediction | contracts you are offered | being asked, not applying |
| **Time** | — it is always spent | everything; and it is per-observer | mortality |

**Knowledge is the only currency that cannot be lost.** That is the whole game. It is also why the
teaching-a-junior mechanic is the deepest progression in the game: it is the only way to *convert*
knowledge into reputation without flying anything.

### 4.2 The ladder is an apprenticeship, not a level tree

| Rank | Real job | The actual task | What it teaches | Why it's not busywork |
| --- | --- | --- | --- | --- |
| 0 | Nobody | Wake up, get to work, be late once | The world is a schedule | Establishes that transport is real |
| 1 | Pad technician | Clipboard, hoses, valves, chilldown, ullage | Fluids, cryogenics, pressure, sequence | You will catch one fault that would have destroyed the vehicle |
| 2 | Junior engineer | A constraint with a deadline: *"40 kg too heavy, Isp short, by Friday"* | Trade studies, nozzle/expansion, cooling | Your fix flies |
| 3 | Flight director / test pilot / astronaut | Call Go/No-Go under time pressure, or fly it | Comms, abort logic, physiology, judgement | You are the last line of defence |
| 4 | Chief engineer / CEO | Capital allocation, hiring, contracts, failure review boards | Economics, risk, leadership | You sign for other people's mistakes |

Each transition is a *promotion, not a purchase*, and each one is earned by a specific, observable
act of judgement — usually catching something. This is the LPP structure from 2.2(b).

**And the ladder is not a ladder.** It is a set of companies with different ladders, and a career is
a path through several of them. You may resign — a physical act: you hand back the clipboard. You may
be hired laterally at a rank *above* where you left, because what is being bought is your Esse and
your Akashi, not your seniority. Moving from pad technician at a struggling sounding-rocket outfit to
junior engineer at a bigger one is simultaneously an advance (title, hardware) and a regression
(smaller budget, less trust, someone else's procedures). The game represents both. Reputation is
portable; that is what makes the exported report the real currency, and it is why the protégé is a
consequence of *role* rather than of time in service.

### 4.3 Esse — the rating, defined precisely

Esse must be three numbers, not one, because they measure three genuinely different faculties, and
because collapsing them into one number is how rating systems start lying.

```
Esse ≡ ( E_cal , E_par , E_an )

E_cal  Calibration     Predicted vs. actual, across all domains.
                       Scored with proper scoring rules (CRPS for intervals, log score for
                       categorical calls). Aggregated into a reliability curve, not an average.
                       Reported with N and a 95% interval. Provisional until N ≥ 30.

E_par  Pareto          Hypervolume improvement contributed to the global front on weekly
                       contracts (same epoch, same ephemerides for everyone).
                       Zero-sum-ish, floor-published, so the metric cannot inflate.

E_an   Anomaly         Blind, ranked, clocked root-cause resolution. Duel-style Elo per
                       mission; weighted by how deep the fault was hidden and how little
                       telemetry you were given.
```

Rules that make it trustworthy:
1. **Proper scoring rules only.** Never accuracy percentage, never "points". A proper rule cannot be
   gamed by hedging or by claiming impossible certainty.
2. **Committed before execution.** The grease mark is hashed and timestamped before the sim
   advances. You cannot retro-fit a prediction.
3. **Epoch-scoped.** Live catalog sync changes the universe. Ratings are pinned to an epoch; we never
   silently compare across epochs.
4. **Recomputable.** Anyone can download the input log and re-simulate. The rating is a function of a
   verified replay, not a server's word.
5. **Never purchasable, never boostable, never decaying.** Absence does not erase skill.
6. **Shown to the player in plain language**, with their own error distribution, because Esse is
   also the competence mirror from 2.2(a).

> Anchor sense: 1200 = has flown; 1500 = calibrated amateur; 1800 = undergraduate aerospace level;
> 2100 = practising professional; 2400 = exceptional. Any stated anchor is a *claim* and will be
> published with its evidence in the credibility doc, or not at all.

### 4.4 Akashi — the portfolio

An Akashi entry is generated deterministically from `(design hash, epoch, input log)`. It contains:
parametric cross-sections, the P&ID, the porkchop, predicted vs. actual curves, margin tables, the
provenance of every assumed constant, and the player's own written root-cause analysis (which is
*required* — an unexplained anomaly does not become an entry).

It exports as HTML (primary, self-contained, offline-readable) and PDF (print). Every export embeds a
verification link. **The report is the product.** A player who never becomes a CEO but has 40 good
Akashi entries has a career document.

### 4.5 Eidos — the lens

Eidos is not a tooltip. Because every model is analytic, the kernel can produce sensitivities on
demand. Hovering anything answers: *what happens if this changes, and by how much?*

- **On a component:** ∂Δv/∂ε, ∂T_liftoff/∂ε, drawn as flow lines between the part and the effects.
- **On an equation:** every term is hoverable and highlights the physical object it refers to.
- **On a trajectory:** the derivative of the path with respect to the next input — literally "the
  shape of what you're about to do".

Eidos is *free to build* because we refused to make the sim a black box. It is also the accessibility
mechanism that replaces difficulty settings.

---

## 5. The interaction grammar

Zero-UI is a *grammar*, not an absence of interface. The dashboard disappears; the control panel
stays, because a control panel is a physical object that a pilot actually touches.

### 5.1 The actuator repertoire — the complete set of verbs

```
Valve { id, target }        // open/close/modulate a real valve
Throttle(f32)               // main thrust, with real spool lag
Gimbal { x, y }             // thrust vector
Switch { id, bool }         // anything with a toggle
Hand { ray, grip, twist }   // reach, grab, turn — the universal verb
Panel { id, button }        // physical push-buttons on a real panel
Text { field, string }      // for checklists, notes, procedures
```

That is all. There is **no verb** for *add propellant*, *execute burn*, *repair*, *refuel*, or
*unlock*. Those are all consequences of the verbs above. A CI lint rejects new kernel entry points
that are not actuators (see `VISION.md` §3, law 4).

### 5.1b The second verb: the mark

The actuator set in §5.1 is the *acting* grammar. There is one more verb, and it belongs to the
*expecting* grammar:

```
Mark { instrument, value, band }   // a yellow tick on glass, a chalk tick on a scope, a pin on a print
```

It is not a UI element; it is a physical object with physical behaviour:
- it is a **China marker**, and it lives in your coveralls pocket, so it can be *forgotten* (a player
  who forgets to mark flies uncalibrated, and their Esse says so),
- it **stays on the glass** for the session, accumulating a visible record of your own predictions,
- it can be **wiped off** with a thumb, which is the honest way to abandon a prediction you no longer
  believe — and it is recorded, because a wiped mark is data about you too,
- long-press widens it into a **band**: the physical form of "I don't know yet".

### 5.2 Canonical gestures

A small set of hand motions, reused verbatim for the whole game, so that the *motion* becomes the
memory index (mechanism 12 in the table):

| Gesture | Physical action | Meaning | First met |
| --- | --- | --- | --- |
| **The fill** | quarter-turn of a ball valve, then hold | any propellant transfer | Day 1, pad |
| **The bleed** | pull a T-handle out, wait, push back | any vent/purge | Day 1, pad |
| **The tap** | two-finger tap on a gauge face | read the instrument closely | Day 1, workshop |
| **The crank** | circular turn with resistance and ratchet | anything with stored energy | Block 2, test stand |
| **The bolt** | hold + rotate, 6 discrete detents | any structural fastener | Block 2, after a failure |
| **The visor** | swipe up/down at the brow | Eidos on/off | Block 1, apartment |

Six gestures. Never add a seventh without deleting one.

### 5.3 The instruments are the HUD

There is no floating overlay by default. Everything the player needs is either:
1. **physical** — a gauge, a needle, a lamp, a sound, a shake;
2. **in the room** — the light through the window tells you the time of day;
3. **in the visor** — the Eidos layer, explicitly an object on the player's face, which can be
   removed, and which is intentionally *not* available in ranked visor-up play.

### 5.4 The mobile case (this is the hard one)

A zero-UI first-person simulator on a 2017 mid-range Android phone in a browser is the single
riskiest requirement in this document. Rules:

1. **The screen is the cockpit, not a camera.** On mobile, first-person *interiors* are the default;
   outdoor walking uses a constrained, guided view. Don't fight the platform.
2. **No virtual joystick skin.** Touch directly on the object: drag the valve, slide the throttle
   lever, pinch nothing. Targets are ≥ 9 mm effective (Fitts).
3. **Fewer than 7 interactive objects visible at once** (Hick). Zoom/step in to reveal more.
4. **The CAD terminal and the Akashi desk are better on a phone than on a desktop.** Lean into this:
   mobile is where you *review*, *design*, and *read*; desktop is where you *fly*.
5. **Never require simultaneous precise inputs.** Two-thumb precision is not a thing on glass.
6. **Portrait mode is a first-class mode** for terminal/instrument views.
7. **Resume, don't restart.** Mobile sessions are 4–9 minutes. Every screen must be a legal entry
   point, and the game must be resumable mid-mission with a *re-read* of the state, not a rewind.

### 5.5 Automation is a physical object, not a setting

To resolve the "45-minute cryo load × 50" problem without a "skip" button:

- The **logic rack** is a physical panel in the bay. You wire relays to sensors with visible
  connectors: `IF pump_housing_temp > 92 K THEN hold_ignition`. It compiles into the same
  deterministic tick loop.
- Ranked/Esse play always permits automation you authored. Automation you *bought* is allowed but
  flagged in the report, because that is exactly how real engineering works and pretending otherwise
  would be dishonest.
- The expert's reward for automating is **time compression on the boring parts**, which is the
  correct reward in a game about delta-v.

### 5.6 The protégé

Promotion to Rank 2 gives you a junior. This is a *mechanic*, not a character:

- They ask you questions you must answer with a selection from your own reasoning.
- They make the mistake you made last week, because the sim replays *your* last week's error pattern.
- Answering them well produces an Akashi sub-entry, a reputation gain, and — this is the point — a
  **self-explanation** event, which is one of the better-evidenced learning interventions we have.
- They can notice things you missed (a residue on a fitting, a number that doesn't rhyme), which is
  the "give" that keeps them from being a burden.

---

## 6. The pedagogy machine

The loop must fire **30 times an hour — once every two minutes** — with no forms, no typing, and no
paragraphs. That constraint is what produced the five mechanisms in §0. This section specifies them.

### 6.1 Expectation is a grease pencil, not a form

*Replaces: the Prediction Card's typing, the calculator, and the entire prediction UI.*

Real test pilots, flight engineers and submarine mechanics carry a **China marker**. Before they open
a valve or throttle up, they put a quick yellow mark on the glass face of the pressure gauge where
they expect the needle to settle. Odderon's expectation gesture is exactly that, and it costs half a
second:

| Scale | Where you mark | What you mark | Loop frequency |
| --- | --- | --- | --- |
| **Micro** | gauge glass, switch cap, lever travel | where the needle will settle | every 1–2 min |
| **Meso** | trajectory scope, docking reticle, porkchop | where the periapsis / intercept / arrival will move | every 5–15 min |
| **Macro** | CAD blueprint, ledger, habitat cross-section | where the thrust/mass ratio, margin, or NPV will land | every 30–60 min |

**The gesture:** hover the gauge → a faint yellow ghost tick follows at the pointer (*this already
commits nothing*) → one click snaps a yellow mark onto the glass → you pull the lever.

**The payoff:** when the needle swings past your mark into the red, the gap between the yellow mark
and the steel needle **is δ**, and the brain registers the violation in about 50 ms with no words. The
grease mark stays on the glass for the rest of the session as a record of the gap. Over a session, the
player accumulates a row of yellow marks on their own instruments — a visible, private calibration
chart, made of nothing but their own predictions.

Design constrains, so this cannot become a chore:
- **Optional, never required.** A mark that isn't there simply produces no δ — and the player feels the
  absence. Nothing nags.
- **Uncertainty is a blur, not a number.** Long-press a mark to widen it into a band. A wide band is an
  honest "I don't know yet", and it scores honestly (proper scoring rules, §4.3).
- **Captured implicitly.** The mark records the player's *assumed model* — constant Isp, rigid hull, no
  boil-off — because the kernel knows which simplification the mark implies. That is what makes §6.2
  possible with no extra input.

### 6.2 Attribution is a ghost twin, not a paragraph

*Replaces: the debrief paragraph, the tutorial text, and the "why did this fail" screen.*

The player committed to an expectation, so the kernel holds **two models**:

- **M_expected** — the player's implicit model, at the moment of the mark (vacuum Isp, rigid hull, no
  boil-off, no flex).
- **M_actual** — the real solver, with atmospheric back-pressure, structural compliance, and thermal
  leak.

On divergence, Eidos overlays **the translucent yellow ghost of what you assumed** onto solid reality,
frozen at the divergence instant, and computes

```
argmax over xᵢ of | ∂(Actual − Expected) / ∂xᵢ |
```

then draws **one glowing vector arrow on the physical part responsible** — for example, air vectors
pressing inward on the over-expanded nozzle lip while your ghost rocket pulls ahead of the real one.
No paragraph. The player sees the atmosphere crushing their exhaust plume relative to their own
assumption, and the sentence is *unnecessary* — which is why the Law of Odderon forbids writing it.

Three rules that keep this honest:
1. **One arrow, not five.** The single largest contributor. Five arrows is a lecture.
2. **The ghost is the player's model, not a best guess.** If their assumption was crude, the ghost is
   crude, and it is labelled with a China-marker yellow so it can never be mistaken for data.
3. **The textual debrief still exists — but behind the visor, and it is generated, never authored**
   (*"T+42 s: velocity 180 m/s below prediction; ∂v_final/∂ε = +14 m/s per unit expansion ratio at
   vacuum, −22 kN liftoff thrust at sea level"*). It exists for the report and for professional users,
   and it is what the Akashi export prints. The player is never *required* to read it.

### 6.3 The Ghost Run

One counterfactual, played back side by side with the real flight: only the input you changed. This
is the single fastest intuition-building mechanic I know of, and it is nearly free once the kernel is
deterministic and replayable.

### 6.4 Jargon policy

**A term is never introduced before the player has succeeded at the thing it names.**

Order of operations, always:
1. The player does X and observes consequence Y (cause-and-effect language, no jargon).
2. The player notices the pattern (Eidos shows it as a shape or a slope).
3. *Then* the word arrives — spoken by a colleague, or written on the clipboard they were handed,
   as if it were always there.

Never: glossary, tooltip-on-hover for a concept the player hasn't met, "tutorial" language.

### 6.5 Consolidation, not a lecture

After a productive failure, the game does not explain. It **shows the divergence, then hands back
control.** The player re-flies it. The lesson is theirs.

### 6.6 Provenance as a type, not a tooltip policy

*Replaces: ten thousand hand-written tooltips, and the entire "explain this number" UI.*

**The architectural decision:** no display-facing system may accept a raw `f64`. Every observable
quantity is a **lineage node**:

```rust
struct Prov {
    value: f64,                  // what the gauge points at
    sigma: f64,                  // standard uncertainty, propagated not hand-set
    source: EpistemicSource,     // MeasuredBy(Sensor) | Catalog(GaiaDR3) | DerivedFrom(Eq)
                                 //   | Assumed(by: Player) | Modeled(name, sigma)
    inputs: NodeRange,           // the lineage DAG (indices, not a tree of boxes)
}
```

Two candidate implementations, and the choice matters enormously for performance:

| | A. wrapper on every operation | B. node graph built once per design *(chosen)* |
| --- | --- | --- |
| in-loop cost | ~120 ns per operation → **~1 s of CPU per second of flight**. Fatal. | **0**: the integrator runs flat `f64` |
| when provenance is built | continuously | once, when a design/gauge changes (~120 nodes) |
| when uncertainty is computed | continuously | on demand: a Jacobian, ~720 flop for 120 nodes × 6 inputs |
| cost of clicking a gauge | — | ~0.7 µs. Free. |
| failure mode | the game does not run on the target hardware | none we can see |

So the **hot path stays flat arithmetic**, and the **lineage is a small graph evaluated lazily**. That
is the whole trick, and it is why Pillar 1 costs us nothing instead of costing us the frame rate.

What this buys, with zero per-number UI work:
- Click any gauge, ledger entry, or orbital element anywhere in the 4,000 light-years → the provenance
  tree unfolds: the equation, the sensor noise bar, and **every input highlighted in yellow that the
  player guessed rather than measured**.
- Uncertainty propagates by construction (`σ_f = √Σ(∂f/∂xᵢ·σ_xᵢ)²`), so a number can never silently
  acquire false precision.
- The telescope's angular error bars, the exoplanet hypothesis weights, and the atmosphere model's
  declared validity range are all **the same mechanism**. The Law of Odderon stops being a promise and
  becomes a type error.

### 6.7 The apartment is a spacecraft

*Replaces: every "domestic system" we would otherwise write, and the entire prologue.*

We do not write apartment mechanics. We write one solver, and the apartment is a use of it:

| In the room | What it actually is | Same code later runs |
| --- | --- | --- |
| Window AC unit humming | closed-loop vapour-compression heat pump: compressor → condenser → expansion valve → evaporator, rejecting heat to outdoor air | capsule ECLSS cooling, habitat thermal control |
| Roof solar collector | radiative thermal collector with a fluid loop and a stagnation limit | lunar base heating, O'Neill cylinder agriculture |
| Fuse box on the wall | DC bus, breakers, load budget, voltage sag under startup inrush | every spacecraft power bus in the game |
| The room itself | 25 m² pressurised atmospheric volume: solar radiative gain through glazing, convective loss through walls, infiltration | capsule cabin, habitat sector, ark ship compartment |
| The kettle, the fridge, the lamp | real loads on the real bus with real duty cycles | — |

The Day-1 experiment, which the player will find without being told: **close the curtain.** Peak
summer solar gain through 3 m² of glazing at 800 W/m² and SHGC 0.6 is ~1440 W; closed it is ~120 W.
The room cools by several kelvin over minutes, and the AC's electrical draw visibly drops on the wall
meter. **On Day 1, in a cramped room on Earth, the player is already living inside the spacecraft
physics engine** — and we have shipped one solver instead of two games.

This is also why the room can be modelled to absurd fidelity without any bespoke content: fidelity is
a property of the solver, and the solver is the whole game.

---

## 7. Failure design

> A game that cannot let you fail is a game that cannot let you learn. A game that punishes failure
> expensively is a game nobody finishes.

### 7.1 Five kinds of failure

| Kind | Example | Cost | Educational payload | Must be recoverable? |
| --- | --- | --- | --- | --- |
| **Cheap physical** | test article burns through on the stand | money, an afternoon | the richest: you see the mechanism | yes, always |
| **Procedure** | you ignite before chilldown completes | a flight, an investigation | sequence and discipline | yes, with a review board |
| **Design** | TWR < 1, vehicle never leaves the pad | days, reputation | fundamental | yes |
| **Judgement** | you launch into weather you saw coming | reputation, sometimes crew | the professional core | expensive, never fatal |
| **Crew** | someone is hurt | the most | irreversible | **only once per career, and never scripted** |

Crew loss is the one place the game is allowed to be heavy. It is not a game-over; it is a
*consequence that the rest of the career is lived in the shadow of*. It must be rare, earned, and
never used for drama.

### 7.2 The cost curve

Failure must get *more expensive as understanding grows*, because that is the real shape of
engineering, and because cheap failure forever becomes a joke:

- Act I: failure is almost free (a garage, a small grant).
- Act II: failure costs a vehicle but not a company.
- Act III: failure costs a contract, a customer, sometimes a person.

The player should feel this shift as a *rise in seriousness*, not as a difficulty spike.

### 7.3 The Anomaly Drawer — capped open loops

Open anomalies live in a drawer in the workshop. Each is a *specific*, small, real discrepancy. The
drawer holds **at most five**, and when full, the game will not open a sixth without closing one.
This is the Zeigarnik effect used with a budget — enough to pull you back tomorrow, never enough to
become dread.

### 7.4 Rescue over restart

Where physically possible, a bad situation is a *mission* rather than a reload: the tumbling
satellite you can still catch, the crew you can still bring home, the leak you can isolate. Rescue is
the best content in the game because the player's understanding is the only thing that can save it.

---

## 8. Onboarding: the first 30 minutes

This is the most important 30 minutes in the product, and the one I expect to rewrite five times
after watching people play. Written minute-by-minute, with the mechanism each beat serves.

| Time | What happens | Mechanism | Rule being tested |
| --- | --- | --- | --- |
| 0:00 | Black. A fan. Cicadas far away. A window AC cycling on. No logo, no menu, no music. | Sensory anchor, nostalgia (10) | Never open on a title screen |
| 0:20 | Eyes open. Sunlight bars across a wooden desk. Dust in the light. | Small room (P4) | The first frame must be *warm* |
| 0:45 | A phone buzzes: a shift starts at 8. It is 7:41. | Autonomy with a real stake | The first objective is *time*, not a quest |
| 1:30 | Interact: tap a thermometer (the canonical *tap*), open a window, the grille rattles. | Canonical gesture 1 | Learn input on an object with no stakes |
| 3:00 | Down the stairs. A tram is visible; you can miss it. | Transport is real | Missing it is a *lesson*, not a fail state |
| 5:00 | At the edge of town, a distant white plume. Count 8 seconds. Then the windows rattle. | Consequence at distance; awe seed (9) | The game's first "wow" is a *computation* |
| 6:00 | Clipboard handed over. One item is wrong. | LPP (11); productive failure (1) | Nobody explains anything |
| 9:00 | The hose. The *fill* gesture. Frost line. A gauge that doesn't move. | Embodied cue (12) | First real system, one valve |
| 13:00 | The wrong checklist item causes a vent to be missed; pressure alarm. | Productive failure, low cost | Failure is *small* and *attributable* |
| 16:00 | Static fire. Two yellow marks on two gauges: one you'll be right about, one you won't. | RPE (4), grease pencil | Prediction before consequence, always — and it costs half a second |
| 18:00 | Fire. Sound. The stand shakes. Something is *off* in the pitch. | Peak (6); ears as instrument | The peak is earned |
| 22:00 | Debrief. Your prediction vs reality. A number about *you*. | Competence mirror (3) | Felt-learning fix, non-negotiable |
| 25:00 | Walk home in the late light. The pad is an ember. | Warm ending (6) | Sessions end on the return, not the fire |
| 28:00 | On the desk: one open question, written on the clipboard in your own handwriting. | Zeigarnik (5), capped | One loop, not five |

Deliberately absent: any mention of delta-v, orbit, apogee, TWR, or the word "tutorial".

---

## 9. Session architecture

### 9.1 The four-minute rule
Nothing may be four minutes of the same texture. If a process is longer than four minutes of real
time, it must have a *beat* every four minutes: a change in sound, a decision, a looking-out-the-
window.

### 9.2 Clocking out (the ending ritual)
Every session ends with the player *leaving*. Not with a menu. The diegetic exit is:
- save happens implicitly (the save is an input log, so it always exists),
- a walk, or a climb out of the cockpit, or a curtain,
- the one open loop, shown once, in-world.

If the player closes the tab mid-task, nothing is lost and nothing is punished; on return, the world
has moved on by however much time passed, *at the rate the world moves* (Section 11).

### 9.3 The morning after
On return, the first thing is not a summary screen. It is the room. Then the drawer. Then the day.

### 9.4 There are no loading screens
Not "short" ones — none. Entering a ship, leaving a planet, crossing a solar system: all of it is a
continuation of the same world, never a suspension of it. The only moment of waiting in the entire
game is the first boot, and it is choreographed as *waking up* — a fan, cicadas, a room fading up from
black — because the opening scene is pure fragment shader and needs no simulation to render.

Full doctrine, including the 8.6-order-of-magnitude descent problem, the nine rules, and the CI-gated
hitch budget: [`IMMERSION.md`](IMMERSION.md). Also note **N8: no teleportation.** The absence of a
loading screen is a commitment to the absence of fast travel — travel costs what physics says.

### 9.5 The epistemic peak, and the golden hour

*Replaces: difficulty tuning, session-length heuristics, and the "when may they stop" problem.*

Most simulators end when the player is exhausted at 02:00 after a failed launch — which guarantees a
negative memory of the session (peak–end, §2.1 mechanism 6). We can do better, because we *record the
data needed to detect the peak*.

**The detector.** During play, the game tracks a scalar: the **epistemic peak score** — how sharply
the loop just closed. It is computed from quantities we already have:

```
peak_score ≈ |δ| / σ(prediction)        // how surprising, normalised by your own stated uncertainty
           × convergence                 // did you nail it, or did nobody know?
           × stakes                      // mass × energy × irreversibility of what was at risk
           × novelty                     // first time you have seen this class of divergence
```

So the two moments that score highest are the two that matter: *"I called it within 1%"* and *"I had no
idea and reality showed me something new."* Rote repetition scores near zero, which is correct.

**The choreography.** When the score crosses a threshold after ≥ 40 minutes of play, three things
happen, and none of them is a menu:

1. **The light changes.** The local environment transitions to **golden hour** — long amber
   *Shinkai* shadows across the workshop floor, cicadas giving way to crickets. This is a lighting
   parameter, not a cutscene, and it costs nothing (PERFORMANCE.md I7).
2. **A partial packet arrives.** The dot-matrix telemetry printer on the desk chirps and prints the
   *first half* of something high-stakes:
   `RADAR, LUNAR SOUTH POLE — subsurf. dielectric anomaly at 14 m depth, reflec.—`
   (the rest of the line is cut off by the sheet feed: a curiosity gap opened with a physical object)
3. **A physical choice, not a prompt.** A small card is taped to the desk lamp switch:
   *"Call it a day at the peak? The log is saved. One unread signature on the desk."*
   You click the lamp off. The warm room falls into cool twilight. The session ends at its highest
   point, with one loop open — Zeigarnik, budgeted, and *printed on paper*.

**Why this is the correct engineering.** The ending is not authored content; it is a reaction to a
measured property of the player's own session. It cannot be exploited (it fires on real δ), it cannot
be repeated for reward (novelty decays), and it turns the peak–end rule from a thing we hope for into a
thing we schedule. The game is literally **designed to be put down at the right moment** — which is the
opposite of every retention mechanic in the industry, and the reason an adult with a job can play it
for a decade.

---

## 10. Awe budgeting

Awe is the most valuable and most easily spent resource in the game. Rules:

1. **Ration by act.** One designed awe event per act. Everything else must be found.
2. **Never announce it.** The best moment in the game should be one the player was not looking at.
   Concretely: a launch you can *miss* because you were fixing a pump. That is the "unphotographed
   launch", and it is worth more than any cutscene because it belongs to the player's attention.
3. **The overview moment is earned, physical, and quiet.** First cupola. No music. The reflection in
   the glass. The instruments still ticking. The player's own hand on the frame.
4. **Contrast is the mechanism.** The cramped room is not a prologue; it is the *instrument* that
   makes the sky large. Return to it deliberately, years later, in-game.
5. **Never fake scale with numbers.** Awe comes from *being somewhere*, not from a "1,000,000 km"
   label.

---

## 11. World, story, and the social layer

### 11.1 The city is the quest board
No quest markers. Contracts arrive as paper, phone calls, people at the gate, and bulletin boards.
Jobs are generated from real catalogs and real economics, so they are infinite without being random
nonsense.

### 11.2 Async presence ("causal mail")
Other players' ships appear in your sky as objects with real orbital elements, and their messages
arrive with real light-time delay, because the kernel is deterministic and the universe is a function
of time. No netcode, no lobbies, no tick reconciliation. Relatedness, delivered at the speed of light.

### 11.3 What the story is about
Not "humanity's destiny". The theme is narrower and truer:

> **Competence is a form of belonging.** You begin as a cog who is not sure they matter, and the game
> never tells you that you do. It lets you become someone whose judgement other people rely on. That
> is the entire emotional arc, and every system in this document serves it.

Which is why the ending is not a victory. It is a **handover**: the clipboard, in someone else's hands.

### 11.4 The museum of your work (this is the once-in-a-generation idea)
Your archive is a place. A building on the edge of town, growing as you build. Inside:
- every vehicle, at true scale, in parametric geometry (zero assets — so this is cheap),
- the wrecks, kept, with the anomaly that killed them,
- the reports, readable on a desk, in the order you wrote them,
- the people you worked with, in photographs that were rendered from the sim you ran,
- and, at the end, the pad, still warm.

A save file that is an architecture. Nobody has shipped this, and it costs us almost nothing because
geometry is generated and the reports already exist.

---

## 12. Economy and ethics of progression

### 12.1 Money is physics, not a difficulty knob
Contract prices derive from Δv, mass, risk and time; costs derive from materials, propellant, and
labour. The player discovers orbital refuelling the way the industry did: by watching the balance
sheet die. Nothing is fudged to make a mission affordable.

### 12.2 The failure of a company is a valid ending
Bankruptcy is real, recoverable (you keep your knowledge and your Akashi), and moves you to a
different path — the backyard shed, the freelancer, the pirate. A game about competence must allow
you to lose the company without losing the skill.

### 12.3 Monetization (stated now, because it constrains design)
- Free to play the core simulator.
- Paid: cosmetic *in-world* objects (a better desk lamp) and institutional licences.
- Never: anything affecting Esse, sim fidelity, mission access, or the ability to verify someone
  else's report.
- Verification is free and always will be. A recruiter must never pay to check your work.

---

## 13. Difficulty and accessibility

### 13.1 Two visors, not two difficulties
`Visor down` (annotated, Eidos, cause-and-effect language) and `visor up` (raw instruments, ranked).
The player flips a *physical object* to change it. Same physics. No mode-selection menu, because a
menu is an admission that the world isn't enough.

### 13.2 Adaptive help that is never condescending
When the player is stuck (repeated identical failure), help arrives **as a person**: a colleague
walks over, or the protégé asks a question, or the drawer has a note in someone else's handwriting.
Never a popup. Never "hint".

### 13.3 Accessibility budgets (targets, not aspirations)
- **Vision:** full colour-blind-safe palette for all instrument coding (never colour alone; shape +
  position + sound). Scalable instrument text. Visor font is a stroke font, so it is crisp at any
  size at zero texture cost.
- **Hearing:** every audio cue has a visual channel, and vice versa — which is *physically correct*,
  since we synthesize sound from kernel numbers and can therefore also draw the waveform.
- **Motor:** no action requires sub-200 ms precision. Every canonical gesture has a "slow mode" that
  is diegetically a torque wrench / a two-handed grip.
- **Cognitive:** the Eidos visor *is* the accessibility feature. Progressive disclosure is the
  default; complexity is opt-in by flipping the visor up.
- **Time:** pause anywhere in single-player; the sim is a function of time, so this is free.

### 13.4 Performance budget (your hardware, not mine)

Reference A: **2015 laptop**, Intel HD 520 / UHD 620 class, 2 cores, 4 GB RAM, 1366×768.
Reference B: **Android 8+**, HiSilicon **Kirin 710** (Mali-G51 MP4), 2–3 GB RAM, Chrome.

**The renderer is specified in [`GRAPHICS.md`](GRAPHICS.md) — it is the authoritative document for
how pixels are produced and what they cost.** `PERFORMANCE.md` is kept for the platform-level
doctrine that still holds (no game loop, events instead of ticks, audio carries latency). Summary of
the contract:

| Budget | A (desktop, 1080p) | B (mobile, Android 8) |
| --- | --- | --- |
| Output resolution | **1080p, true** | 1080p, adaptive to 720p |
| Pixels actually shaded per frame | **~1/3** (progressive refinement) | ~1/4 |
| Target frame rate | **60, floor 30** | 30, floor 24 |
| Frame budget | 60 fps (16.6 ms); static camera < 1.5 ms (R2) | 30 fps floor |
| Draw calls | **≤ 8** | ≤ 8 |
| Triangles, visible | ≤ 120 k | ≤ 60 k |
| Bandwidth, per frame | ≤ 32 B per screen pixel (single pass) | ≤ 32 B per screen pixel |
| Shadow maps | **zero** (analytic gobos) | zero |
| MSAA | **none** (temporal + analytic edges) | none |
| Textures | procedural only + 1 stroke font | same |
| Resident memory | ≤ 180 MB | ≤ 120 MB |
| Catalog tile cache | ≤ 32 MB (LRU) | ≤ 16 MB |
| Initial download | ≤ 2.5 MB (wasm + js + font) | same |
| **Idle** | **0 scheduled frames/s, 0 timers, no GPU work** | same; screen-off friendly |

Platform notes I am committing to:
- **WebGL2 required, WebGL1 fallback path maintained.** GLES 3.0 is present on Android 8 and on 2015
  Intel, but those driver stacks are the buggiest in the world; a fallback shader set is not
  optional.
- Simulation runs in a **Web Worker**. `SharedArrayBuffer` when COOP/COEP headers allow; otherwise
  structured-clone snapshots at a low rate.
- **There is no game loop** (PERFORMANCE.md I3). An event queue and a presenter replace
  `while(true) { update(); draw(); }`. When nothing is pending, no frame is scheduled and the
  compositor holds the last surface.
- **Input latency is decoupled from frame rate** (PERFORMANCE.md I8). Audio runs at 48 kHz off the
  sim's continuous state, so an interaction is *heard* within ~8 ms even on a 30 fps frame. This is
  what makes the game feel responsive on a phone, and it is nearly free because all sound is
  synthesized from kernel numbers.
- **Idle claim, precisely:** `requestAnimationFrame` calls per second = 0; zero running timers; zero
  allocation; package power at the device's desktop-idle floor. Not "0 % CPU" — *nothing scheduled*,
  which is stronger and measurable.

---

## 14. Content strategy: systems, not levels

There is no level list, and this is deliberate — it is also the only way this ships.

| Content source | What it gives us | Why it doesn't run out |
| --- | --- | --- |
| Real catalogs (stars, exoplanets, bodies) | places, properties, uncertainty | 1.8 billion stars, ~6,000 exoplanets and growing |
| Real physics | every mission is a new optimisation problem | the space of designs is unbounded |
| Real economics | contracts, prices, competition | derived, not authored |
| Player authorship | vehicles, habitats, logic racks, procedures | the players make the content |
| Real mishap literature | anomaly missions | a finite but deep library, and it grows |

What we *do* author: the town, the people, the clipboard, the debrief's voice, the ending.

---

## 15. Metrics: what we measure, and what we refuse to

**Measure:**
- **Calibration improvement over time** (the real learning signal, and the only one I trust).
- Time-to-first-meaningful-consequence (< 5 min).
- Time-to-first-productive-failure (< 20 min).
- **Session end valence** (did it end warm? peak-end).
- Return-without-prompting (the anti-streak test: do they come back without being nagged?).
- Transfer test: performance on a *new* domain that shares the physics (fluids → thermal → structures).
- Qualitative: do players explain the physics to someone else? (self-explanation, and the protégé
  mechanic's real-world analogue).

**Refuse to optimise:**
- Daily active users. Session length. Ad-adjacent "engagement". Anything that would encourage us to
  make the game *longer* rather than *better*. If the numbers tempt us into a streak, the numbers
  are wrong.

---

## 16. Design risk register

| Risk | Severity | Mitigation |
| --- | --- | --- |
| Onboarding is too austere; players bounce in the first 4 minutes | **High** | Prototype the first 30 minutes *before* almost anything else. It is the product. |
| Productive failure feels like the game is broken | **High** | The competence mirror in every debrief; watch for the exact moment of churn and fix it there. |
| Zero-UI is unlearnable on a phone | **High** | Mobile-first interaction prototype at Block 1, not Block 8. If touch fails, mobile becomes "design and review only" and I will say so plainly. |
| Simulation depth is invisible to the player | Medium | Eidos exists precisely to make depth *visible*; if a system cannot be sensed, cut it or expose it. |
| Economy becomes a spreadsheet nobody wants to open | Medium | Money is always shown as consequence in the world (a bill, a creditor at the gate), never as a table. |
| Scope: this is a decade of work | **Fatal if unmanaged** | Block ladder (Section 17). Each block is playable, on `main`, and reviewable. |
| The vision outruns the hardware | Medium | Budgets in 13.4 are the contract. When a feature breaks them, the feature goes. |

---

## 17. The block ladder (how we will actually build this)

You described the workflow: **blocks, each a playable MVP, pushed to the branch, merged to `main`,
you `git pull` and play, then we iterate.** So each block must be *complete enough to judge*.

| Block | Codename | What you will be able to do | Why this block exists | Promises it proves |
| --- | --- | --- | --- | --- |
| **1** | *The Room* | Wake up. Walk. Close the curtain and watch the room cool and the AC's draw fall. Flip the visor. Watch the sky, the gobos, a distant plume with a late shockwave. | Prove the art direction, the cost model on your hardware, **and that the apartment is a spacecraft** (6.7). | P4, one-solver, zero-asset, render-on-demand, WebGL2+WASM pipeline |
| **2** | *The Clipboard* | Get to work. Catch the wrong checklist item. Fill, bleed, chill. Static fire, one real failure mode — and the wreck stays on the stand, examinable. | Prove Zero-UI input and productive failure. | P2, actuator-only verbs, the hand |
| **3** | *The Mark* | Yellow-tick two gauges before the fire. See the ghost twin with one arrow on the offending part. Export an Akashi page. | Prove the pedagogy machine and the artifact pipeline. | P1, P3, the Loop of Consequence end-to-end at 30 fires/hour |
| **4** | *The Sounding Rocket* | Author a small vehicle, fly it, recover it, launch one for a customer. | Prove authorship, flight, and the first money. | IKEA, the ladder begins, first Esse event |
| **5** | *The Drawer* | Persist anomalies across sessions; rescue a failed test; automate a procedure with the logic rack. | Prove failure design and the session architecture. | P5, peak-end, Zeigarnik budget |
| **6** | *The Orbit* | First orbital mission; light-time comms; the first real rendezvous. | Prove the astro kernel and the light-cone ledger. | Simulation depth, async presence |
| **7** | *The Museum* | See your career as a place. | Prove the archive. | Legacy, the ending |
| **N** | … | | | |

Blocks 1–3 are the ones that decide whether this game exists. Everything after is expansion of a
proven machine. If you want, I can start Block 1 immediately after your review of this document.

---

## Appendix A — mechanism → feature traceability

Every feature must trace to a mechanism. If a feature appears with no row here, it is decoration.

| Feature | Mechanism(s) | Section |
| --- | --- | --- |
| The clipboard with the wrong item | Productive failure; LPP | 2.2(b), 4.2 |
| Grease-pencil mark (hashed) | RPE; commitment; proper scoring | 6.1, 4.3 |
| Ghost twin diff + single arrow | Attribution without text; argmax sensitivity | 6.2 |
| Provenance node graph | Honesty; zero-cost uncertainty | 6.6 |
| The apartment's AC / bus / volume | One solver, two games | 6.7 |
| Golden hour at the epistemic peak | Peak–end; Zeigarnik, scheduled | 9.5 |
| Debrief attribution paragraph | RPE; self-explanation; curiosity gap closure | 6.2 |
| Competence mirror | Felt-learning paradox fix | 2.2(a) |
| Ghost Run | Counterfactual; desirable difficulty | 6.3 |
| Canonical gestures | Embodied cognition | 5.2 |
| Protégé | Learning by teaching; relatedness; self-explanation | 5.6 |
| Anomaly Drawer (max 5) | Zeigarnik + curiosity gap, budgeted | 7.3 |
| Designed ending / clock out | Peak–end; duration neglect | 9.2 |
| The Museum | Nostalgia; IKEA effect; legacy | 11.4 |
| Awe rationing | Overview effect; hedonic adaptation | 10 |
| Cramped first room | Contrast → awe; small self | P4, 10.4 |
| Visor flip (not a menu) | Autonomy; competence signalling | 13.1 |
| Help arrives as a person | Relatedness; anti-condescension | 13.2 |
| No streaks, no loot | SDT (over-rewarding undermines); ethics | 2.3 |
| Async causal mail | Relatedness without netcode | 11.2 |

---

## Appendix B — references

The ones I actually used to justify mechanics. Cited by short form above.

- Bjork, E. L., & Bjork, R. A. (2011). Making things hard on yourself, but in a good way: Creating
  desirable difficulties to enhance learning. In *Psychology and the Real World* (pp. 56–64).
- Brown, J. S., Collins, A., & Duguid, P. (1989). Situated cognition and the culture of learning.
  *Educational Researcher*, 18(1), 32–42.
- Chi, M. T. H., Bassok, M., Lewis, M. W., Reimann, P., & Glaser, R. (1989). Self-explanations: How
  students study and use examples in learning to solve problems. *Cognitive Science*, 13(2), 145–182.
- Deci, E. L., & Ryan, R. M. (1985). *Intrinsic Motivation and Self-Determination in Human Behavior*.
- Deslauriers, L., McCarty, L. S., Miller, K., Callaghan, K., & Kestin, G. (2019). Measuring actual
  learning versus feeling of learning in response to being actively engaged in the classroom.
  *PNAS*, 116(39), 19251–19257.
- Fiorella, L., & Mayer, R. E. (2013). The relative benefits of learning by teaching and teaching
  expectancy. *Contemporary Educational Psychology*, 38(4), 281–288.
- Goldin-Meadow, S., Nusbaum, H., Kelly, S. D., & Wagner, S. (2001). Explaining math: Gesturing
  lightens the load. *Psychological Science*, 12(6), 516–522.
- Kahneman, D., Fredrickson, B. L., Schreiber, C. A., & Redelmeier, D. A. (1993). When more pain is
  preferred to less: Adding a better end. *Psychological Science*, 4(6), 401–405.
- Kahneman, D., Knetsch, J. L., & Thaler, R. H. (1990). Experimental tests of the endowment effect
  and the Coase theorem. *Journal of Political Economy*, 98(6), 1325–1348.
- Kapur, M. (2008). Productive failure. *Cognition and Instruction*, 26(3), 379–424.
- Kapur, M. (2016). Examining productive failure, productive success, unproductive failure, and
  unproductive success in learning. *Educational Psychologist*, 51(2), 289–299.
- Kapur, M., & Rummel, N. (2012). Productive failure in learning from generation and invention
  activities. *Instructional Science*, 40(4), 645–650.
- Lave, J., & Wenger, E. (1991). *Situated Learning: Legitimate Peripheral Participation*. Cambridge.
- Loewenstein, G. (1994). The psychology of curiosity: A review and reinterpretation.
  *Psychological Bulletin*, 116(1), 75–98.
- Norton, M. I., Mochon, D., & Ariely, D. (2012). The IKEA effect: When labor leads to love.
  *Journal of Consumer Psychology*, 22(3), 453–460.
- Piff, P. K., Dietze, P., Feinberg, M., Stancato, D. M., & Keltner, D. (2015). Awe, the small self,
  and prosocial behavior. *Journal of Personality and Social Psychology*, 108(6), 883–899.
- Pouw, W. T. J. L., van Gog, T., & Paas, F. (2014). An embedded and embodied cognition review of
  instructional manipulatives. *Educational Psychology Review*, 26, 51–72.
- Redelmeier, D. A., & Kahneman, D. (1996). Patients' memories of painful medical treatments.
  *Pain*, 66(1), 3–8.
- Reid, C. A., Green, J. D., Wildschut, T., & Sedikides, C. (2015). Scent-evoked nostalgia.
  *Memory*, 23(2), 157–166.
- Roediger, H. L., & Karpicke, J. D. (2006). Test-enhanced learning. *Psychological Science*, 17(3),
  249–255.
- Ryan, R. M., Rigby, C. S., & Przybylski, A. (2006). The motivational pull of video games: A
  self-determination theory approach. *Motivation and Emotion*, 30(4), 344–360.
- Schultz, W., Dayan, P., & Montague, P. R. (1997). A neural substrate of prediction and reward.
  *Science*, 275(5306), 1593–1599.
- Sedikides, C., Wildschut, T., Arndt, J., & Routledge, C. (2008). Nostalgia: Past, present, and
  future. *Current Directions in Psychological Science*, 17(5), 304–307.
- Skulmowski, A., Pradel, S., Kühnert, T., Brunnett, G., & Rey, G. D. (2016). Embodied learning
  using a tangible user interface. *Computers & Education*, 92, 64–75.
- Soderstrom, N. C., & Bjork, R. A. (2015). Learning versus performance: An integrative review.
  *Perspectives on Psychological Science*, 10(2), 176–199.
- van Limpt-Broers, H., Postma, M., van Weelden, E., Pratesi, S., & Louwerse, M. (2024).
  Neurophysiological evidence for the overview effect: A virtual reality journey into space.
  *Virtual Reality*, 28.
- Wildschut, T., Sedikides, C., Arndt, J., & Routledge, C. (2006). Nostalgia: Content, triggers,
  functions. *Journal of Personality and Social Psychology*, 91(5), 975–993.
- Yaden, D. B., Iwry, J., Slack, K. J., Eichstaedt, J. C., Zhao, Y., Vaillant, G. E., & Newberg,
  A. B. (2016). The overview effect: Awe and self-transcendent experience in space flight.
  *Psychology of Consciousness*, 3(1), 1–11.
- Zeigarnik, B. (1927). On finished and unfinished tasks. *Psychologische Forschung*, 9, 1–85.

*(Interaction constants — Fitts's law, Hick's law, the 100 ms/1 s/10 s response limits, Doherty
threshold — are standard HCI results applied in Section 5.)*

---

## Appendix C — vocabulary

| Word | Language | Meaning in Odderon |
| --- | --- | --- |
| **Eidos** | εἶδος, Greek: form, species, the intelligible structure of a thing | The visor. The lens that shows *why* a number is that number. |
| **Akashi** | 証, Japanese: evidence, proof | The portfolio. Every design and mission becomes a verifiable technical report. |
| **Esse** | Latin: to be, to exist | The rating. Three numbers: calibration, Pareto contribution, anomaly resolution. |
| **Odderon** | physics: the C-odd partner of the Pomeron, still unmeasured | The game. The odd one. The trajectory nobody has closed yet. |
| **The clipboard** | — | Day 1. Legitimacy. The beginning of belonging. |
| **The drawer** | — | At most five open anomalies. The hook tomorrow. |
| **The mirror** | — | The debrief that tells you, numerically, how much better you got. |

---

## Decisions (closed)

| # | Question | Decision | Where it landed |
| --- | --- | --- | --- |
| 1 | Is the Loop of Consequence the right spine? | **Yes.** The grease pencil, the ghost twin, the provenance graph, the one-solver apartment and the epistemic peak are all integrated, and each one *reduces* the code we write. | §0, §6, §9.5 |
| 2 | The refusals in §2.3 | **Kept as written.** No streaks, no variable-ratio rewards, nothing purchasable touches Esse, no fake difficulty, no punishment for absence, no lying about uncertainty, no cutscene awe, no "easy mode", no score inflation. | §2.3 |
| 3 | Block ladder | **Block 1 = *The Room***, exactly as specified. The previous build was abandoned and is being rebuilt from scratch; nothing in this repository predates the rebuild. | §17 |
| 4 | Esse | **Three numbers**: calibration (proper scoring), Pareto contribution, blind anomaly resolution. Never one. | §4.3 |
| 5 | Protégé and Museum | **Both in.** Protégé at Block 5; Museum at Block 7, with its foundation laid at Block 1 — every wreck is recorded the moment it happens, because a museum cannot be retro-fitted. | §11.4 |

### One tacit rule, made explicit: the ladder is not a ladder

The career is **not** a single ordered climb, and saying so out loud matters because a rule that
lives only in the author's head becomes an invisible bug.

- You **can resign.** Leaving a company is an action, not a menu: you hand in the clipboard, and the
  rest of the town keeps its opinion of you.
- You **can be hired laterally.** Another company may take you at a rank *above* where you left,
  because the thing they are buying is your Esse and your Akashi, not your seniority. A player who
  spends four years as a pad technician at a struggling sounding-rocket outfit, then moves to a
  competitor as a **junior engineer with a better title but a smaller budget**, has genuinely
  advanced and genuinely regressed, and the game must represent both.
- The **protégé is a consequence of role, not of time.** You get a junior when you hold a role that
  has one, wherever you are. Resign and you lose the protégé; be hired above them and you inherit
  someone else's.
- Companies therefore have **different ladders**, and a career is a path through several of them —
  which is also the only honest model of how aerospace engineers actually move.

This is recorded in §4.2 as **lateral movement** and it is load-bearing for the economy: it is what
makes reputation portable, which is what makes Akashi worth exporting, which is the whole product.
