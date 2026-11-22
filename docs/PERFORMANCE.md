# Odderon — Platform Performance Doctrine

**What this file is, and what it is not.**

`GRAPHICS.md` is the authoritative specification of how Odderon produces pixels, what the two target
GPUs are, and what a frame costs on each. Read that first; it is the newer document and it governs.

This file holds the *platform-level* doctrine that sits underneath the renderer and does not change
when the pixel pipeline changes: that there is no game loop, that the universe is a query rather than
a tick, that time is spent only where something changed, and that audio carries the temporal
resolution when the frame rate cannot. Those commitments are what make a browser simulator on a 2015
laptop possible, and they are independent of whether a pixel is shaded by a forward pass, a deferred
pass, or an analytical reconstruction.

`PERFORMANCE.md` previously carried its own frame ledger, its own benchmark history, and its own
resolution targets. That ledger has been **removed** rather than reconciled, because two documents
claiming to define the frame budget is how a project ends up optimising against a number nobody
measures. Numbers now live in `GRAPHICS.md`; the discard is recorded at the end of this file so the
decision is auditable rather than quietly forgotten.

Companion docs: [`GRAPHICS.md`](GRAPHICS.md) (renderer, authoritative),
[`VISION.md`](VISION.md), [`GAME-DESIGN-BIBLE.md`](GAME-DESIGN-BIBLE.md), [`CREDIBILITY.md`](CREDIBILITY.md),
[`IMMERSION.md`](IMMERSION.md).

---

## 0. The two claims that are not frame-budget claims

Two things in the original doctrine look like rendering claims but are not, and they survive any
renderer:

1. **Idle is free because nothing is scheduled.** Not "0 % CPU" — *nothing scheduled*. This is a
   property of the control flow (§I3), not of the shader.
2. **Latency is decoupled from frame rate.** A valve turn is *heard* within a few milliseconds even
   if the next frame is 30 ms away (§I8), because audio runs off the kernel's continuous state. This
   is a property of the audio path, not of the rasteriser.

Everything else about pixels belongs to `GRAPHICS.md`.

---

## 1. The platform doctrine

Each item is a *design* commitment, not a later optimisation, and each one is a reason the renderer
has less work to do.

### I1 — Decoupled sampling: the frame is a sampler, not a tick

The core inversion. Today's engines do `tick sim → draw frame → tick sim → draw frame`. Odderon:

```
        sim (event-driven, sparse, deterministic)
              │  emits timestamped state deltas
              ▼
       state ring buffer (immutable, indexed by sim-time)
              │
       ┌──────┴───────┐
       ▼              ▼
   RENDERER       AUDIO THREAD (48 kHz, continuous)
   (opportunistic sampler — runs 0, 12, 30, 60, 120 Hz as needed)
```

The renderer samples the ring buffer at *whatever rate it wants*. The sim does not care. Consequences:

- Rendering at 30 Hz while the sim runs at 200 Hz costs nothing extra.
- **Rendering at 0 Hz is legal.** The last sample stays on screen — the "frozen painting" the vision
  asked for, as an architectural property rather than a hack.
- **Interpolation is exact, not a guess.** Between two sim samples we evaluate the closed-form
  propagation. There is no "interpolated position" error, because the object *is* a function of time.

**Novel bit:** because interpolation is analytic rather than linear, we can sample *arbitrary
sub-tick moments* — slow motion, high-speed shutter, and motion blur with no ghosting (I6).

### I2 — The deviceless frame: there is no game loop

There is no `while(true) { update(); draw(); }`. There is an **event queue** and a **presenter**.

```rust
// pseudo-Rust, the actual control flow
loop {
    let ev = next_event_or_idle();            // blocks; no spin
    match ev {
        Input(a)      => sim.actuate(a),
        Signal(s)     => sim.deliver(s),       // light-cone arrival
        Tick(t)       => sim.advance_to(t),    // only if something is due
        Idle          => { present_if_dirty(); park(); }   // <-- the important branch
    }
}
```

`park()` means: **do not schedule the next animation frame.** The compositor holds the last surface.
This is why idle is genuinely free, and why the battery survives on a phone.

**The honest definition, replacing "0 % CPU":**

> **Odderon's steady state is: no scheduled frames, no timers running, no allocation, no GPU work.**
> Measured as: `requestAnimationFrame` count per second = 0; package power within noise of the
> device's desktop-idle floor; the process does nothing until an event arrives.

That is a stronger and more honest claim than "0 %", and it is *measurable*.

### I3 — Audio carries the temporal resolution

The single most under-used insight available to us, and it costs almost nothing.

Perceptual motion continuity comes from **the whole sensorium**, not just pixels. A game can present at
30 Hz and still feel instant *if the other channels are fast*:

- **Audio runs on its own clock at 48 kHz**, fed by the sim's continuous state — always faster than any
  frame rate.
- A valve turn produces its *sound* in the next audio buffer (~5 ms), even if the next frame is 30 ms
  away. The interaction feels instantaneous because the ear's temporal resolution is ~2 ms and the
  eye's is ~13 ms.
- Haptics (phone vibration, if present) fire from the same path.
- The gauge needle's *motion* can be interpolated in the compositor; its *click* is immediate.

So **input latency is decoupled from frame rate.** That is a genuine, measurable innovation, and it is
free — we already synthesize all sound from kernel numbers.

**Corollary:** in an emergency, we can drop to 12 fps and the game still *feels* responsive.

### I4 — Determinism is a performance feature

Because the sim is deterministic and closed-form, we can *prove* an object is unchanged: zero state
delta plus a sub-threshold screen-space derivative means its pixels cannot have changed. The renderer
in `GRAPHICS.md` uses exactly this to skip work (the "living still" cache / transaction elimination).

An engine that cannot prove its state is unchanged must redraw. Ours can, and that is why it can stop.

### I5 — Zero allocation in the frame path

A garbage-collection pause is a loading screen's ghost: a 4 ms GC pause is a visible stutter with no
cause the player can name. All buffers are pooled and pre-sized, and the frame path allocates nothing.
See `IMMERSION.md` N4.

---

## 2. Three platform-level claims, and how they are falsified

| Claim | Number | How you falsify it |
| --- | --- | --- |
| Idle is genuinely free | 0 scheduled frames/s; 0 timers; package power at idle floor | Task manager + a power meter, hands off the keyboard |
| Latency is frame-independent | ≤ 8 ms input → audio | Log timestamps in a dev build |
| The world never stops for a transition | 0 shader compiles after boot; 0 synchronous GPU stalls in the loop | `IMMERSION.md` N9 table, gated in CI |

Frame time, resolution, bandwidth, draw calls and the R1/R2/R3 tests are **not** in this file. They are
in `GRAPHICS.md` and they are gated there.

---

## 3. What was removed, and why

Recorded so the change is auditable. Everything below was deleted from this file in the rebuild:

| Removed | Why |
| --- | --- |
| The GFLOP/s frame ledger and the "1080p60 at ≤ 11 ms" contract | It described a forward/deferred renderer that is no longer the design. `GRAPHICS.md` specifies analytical reconstruction instead. |
| The "Round 1 → Round 2" benchmark history (11 fps at 1880×959, the DPR governor, the march-length cuts) | It was a post-mortem of an earlier build that has been abandoned. Its engineering lessons are preserved in `GRAPHICS.md`'s R1–R3 tests and in §4 below; its *numbers* are not evidence about the new renderer. |
| I2 progressive refinement as a foundation, I5 the slot machine, I7 the sky as a backdrop, I9 perceptual foveation, I10 speculative synthesis, I11 painterly-as-cheap | These are still good ideas, but they are now either subsumed by `GRAPHICS.md` (analytical box filtering replaces the reprojection pipeline; the analytical occluder list replaces the gobo stack; SH radiosity replaces the deferred light chain) or no longer load-bearing. `GRAPHICS.md` is the single source of truth for the pixel pipeline. |
| The 1080p-output-row claim table | Superseded; the resolution contract lives in `GRAPHICS.md`. |
| "Bible §13.4 says X, so change it to Y" change-log entries | The bible has been updated directly. A doc that tracks its own edits is a diary, not a spec. |

**One lesson from the abandoned build is kept deliberately**, because it is the kind of thing that is
learned once and painfully: an earlier version of this engine reported a governor that "dropped
resolution below 26 fps", and it never fired, because it measured the wrong quantity. *Any budget that
is not measured in the units the platform charges you in is a decoration.* Every gate in `GRAPHICS.md`
is therefore expressed in measurable quantities — bytes per pixel, milliseconds of frame time, draw
calls counted at runtime — and each one has a named test that fails the build.

---

## 4. Why this ordering is right

The temptation, with a renderer as ambitious as `GRAPHICS.md`'s, is to build the clever path first: the
analytical reconstruction, the tile scheduler, the SH radiosity. The doctrine says the opposite, and it
is the same doctrine the game teaches:

> **Measure the simple architecture first, then add cleverness only where the measurement demands it.**

Concretely, for each block: ship the straightforward path, benchmark it on Reference A and Reference B
(`GRAPHICS.md` §1), publish the number including the misses, and only then reach for the next
technique. If the simple path already holds 60 fps on a static camera and a bounded frame while moving,
the clever path is optional. If it does not, the measurement tells us which lever to pull — which is
exactly what the last version failed to do, and why it spent its effort on the wrong one.
