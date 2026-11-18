# Odderon interface standard

Odderon ships as a **game**, not a website. This document is the binding
description of the shipped interface: what it must look like, what it must
never look like, and which files own which part of it. Change the code and this
document together.

The audience for this file is anyone — human or agent — about to touch `web/`.

## 1. The rule

Odderon's screens are the mission software of the vehicle: a flight computer
rendered on glass inside a cockpit. A player who lands on any route must be able
to tell within one second that they are looking at a game, not a marketing page,
a documentation site, or a web application.

Concretely, the shipped interface must **not** contain:

- a hero section, an oversized marketing headline, or a strapline block;
- a landing-page menu of large centred buttons with paragraph descriptions;
- a card grid, drop shadows on content, or rounded web cards;
- document flow: the browser page itself never scrolls on desktop; only an
  instrument's own data column may scroll inside its bezel;
- an `<h1>` used as page branding inside the game console (the title screen is
  the only route with a `h1`, and it is the game's name plate);
- a website footer with site-map links, copyright bands, or "about" columns;
- native browser-styled form controls: default checkboxes, default select
  arrows, default number spinners, default focus rings;
- external CDN fonts, icon packs, or third-party UI kits.

…and **must** read as cockpit instrumentation:

- thin, luminous monochrome HUD strokes over near-black space, with one amber
  and one red caution family and one green "nominal" family;
- type laid straight on the rendered frame, held by a gradient scrim that fades
  to nothing at one edge — never type inside a filled plate;
- stencilled condensed display type for names, monospaced type for every
  number, and small-caps labels with wide tracking for instrument names;
- switch-cap keys and rocker switches that light when armed, ruled entry wells,
  and rotary-style sliders;
- a persistent command strip (active station, operation, frame, authority) and
  a persistent station rail, as a real console has;
- live, changing telemetry in the chrome — the software is doing something.

### The four laws

`tests/browser/interface.mjs` (`npm run test:ui`) enforces these mechanically.
They are the definition of the standard; the prose below only explains them.

1. **Ink law.** No element of 120×50 px or more may carry a filled plate. It
   fails if its background alpha is ≥ .45, if its only edge is a lone
   `inset 0 0 0 1px` ring, or if it has a native border. Type sits on a
   gradient scrim or on the frame itself.
2. **Control law.** Every visible control computes `appearance:none` and
   `border-radius:0px`. A typed field must carry a box-shadow entry rule
   (an underline or a bracket), never a native bevel. A slider's hit area is
   ≥ 20 px tall.
3. **Frame law.** `body` is `overflow:hidden` and the document overflows by
   ≤ 0 px at 1440×900, 1280×720, 1024×600 and 390×844, on all three routes.
   Scrolling happens inside an instrument's own well, never on the page.
4. **Type law.** No word computes below **9 px** on any route or station, and
   both canvases pages resolve all three Odderon faces
   (`document.fonts.check`) before the first frame is painted.

## 2. Visual language

| Token | Value | Use |
| --- | --- | --- |
| `--hud` | `#7fe3d8` | primary instrument phosphor |
| `--hud-bright` | `#c8fff6` | live readouts, focus |
| `--hud-dim` | `#4c9d99` | labels, secondary strokes |
| `--amber` | `#f4b860` | caution, explanatory asides, alternate curves |
| `--red` | `#ff6f56` | failure and refusal text only |
| `--green` | `#79e6a4` | nominal state, verified results |
| `--violet` | `#b79cf7` | patched/derived trajectories |
| `--space` | `#02060a` | the void behind the glass |
| `--frame` | `rgba(6,15,21,.93)` | instrument housing fill |
| `--stroke` | `rgba(103,214,205,.19)` | bezel hairlines |
| `--txt` | `#cfe3e1` | body copy on instruments |
| `--txt-2` / `--txt-3` | `#8fa9ab` / `#5d767b` | secondary and tertiary copy |

Typefaces are bundled locally as WOFF2 in `web/assets/fonts` (see the OFL note
there), so the build has no network dependency and can ship inside a desktop or
Steam wrapper unchanged:

- **Odderon Display** (Barlow Condensed) — station names, panel titles, numbers
  that belong to a name plate. Never for body copy.
- **Odderon Data** (IBM Plex Mono) — every measured value, unit, code, label
  suffix and status token. This is what makes readouts read as instruments.
- **Odderon Console** (Rajdhani) — explanatory sentences and control captions,
  only where a player must actually read a paragraph.

Canvas drawers cannot inherit a CSS font stack, so scene labels use
`web/glyphs.js` (`CANVAS_MONO`, `CANVAS_DISPLAY`, `paintLabel`). A drawer must
never hard-code a font: a system fallback there is invisible to the DOM audit
and makes a solved scene look like a web chart. `loadCanvasFaces()` resolves the
bundled faces before the first frame, so a label is painted once in instrument
type instead of once in a fallback.

Two hard floors apply to all type:

- **9 px.** No element carrying words may compute below 9 px at any viewport.
  Instrument type is small by design; under 9 px it is a smudge, not a readout.
  `tests/browser/interface.mjs` asserts this on every route at desktop and phone
  width. (It began as 8 px and was raised, because a 1×-DPI screenshot proved
  8 px is legible in the DOM but not on the screen.)
- **No squeezed prose.** A grid row that is `auto` must be allowed to hold its
  full content. Only the row that owns the live instrument (a canvas) may absorb
  a short container; otherwise a two-line brief is sliced mid-line with the
  descenders cut off.

Three stacking rules exist because each one silently produced overlapping type
on a phone before it was written down:

- **Stacked stations restate their rows.** A station that goes from two columns
  to one at narrow width must set `grid-template-rows` as well as
  `grid-template-columns`. Changing only the columns leaves the desktop row
  template in place, so the two tracks split the framed height and the second
  block spills out of its own row across the next section.
- **Grid areas do not move their children.** A child with `grid-row:3` stays in
  row 3 even when the template's named areas are reordered; the phone rule must
  restate the row number too.
- **A stacked block is content, not a pane.** Inside a single scrolling well,
  every station block is `flex:0 0 auto`. A block that may shrink divides a
  1,500 px control column away to nothing.

No CSS animation loops or keyframe animations exist anywhere: the governance
rule in `tests/browser/idle-audit.test.js` forbids `@keyframes` and
`setInterval`, and the renderer is demand-driven. The interface must look alive
through *data*, never through motion.

**No `backdrop-filter` anywhere near a canvas.** A backdrop blur over a live
WebGL surface makes the browser's GPU process redraw the scene every vsync even
when the game renders zero frames. It is invisible styling with a visible cost:
it took the idle gate from 0.05 % of a core to 34 %, with the app itself
correctly reporting no frames and no scheduled work. Plates hold their type with
a gradient of their own ink; a dialog dims with a flat wash.

## 3. Routes

| Route | Player-facing role | Shell |
| --- | --- | --- |
| `/` | Title screen: launch manifest and soft-key stack | `hud.css` + `title.css` |
| `/mission.html` | Flight Deck console; six stations | `hud.css` + `mission.css` |
| `/mission.html` Academy station | lesson dock above the live flight board | same |
| `/observatory.html` | Earth–Moon tracking station | `hud.css` + `observatory.css` |

`hud.css` is the only shared sheet. It owns the eleven `@font-face` rules, the
tokens, the `[hidden]` law, the control reset, the slider law, the dialog chrome,
focus, print and reduced motion. A route sheet owns that route's layout and
nothing else.

### Title screen

The camera plate (`./assets/title-orbit.jpg`) fills the frame. On it: a stencil
wordmark plate high-left, the operation card bottom-right, a telemetry column
top-right, and the main menu as a **soft-key stack pinned bottom-left** with
function numbers and sub-captions. A ruled HUD rim and a pitch ladder sit over
the plate, and the plate is labelled as what it is — a live external camera
(`EXT CAM 03 · DOCKING APPROACH`, timecode, scale bar). The status line under
the menu is the game's own state (`FOUNDATION 0.3 · LOCAL PRACTICE`), never a
copyright or site-map band.

The wordmark block carries no marketing strapline: the line under the title is
mission state (`Contract 101 staged · Window 03:00:00`), because a launch screen
inside a game reports the current operation, not the product's pitch.

Each soft key is type on the plate: a gradient scrim fading left to right, with
the same ramp masking its hairline so the ring does not close into a plate
outline under the last letter. The stamp is positioned from the top of the void
cell, not of the frame — its children are absolutely placed inside a grid area,
so `top:16px` would print the kicker through the wordmark at phone width.

### Flight Deck

A fixed grid: a top command strip, the console body, and a status line. The
console body is `grid-template-columns:56px minmax(0,1fr)` with a single
`minmax(0,1fr)` row, so the station rail is a **vertical rail docked to the left
edge of the frame, spanning its full height** — a hardware legend strip, not a
row of tabs. `.console-main` beside it holds the command strip and
`#station-surface`, the one scrollable well on the screen.

The rail is the physical keyboard of stations:

| Key | Station | Workspace |
| --- | --- | --- |
| FDO | Flight | orbit instrument + maneuver control column |
| ACD | Academy | lesson well above the live flight board (split station) |
| PRP | Engine | Schlieren cross-section + geometry column + readout strip |
| VHC | Vehicle | discipline authority, cryo fit-out, stage mass ledger |
| PCL | Predict | six-field commit ledger + attempt authority + commit key |
| FDR | Debrief | signed prediction-versus-reality schedule |

At 820 px and under, the rail becomes a bottom hardware game bar (≤ 70 px above
the viewport foot, > 300 px wide), the command strip keeps only its first two
blocks, and the active workspace becomes the only scrollable surface. The chrome
never reflows into a document.

Two structural rules follow from that, and both are asserted by
`npm run test:ui`:

1. **`hidden` means gone.** The system rule `[hidden]{display:none!important}`
   outranks every author `display` in the sheets, so a container that is hidden
   by script — a control row, a lesson block, a panel — can never keep painting.
2. **A station either shows or hands over.** The Academy station is a nameplate
   plus a fixed-height lesson well, sitting *above* the live flight board, which
   is a sibling panel. The lesson therefore always runs against the real plan,
   and the board keeps its own column instead of collapsing under the prose.

Two geometry invariants are asserted, because both were broken by a rule that
looked harmless: the rail must reach the foot of `.console-body` within 2 px
(`align-self:stretch`, plus `.console-body > *{min-height:0}`), and every direct
child of the console body must be allowed to shrink (`min-height:0`), or the
rail's content pushes the frame taller than the viewport.

Height budgets are set for a 1280×720 laptop as the floor: the propulsion rack
must keep at least 180 px of cross-section (its readout band reflows, the
canvas never collapses) and must not overflow its own bezel; the academy well
must keep a scrollable reading window while the flight board stays ≥ 200 px. On
a phone the cross-section keeps `min(48vh,300px)` for the same reason: the
engine view scales with its own height, so a short well squeezes the nozzle
until the drag captions sit in the plume. Canvas captions are dropped to their
value alone (`O/F 2.00`, `Pc 1.50 MPa`) below 560 px of canvas width.

### Observatory

The ephemeris viewport owns the frame; the telemetry column docks right; the
time scrubber runs along the bottom. Instruments: distance, relative speed,
display-layer rockers, the L3 status line and the scale note. No intro block,
no footer. The gate asserts the scene is > 70 vw and the telemetry column
< 30 vw.

At 820 px and under the telemetry moves under the glass as one ruled block above
the time bar, and the scene keeps the top of the frame. The drag/status chips
move up off the foot so the scale-and-zoom rail does not print through them.

A scene drag is also where the node-time control is exercised: because a coast
can coil its ground track across itself, the nearest-point search that maps a
pointer to a path epoch breaks ties towards the node's *current* epoch. Without
that bias a drag of a few pixels teleports a burn from the path start to the
path end.

## 4. Component contract

Every interactive element keeps its existing `id` and `data-*` hooks: the
acceptance suites address controls and canvases by id, and physics, signing and
verification code depends on them. Redesigns change *presentation*, never the
DOM contract. Known load-bearing hooks:

- `#plan-canvas`, `#engine-canvas`, `#lens-canvas`, `#world`, `#overlay`,
  `#scene` — sized by their immediate parent and observed with
  `ResizeObserver`, so keep each canvas first child of a positioned box that has
  a definite height;
- `[data-flight-mode]` / `[data-flight-panel]` — station switching; exactly one
  panel is visible at a time;
- `#system-state`, `#distance`, `#speed`, `#trajectory-status` — text asserted
  verbatim by the gate;
- station keys `1`–`6` and `Q`/`E` (`web/flight-shell.js`): digits select a
  station, Q/E step along the rail; suppressed for modifier chords, auto-repeat,
  typing inside a field, an open dialog, or a debrief that does not exist yet;
- `.spacecraft` — must paint the title artwork;
- `.game-menu`, `.flight-mode-nav`, `.settings-done` — addressed by the gate.

Classes owned by `hud.css` (do not restyle locally): `.panel`, `.panel-title`,
`.station-panel`, `.station-head`, `.plan-check`, `.plan-check-line`,
`.panel-code`, `.section-label`, `.field-label`, `.hint-line`, `.metric*`,
`.hud-chip`, `.status-line`, `.engine-readout`, `.prediction-grid`,
`.commit-*`, `.toggle-row`, `.switch`, `.causal`, `.lens`, `.onboarding-*`,
`.profile-*`, `.verify-*`, `.prediction-table`, and all `dialog` chrome.

## 5. Storefront boundary

The storefront is **not** part of this repository. A separate site (today
Cloudflare Pages, later a Steam page) sends the player to the deployed root
route with a Play button. Odderon therefore contains no landing page, no
purchase flow, no account wall and no marketing copy of its own: `/` is already
the game. Static hosting, a local server, a desktop webview wrapper and a future
Steam build all serve the same `web/` directory unchanged.

## 6. Changing the interface

1. Edit `web/hud.css` for anything shared; edit the route stylesheet only for
   that route's layout.
2. Keep the colour and type tokens above; add a token rather than a literal.
3. Never add `@keyframes`, `setInterval`, CDN assets, or a scrolling body.
4. Re-run `node --test tests/browser/idle-audit.test.js`, `npm run test:ui` and
   the browser gates (`npm run test:browser`, `npm run test:idle`) after any
   change to markup, shell, or control styling. `npm run test:ui` is the guard
   rail for this document: if it fails, the interface has regressed, not the
   test. `npm run test:idle` is the guard rail for the no-blur rule — it counts
   frames *and* samples browser CPU, so a styling change that re-arms the GPU
   shows up there even though the app reports a perfectly idle renderer.
5. Update this document if the standard itself changes.
