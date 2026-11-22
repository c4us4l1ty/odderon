# Odderon — Resolved Contradictions

**The single register of contradictions that have been closed, and of which document is authoritative
for what.** Written because a design document set this large will always develop two answers to the
same question, and the only defence is one page that says which answer won and why.

Rule: when two documents disagree after this page was written, **the authority column decides**, and
the losing document gets corrected in the same commit. Do not add a reconciliation paragraph — delete
the loser.

---

## 1. Authority by topic

| Topic | Authority | Superseded / not authoritative |
| --- | --- | --- |
| The vision, philosophy, non-negotiables | `IMPORTANT-1.md`, `IMPORTANT-2.md` | — |
| Thesis, architecture, crate layout, determinism, data pipeline | `VISION.md` | old milestone ladder (deleted, see D2) |
| Gameplay, pedagogy, psychology, career, economy, block ladder | `GAME-DESIGN-BIBLE.md` | — |
| **Rendering, frame cost, GPU strategy, R1–R3 tests, memory envelope** | **`GRAPHICS.md`** | pixel-pipeline content of `PERFORMANCE.md` (see D3) |
| The look: light ratios, palette, histogram targets, geometry budget | `ART-DIRECTION.md` | — |
| Platform doctrine: no game loop, events, audio latency, allocation | `PERFORMANCE.md` | its old frame ledger (deleted, see D3) |
| No loading screens, boot, hitch budget, descent | `IMMERSION.md` | — |
| V&V, export formats, Proof-of-Flight, HIL, credibility rungs | `CREDIBILITY.md` | — |
| **Hardware targets** | `GRAPHICS.md` §1 and bible §13.4 (must agree) | — |
| **What to build next, in what order** | bible §17 (block ladder) | — |

---

## 2. Closed contradictions

### D1 — The documents described a codebase that does not exist

`ART-DIRECTION.md` was a precise post-mortem of a renderer — file paths, commit hashes, per-building
triangle counts, a kit-versus-hand-authored cost table, scripted capture tooling — and the bible's
decisions table asserted *"Block 1 … is built: `index.html`"*. None of it is in the repository.

**Resolved:** the earlier attempt failed and the project restarts from this commit. Residue has been
removed from the documents rather than preserved as history:

- `ART-DIRECTION.md` keeps its **doctrine** (light ratios, value structure, palette, which
  deliberate-looking-wrong things are deliberate, the histogram method) and drops its measurements to
  the status of "previous build, not evidence about the current one".
- The bible's decisions table no longer claims a build exists.
- Named scripts (`server.mjs`, `screenshot.mjs`, `art-compare.mjs`), paths (`crates/…`, `tools/…`,
  `.artifacts/…`) and a short hash were removed or re-framed as **Block 1 deliverables**.
- The art-direction measurements that survive as *judgement* (the fill-light calibrations, the "a big
  shaded-vertical bounce flattens the scene" lesson, the noise-weight ratios) are kept, because they are
  cheap and they are the kind of thing that costs a week to re-learn. They are labelled as doctrine.

**Standing rule:** a document may not describe a file as existing. If it is not in `git ls-files`, it is
a plan, and it must be written as a plan.

### D2 — Two ladders (VISION M1–M4 vs. bible Block 1–7)

`VISION.md` §8 proposed a milestone ladder "The Room / The Pad / The Sounding Rocket / The Orbit" while
`GAME-DESIGN-BIBLE.md` §17 specified Blocks 1–7 and the decisions table closed on it. Two ladders is
how a project ends up with a Block 4 that contradicts an M3.

**Decision as asked — the best way to build Odderon:**

> **Keep the block ladder. Delete the milestone ladder.**

Reasons, in order of weight:

1. **The block ladder encodes *promises*, the milestone ladder encoded *subsystems*.** A block must be
   playable and must prove several pillars at ~1 % scope. A milestone proves one thing deeply. The
   subsystem-shaped ladder is exactly the failure mode the bible names as fatal ("building one
   subsystem perfectly for two years"), so the ladder that avoids it is the one to keep.
2. **The block ladder ends at the thing that makes the project unique.** Block 7 *The Museum* and the
   archive are the once-in-a-generation idea; the milestone ladder stopped at "The Orbit", i.e. it
   stopped before the differentiator.
3. **One of them had to produce a reviewable artifact per step.** Blocks are judged by playing them,
   on `main`, with benchmark numbers — and `CREDIBILITY.md` rung 1 requires precisely that kind of
   shipping cadence.
4. The M1–M4 content was not wasted: it was re-expressed as the block ladder's first six blocks, which
   cover the same ground in a better order (the static fire and the grease mark arrive before the
   sounding rocket, not after).

`VISION.md` §8 now states the single ladder and points at bible §17. **Nothing else may propose a
second ladder.**

### D3 — Two frame budgets (PERFORMANCE 1080p60/11 ms vs. GRAPHICS analytical APR)

`PERFORMANCE.md` carried a full GFLOP/s ledger and an "1080p at 60 fps, ≤ 11 ms worst case" contract
derived from a forward/deferred renderer, plus a "Round 1 → Round 2" benchmark history from the
abandoned build. `GRAPHICS.md` specifies a different pixel paradigm whose tests are stated in different
units entirely (bandwidth per pixel, static-camera frame time, zero shadow maps).

**Decision as asked — `GRAPHICS.md` is the newer, correct document:**

- `GRAPHICS.md` is **authoritative for all pixel and frame-cost claims.**
- `PERFORMANCE.md` was **rewritten**, not patched. Its frame ledger, its resolution contract, its
  benchmark history and its superseded innovations were **deleted**. What remains is the platform
  doctrine that is renderer-independent: decoupled sampling (I1), no game loop (I2), audio carries
  temporal resolution (I3), determinism enables skipping work (I4), zero allocation (I5) — plus two
  measurable claims (idle = nothing scheduled, input→audio ≤ 8 ms) that survive any renderer.
- `GAME-DESIGN-BIBLE.md` §13.4 had been kept in sync by editing from `PERFORMANCE.md`; it now points at
  `GRAPHICS.md` as primary and no longer asserts a frame-budget number that no document defines.
- The 1080p claim was **not** carried forward as a promise. `GRAPHICS.md` sets out to *exceed* that
  quality by other means; whether it does is R2's job to answer on real silicon.

**One lesson from the old build is kept deliberately** (recorded in `PERFORMANCE.md` §3): an earlier
governor "dropped resolution below 26 fps" and never fired, because it measured the wrong quantity.
*Any budget not expressed in the units the platform charges you in is decoration.* Hence GRAPHICS'
tests are in bytes-per-pixel, milliseconds of static-camera frame time, and draw calls counted at
runtime.

### D4 — Two mobile SoCs (Snapdragon 625 vs. Kirin 710 / Mali-G51)

The bible named a Snapdragon 625; `GRAPHICS.md` builds its entire Path A / Path B split on the Mali-G51
in the Kirin 710, and the two GPUs have different bandwidth and different tile behaviour.

**Decision as asked — Kirin 710 (Mali-G51 MP4) is the actual device.** Corrected in bible §13.4 and in the
reference-B definition at the head of the bible. Reference B is now *Android 8+, Kirin 710, 2–3 GB RAM, Chrome*
everywhere it appears.

### D5 — `GRAPHICS.md` was stored as raw pasted output

The file arrived as a paste: a literal `text` code fence, mashed strip-mine duplications
(`16×1616×16`, `0.53∘0.53∘`, `Fi→jFi→j​`), a dangling "here are five insanely novel innovations" with no
orientation, and no statement of standing relative to the other documents.

**Resolved:** rewritten as a proper specification with a status line, an authority claim, defined
reference hardware, section numbering that other docs can cite, and an explicit "this is a hypothesis
with tests" framing. **Content was not changed or softened** — the five innovations, the two hardware
paths, the memory envelope and R1–R3 are preserved. What was added is standing, precision, and the
honest caveat that none of it is measured yet.

### D6 — Editorial residue

- Duplicated `### 6.3 The Ghost Run` heading — removed.
- Duplicated sentence in `IMPORTANT-1.md` (lines 7–8, "A poorly designed rocket fails to reach orbit")
  — removed.
- `VISION.md` cited `PERFORMANCE.md` for renderer numbers — now cites `GRAPHICS.md`.
- `IMMERSION.md` cited "bible §13.3" for matte descent — now cites `VISION.md` §5.4.
- `GAME-DESIGN-BIBLE.md` §5.2 marked two canonical gestures as "M2" — now "Block 2".

---

## 3. Contradictions left open on purpose

Recorded so they are not mistaken for oversights.

| Tension | Why it is not resolved yet | What will resolve it |
| --- | --- | --- |
| `GRAPHICS.md` claims vs. measured reality | Nothing has been rendered. Resolving this by argument would produce exactly the false confidence that killed the last build. | R1/R2/R3 on Reference A and B, published with the misses. |
| Bigger route: is the world one octree of catalogues, or one kernel with several? | Cheap to defer; expensive to get wrong late. | Block 1 decides the frame hierarchy; Block 6 decides catalogue streaming. |
| Mobile fidelity | A zero-UI first-person sim on a 2017 mid-range phone is the riskiest requirement in the set. | The bible's pre-mortem already sets the pivot: mobile becomes design-and-review only. Say so publicly if it happens. |
| Where the player's *first* moment of awe is | The doctrine says "never announce it" and "ration by act", which is deliberately under-specified. | Playtesting. It cannot be designed on paper. |

---

## 4. Adding to this page

A new entry needs three things: **what disagreed**, **which document won**, and **what was deleted or
changed as a result**. An entry without a deletion is a compromise, and compromises are how a doc set
starts disagreeing with itself again.
