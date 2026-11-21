# Odderon

A browser simulator of real aerospace engineering — a warm, sunlit world built from pure
mathematics, where physics is an advantage rather than an obstacle.

Runs on hardware most people already own (2015-era integrated GPU, 4 GB RAM, Android 8+).
No loading screens — not short ones, none.
Zero 3D asset files. **60 fps at 1080p output on a 2015 integrated GPU, enforced as a CI gate** —
and nothing scheduled at all when idle.

## Documents

| Document | What it is |
| --- | --- |
| [`docs/VISION.md`](docs/VISION.md) | The thesis: what Odderon is, the eight laws and how each is *enforced in code*, where the promises fight each other, architecture strawman, milestone ladder |
| [`docs/GAME-DESIGN-BIBLE.md`](docs/GAME-DESIGN-BIBLE.md) | The design: the Loop of Consequence, 13 researched psychological mechanisms, pillars with falsifiable tests, loops, progression, interaction grammar, onboarding minute-by-minute, mobile, session architecture, block ladder |
| [`docs/PERFORMANCE.md`](docs/PERFORMANCE.md) | The performance doctrine: measured millisecond ledger, the nine techniques (decoupled sampling, progressive refinement, no game loop, delta rendering, ≤ 8 draw calls, exact analytic motion blur, painterly sky, audio-carried latency, perceptual foveation), and the CI-enforced frame budget |
| [`docs/IMMERSION.md`](docs/IMMERSION.md) | The no-loading-screen doctrine: why loading screens are an artefact of asset pipelines we don't have, the 8.6-order-of-magnitude planetary descent, the nine rules, the diegetic boot, and the CI-gated hitch budget |
| [`docs/CREDIBILITY.md`](docs/CREDIBILITY.md) | The evidence plan: claim ladder, public validation suite with stated tolerances, export bridge (GMAT / CEA / SPICE / CCSDS), Proof-of-Flight replay verification, HIL, adoption ladder, pre-mortem, honest limits |

## Original design notes

| File | Contents |
| --- | --- |
| [`IMPORTANT-1.md`](IMPORTANT-1.md) | The founding vision: real physics, real universe, Eidos / Akashi / Esse, zero-UI, zero-asset, mega-structures |
| [`IMPORTANT-2.md`](IMPORTANT-2.md) | "Shadows of a Summer Noon": the art direction and the "Nobody on Earth" opening |

## Status

Block 0 (design + credibility docs) — **awaiting review**. Code begins at Block 1, *The Room*.
