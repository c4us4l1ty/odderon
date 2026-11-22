# Odderon — documentation index

Read in this order. Each link says what the document is *for* and what it is authoritative over.

| # | Document | What it is | Authority |
| --- | --- | --- | --- |
| 1 | [`IMPORTANT-1.md`](../IMPORTANT-1.md) | The original vision, in full: hard physics, real universe, zero UI, Eidos/Akashi/Esse, open-ended sandbox. | **The non-negotiables.** Nothing may contradict these. |
| 2 | [`IMPORTANT-2.md`](../IMPORTANT-2.md) | The second brief: the "Shadows of a Summer Noon" art direction, the nobody-on-Earth start, the four life paths. | as above |
| 3 | [`VISION.md`](VISION.md) | The thesis, the flywheel, the eight laws and what enforces each one, crate layout, determinism, data pipeline, the honest tension table. | Architecture. |
| 4 | [`GAME-DESIGN-BIBLE.md`](GAME-DESIGN-BIBLE.md) | The playable design: the Loop of Consequence, five pillars with falsifiable tests, the pedagogy machine, failure design, onboarding minute by minute, session architecture, progression without XP, Esse/Akashi/Eidos, **the block ladder (§17)**. | Gameplay and build order. |
| 5 | [`GRAPHICS.md`](GRAPHICS.md) | The renderer: Analytical Perceptual Reconstruction, the two target GPUs, the five innovations, the memory envelope, and the R1–R3 tests. | **All rendering and frame cost.** |
| 6 | [`ART-DIRECTION.md`](ART-DIRECTION.md) | The look and how it is calibrated: art sun, box occluders, fill light, histogram targets, deliberate-looking-wrong choices. | The visual target. |
| 7 | [`PERFORMANCE.md`](PERFORMANCE.md) | Platform doctrine that survives any renderer: no game loop, events not ticks, audio carries latency, zero allocation, idle = nothing scheduled. | Platform behaviour. |
| 8 | [`IMMERSION.md`](IMMERSION.md) | Why there are no loading screens, the diegetic boot, the 8.6-order descent, the nine rules, the hitch budget. | Transitions. |
| 9 | [`CREDIBILITY.md`](CREDIBILITY.md) | How the claims get earned: the rung ladder, the V&V suite with tolerances, the export bridge, Proof-of-Flight, HIL, the honest limits. | Claims and evidence. |
| 10 | [`DATA-LICENSING.md`](DATA-LICENSING.md) | Which public datasets may ship in a commercial build, which may not, the per-file manifest, the two CI gates, and why catalogue uncertainty is a game mechanic. | **Data rights — a shipping gate.** |
| 11 | [`BUILD-PLAN.md`](BUILD-PLAN.md) | How the work is actually done: the three layers, the toolchain, the build→instrument→measure→correct loop, what "Block 1 done" means as five measurable claims, and the risks specific to the way we build. | **Process.** |
| 12 | [`DETERMINISM.md`](DETERMINISM.md) | The six determinism rules and how CI enforces each; the golden hash; and the **CI-as-compiler** loop that exists because the dev sandbox has no Rust. | **Correctness of replay + how Rust gets built.** |

## Which document wins

When two documents disagree, [`DECISIONS.md`](DECISIONS.md) decides. It holds the authority table and a
register of every contradiction that has been closed, with what was deleted as a result. It is short,
it is binding, and it is the first place to look before changing a number that appears twice.

## Where the project stands

The previous attempt failed and is not represented in this repository. **The project restarts from
here.** Documents were written against that earlier build and carried residue from it — file paths,
benchmark history, a cost table, a claim that a build existed. That residue has been removed:
measurements that belonged to the abandoned build are labelled as doctrine or deleted, and anything
described as a file is a **plan** until it is in `git ls-files`.

Nothing in `docs/` describes code that currently exists. The next deliverable is **Block 1, *The
Room*** (bible §17): a playable room, zero binary assets, render-on-demand, and — the only number that
matters first — a **measured** frame cost on Reference A and Reference B, published with the misses.

## House rules for these documents

1. **No document describes a file as existing.** If it is not tracked, write it as a plan.
2. **No number without a unit and a test.** A budget not expressed in the units the hardware charges
   in is decoration.
3. **No second source of truth.** Two documents may not define the same number. The authority table
   decides, and the loser is corrected in the same commit.
4. **A claim above its evidence is retracted publicly.** `CREDIBILITY.md` §1 forbids speaking rung-6
   language from rung 1, and that applies to the design documents too.
