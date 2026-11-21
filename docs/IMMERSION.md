# Odderon — The No-Loading-Screen Doctrine

**Block 0 addition.** Companion to `VISION.md`, `GAME-DESIGN-BIBLE.md`, `PERFORMANCE.md`,
`CREDIBILITY.md`.

> There are no loading screens. Not because we hid them well — because the architecture never
> produces anything to wait for.

A loading screen is not a rendering problem. It is an artefact of five specific engineering
decisions, and we declined all five in the vision long before this became a requirement.

---

## 0. Why loading screens exist

| Cause | What a normal game does | What causes it |
| --- | --- | --- |
| **Asset residency** | Load a level's textures, meshes, audio into VRAM | Hundreds of MB of pre-made files |
| **Shader compilation** | Compile the level's shader permutations | Driver compiles GLSL at first use: **18 ms** per permutation on 2015 hardware |
| **Scene graph construction** | Instantiate objects, build spatial indices | Gameplay code creates the world at load time |
| **Synchronous I/O** | Block the main thread on the filesystem or network | The engine was never written to be async |
| **Mode switching** | Tear down one world, build another | Planet, orbit, and interior are different "levels" |

Our situation, structurally:

| Cause | Odderon | Why |
| --- | --- | --- |
| Asset residency | **Nothing to load** | Every asset is a function of parameters. There is no file to fetch for a nozzle, a wall, a rock, or a planet. |
| Shader compilation | **Eliminable** | A small, fixed, pre-warmed shader set — compiled during the boot fade, never on transition |
| Scene graph construction | **A query, not a build** | Geometry is generated for what is *observed*, at the resolution the eye requires, continuously |
| Synchronous I/O | **Never blocks** | Catalog data arrives by async range request; missing tiles degrade fidelity, they never stall the frame |
| Mode switching | **There is only one world** | Orbit, atmosphere, and surface are the same kernel at different scales — the atmosphere is a shader that fades in, not a level that loads |

**So the absence of loading screens is not a feature we add. It is a property we would have to work
to lose.**

---

## 1. The descent problem, stated honestly

Entering a planet is where every engine in existence covers the screen with a spinner, because it
spans **8.6 orders of magnitude continuously**:

```
  400 km   orbital          1 px ≈ 2 km     stars, curvature, terminator
  120 km   entry interface  1 px ≈ 600 m    plasma sheath begins to ionise
   70 km   peak heating     1 px ≈ 350 m    comm blackout window
   40 km   hypersonic       1 px ≈ 200 m    aero control authority returns
   10 km   subsonic         1 px ≈ 50 m     recognisable terrain, landmarks
    1 km   terminal         1 px ≈ 5 m      structures, plume gouge, blast berm
   10 m   touchdown         1 px ≈ 5 cm     rocks, strut compression
    1 mm   boot              —              regolith grains, dust on the lens
```

Three mechanisms make it continuous:

1. **Hierarchical local frames** (`VISION.md` §5.1). The camera never sees an absolute coordinate.
   Depth precision is a property of a *local window*, not of the planet, so 24-bit depth is never the
   limiter — the "floating-point kraken" cannot produce z-fighting because it never gets the chance.
2. **Matte descent** (bible §13.3). Distant terrain is an analytic parallax layer that resolves into
   parametric geometry as the eye approaches. The *same* mechanism that makes a 2015 laptop render a
   coastline is what makes the descent seamless: there is no moment of exchange, only a continuous
   increase in the resolution of an answer that was always being computed.
3. **Detail as a function of observation, not of arrival.** Nothing "arrives". There is no boundary
   at 10 km where a new dataset is swapped in. There is a derivative: as the camera closes, the
   parametric terrain subdivides, in the background, budgeted, invisible.

---

## 2. The nine rules

### N1 — The boot is diegetic, and it is the wake-up
The only loader in the game is the first one, and the player must never perceive it as a loader.

Choreography:
```
t=0.0s   black. a fan. cicadas. (audio starts immediately — it needs no world)
t=0.4s   the room fades up from black — walls, window, light bars. 400 KB core module is enough
         to draw this, because the room is a shader, not a simulation.
t=1.2s   physics + catalog modules stream in *behind the fade*, while the player is still in bed
t=2.5s   eyes open. You can move.
```

**The innovation:** *the first scene requires no simulation.* The room is pure fragment shader —
geometry, light, dust, gobos. So the most beautiful thing in the game can be on screen before a single
line of astrodynamics exists. **The load bar is a sunrise, and it is real, not a placeholder.**

On a 3G connection this still works: a 400 KB core renders the room; the rest arrives while you are
lying down, which is the only moment in the game where waiting is *in character*.

### N2 — Compile every shader at boot; compile none at a transition
Shader compilation is the single most common cause of an "invisible" loading hitch: 18 ms is a full
frame drop at 60 Hz, and it always happens exactly at the moment of a visual transition.

Rule: the shader set is **fixed and small** (the slot-machine architecture, PERFORMANCE.md I5, means
one shader covers a wall, a pipe, and a valve). All permutations are compiled during the boot fade and
during idle. **There is no code path that compiles a shader in response to gameplay.**

### N3 — Never block on I/O; degrade instead
No synchronous loads. Ever. If a catalog tile for the star field has not arrived, the sky renders the
stars we have and **adds the rest as they arrive**, silently. The universe is never late; it is
occasionally blurry, and only in the part of the sky the player was not looking at yet.

Same for terrain: a missing tile renders at the parent tile's resolution. A missing material database
falls back to a declared generic with its uncertainty shown (the `Provenance` type does not permit
inventing a number because a file is late).

### N4 — Zero allocation in the frame loop
A garbage-collection pause is a loading screen's ghost: a 4 ms GC pause is a visible stutter with no
cause the player can name. The frame loop allocates nothing. All buffers are pooled and pre-sized.
This is enforced by a CI check on the frame function, not by discipline.

### N5 — There are no mode transitions, only continuations
Entering a ship, leaving a planet, going from atmosphere to vacuum: none of these are scene changes.
- The atmosphere is a shader term that goes to zero with altitude.
- Docking is an approach manoeuvre you fly.
- Entering a building is a door.
- Planetfall is **a long fall** — the most beautiful sequence the game has, and it is not a cutscene;
  the player is flying it, and the matte resolves around them while they do.

### N6 — The world keeps simulating through every transition
Because there is no level to tear down, the world never stops. During the fade, during the door, during
the fall: the company's clock runs, signals are still crossing the solar system, the anomaly drawer is
still open. A transition is a change in *what you are looking at*, never a suspension of reality.

### N7 — Time compression is diegetic, and has no screen
The 10⁶× compression for interplanetary cruises is entered by climbing into a bunk or a cryo pod and
**closing your eyes**. The compression is not a UI; it is the player choosing to stop observing. On
arrival the eyes open. No spinner at 10⁶×, because the game does not render a "fast" animation — it
evaluates the analytic state at a later time, which costs the same as evaluating it at an earlier one.

### N8 — Arrival is never a teleport
A common hidden loader is the "travel to destination" trivially replaced by a loading screen. We refuse
it: travelling costs the time physics says it costs, and if the player does not want to wait, they
either fly something faster or sleep through it (N7). **The absence of a loading screen is also a
commitment to the absence of teleportation.**

### N9 — The hitch budget is a test
Instrumented in the benchmark build and gated in CI:

| Metric | Requirement |
| --- | --- |
| Frame time, p99 (interactive, non-transition) | ≤ 20 ms |
| Frames > 33 ms during gameplay | **0** |
| Frames > 33 ms during a diegetic fade/descent | ≤ 2, and never at a cut |
| Synchronous GPU stalls (`readPixels`, `finish`, `getError` in loop) | **0** |
| Shader compiles after boot | **0** |
| Allocations in the frame function | **0** |
| `await`/`fetch` on the main thread in the frame loop | **0** |

A build that violates any row fails. This is how "no loading screens" stops being a slogan and becomes
a property we can lose.

---

## 3. Where the seams would still show, and the answer

| Seam | Why it's hard | Answer |
| --- | --- | --- |
| **First load on a slow connection** | 2.5 MB of wasm is a wait | The diegetic boot (N1): 400 KB core renders the room; the rest streams behind the fade, in character |
| **Terrain detail pop** | New geometry appearing is visible | Subdivision is budgeted per frame and prioritised by angular size; new detail fades in behind the matte layer that was masking it |
| **Star tile refinement** | Stars appear as data arrives | Stars render from the parent tile; refinement is a *brightness and position* correction of an already-present star, not a new object appearing |
| **Cloud and weather transitions** | Weather changes need new structure | Weather is a continuous field (a parameter of the FBM/SDF), not a state machine. It evolves; it never switches |
| **Interior ↔ exterior** | The classic level boundary | There is none: the door is a door, and the interior is the same parametric geometry as the hull, with the same pipes the solver generated |
| **Very long voyages** | Hours of real time | Sleep (N7). Diegetic, and the only honest option |
| **Entering a megastructure** | 4 km of cylinder to resolve | Matte composition + local frames: you see the curve of the cylinder from outside as a *matte*, and you are inside it before you notice the exchange |

---

## 4. What we refuse

- **Progress bars, spinners, "loading…", "press any key"** — none exist.
- **Cutscenes** for any transition. The peak-end rule (bible §2) says the ending must be *played*; a
  cutscene is a loading screen with better art.
- **Teleport-to-destination fast travel.** Time costs what it costs.
- **Pre-rendered transitions.** They are assets, and they would be the only assets.
- **A pause menu as an exit.** Leaving is a physical act (bible §9.2).

---

## 5. The honest caveat

One place a wait is unavoidable: **a cold first visit with no cache, on a slow link, on a device that
must download the wasm binary.** We cannot make 2.5 MB arrive faster than the connection allows.

What we can do — and what N1 does — is make sure that wait is **the wake-up**, that it is 400 KB
rather than 2.5 MB, that the room is already beautiful, that the audio is already running, and that
the player is never shown a bar or a percentage. A person lying in bed listening to cicadas and watching
a room come into focus is not waiting. They are waking up.

That is the whole doctrine: **the only moment of waiting in Odderon is the one moment where waiting is
the point.**
