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
- instrument housings with cut corners, hairline bezels and index ticks —
  machined panels, not cards;
- stencilled condensed display type for names, monospaced type for every
  number, and small-caps labels with wide tracking for instrument names;
- switch-cap buttons, entry wells with scanline fill, rocker switches, and
  rotary-style sliders;
- a persistent command strip (active station, operation, frame, authority) and
  a persistent station rail, as a real console has;
- live, changing telemetry in the chrome — the software is doing something.

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

No CSS animation loops or keyframe animations exist anywhere: the governance
rule in `tests/browser/idle-audit.test.js` forbids `@keyframes` and
`setInterval`, and the renderer is demand-driven. The interface must look alive
through *data*, never through motion.

## 3. Routes

| Route | Player-facing role | Shell |
| --- | --- | --- |
| `/` | Title screen: launch manifest and soft-key stack | `ui.css` + `title.css` |
| `/mission.html` | Flight Deck console; six stations | `ui.css` + `mission.css` |
| `/mission.html#onboarding` | Academy station inside the same console | same |
| `/observatory.html` | Earth–Moon tracking station | `ui.css` + `observatory.css` |

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

### Flight Deck

A fixed grid: 56 px top command bar, a 78 px station rail, the workspace, and a
24 px status line. The rail is the physical keyboard of stations:

| Key | Station | Workspace |
| --- | --- | --- |
| FDO | Flight | orbit instrument + maneuver control column |
| ACD | Academy | lesson well above the live flight board (split station) |
| PRP | Engine | Schlieren cross-section + geometry column + readout strip |
| VHC | Vehicle | discipline authority, cryo fit-out, stage mass ledger |
| PCL | Predict | six-field commit ledger + attempt authority + commit key |
| FDR | Debrief | signed prediction-versus-reality schedule |

Under 900 px the rail becomes a bottom hardware game bar, the command strip
keeps only its first two blocks, and the active workspace becomes the only
scrollable surface. The chrome never reflows into a document.

Two structural rules follow from that, and both are asserted by
`npm run test:ui`:

1. **`hidden` means gone.** The system rule `[hidden]{display:none!important}`
   outranks every author `display` in the sheets, so a container that is hidden
   by script — a control row, a lesson block, a panel — can never keep painting.
2. **A station either shows or hands over.** The Academy station is a nameplate
   plus a fixed-height lesson well, sitting *above* the live flight board, which
   is a sibling panel. The lesson therefore always runs against the real plan,
   and the board keeps its own column instead of collapsing under the prose.

Height budgets are set for a 1280×720 laptop as the floor: the propulsion rack
must keep at least 180 px of cross-section (its readout band reflows, the
canvas never collapses) and must not overflow its own bezel; the academy well
must keep a scrollable reading window while the flight board stays ≥ 200 px.

### Observatory

The ephemeris viewport owns the frame; the telemetry column docks right; the
time scrubber runs along the bottom. Instruments: distance, relative speed,
display-layer rockers, the L3 status line and the scale note. No intro block,
no footer.

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

Classes owned by `ui.css` (do not restyle locally): `.panel`, `.panel-title`,
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

1. Edit `web/ui.css` for anything shared; edit the route stylesheet only for
   that route's layout.
2. Keep the colour and type tokens above; add a token rather than a literal.
3. Never add `@keyframes`, `setInterval`, CDN assets, or a scrolling body.
4. Re-run `node --test tests/browser/idle-audit.test.js`, `npm run test:ui` and
   the browser gate (`npm run test:browser`) after any change to markup, shell,
   or control styling. `npm run test:ui` is the guard rail for this document:
   if it fails, the interface has regressed, not the test.
5. Update this document if the standard itself changes.
