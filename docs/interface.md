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

- machined plates: near-black gradient bodies, a 1 px lit edge, cut corners,
  corner brackets and a screw, never a rounded card with a shadow;
- thin luminous strokes over near-black space, with one amber caution family,
  one red failure family and one green nominal family — colour is a state,
  never decoration;
- stencilled display type for names, monospaced type for every number, and
  small-caps labels with wide tracking for instrument names;
- switch-cap keys, gated ON/OFF rockers, ruled entry wells, ticked sliders and
  rotary-style handles — every control is a physical part of the machine;
- a persistent command strip (active station, operation, frame, authority), a
  persistent station rail, and a status rail at the frame foot;
- live, changing telemetry in the chrome — the software is doing something.

### The three laws

`tests/browser/interface.mjs` (`npm run test:ui`) enforces these mechanically
across seven route/station pairs. They are the definition of the standard; the
prose below only explains them.

1. **Plate law.** A box of 120×50 px or more may be a machined plate or bare
   glass, never a web card. Rounded corners fail outright; a flat light fill
   (alpha-weighted lightness above .22) fails, because a bright opaque panel is
   what a website looks like from across the room. Translated fills, gradients,
   recessed wells and hairline edges all pass: `dl.brief-spec` is legitimately a
   translucent tint at 8 % alpha and must stay one.
2. **Website law.** No prose block may exceed 240 characters or 520 px; no
   centred copy block may exceed 80 characters; no `h1` outside the title screen
   may exceed 42 px; the shell may expose at most five anchors; and no route may
   carry a footer. Console prose is a brief, not an article — reference
   paragraphs belong in a folded `<details>` strip, which the law exempts,
   because a pilot opens the manual rather than reading it off the glass.
3. **Instrument law.** On the Flight Deck the plan canvas must own more than a
   quarter of the viewport and stand taller than 40 % of it, the station rail
   must be flush to the frame edge and occupy more than 3 % of the width, and
   exactly one station panel may be active. A console with no working
   instrument is a form.

Type, control, frame and idle rules are asserted alongside them:

- **Type law.** No word computes below 9 px on any route or station, both canvas
  routes resolve all three Odderon faces before the first frame, and a named
  face must exist in `document.fonts` (a family the browser cannot find still
  answers `check() === true`, so presence is checked separately).
- **Control law.** Every visible control computes `appearance:none` and
  `border-radius:0px`; a typed field carries a box-shadow rule, never a native
  bevel; a slider's hit area is ≥ 20 px tall; a checkbox inside a rocker row is
  drawn as a travelling slug, so the rook itself is the control.
- **Frame law.** `body` is `overflow:hidden` and the document overflows by
  ≤ 0 px at 1440×900, 1280×720, 1024×600 and 390×844, on all three routes.
  Scrolling happens inside an instrument's own well, never on the page.

## 2. Visual language

| Token | Value | Use |
| --- | --- | --- |
| `--void` | `#04070a` | the machine's own black, behind every plate |
| `--glass` | `rgba(9,16,21,.82)` | instrument glass over rendered scenes |
| `--glass-hi` | `rgba(17,29,36,.9)` | raised plate, header strip |
| `--fl` | `#eef6f8` | primary readout ink |
| `--fl-2` / `--fl-3` / `--fl-4` | `#a9bec6` / `#738d97` / `#4a616b` | secondary, label, engraving |
| `--cy` / `--cy-hi` | `#63e2f4` / `#c6f7ff` | live phosphor, focus lamp |
| `--am` / `--am-hi` | `#ffb43f` / `#ffd894` | caution, armed key, delegated authority |
| `--rd` | `#ff5b45` | refusal and failure only |
| `--gn` | `#57e39b` | nominal state, verified recorder |
| `--seam` / `--seam-2` | hairline rules and recessed edges | plate seams |

Typefaces are bundled locally as WOFF2 in `web/assets/fonts` (see the OFL note
there), so the build has no network dependency and can ship inside a desktop or
Steam wrapper unchanged:

- **Odderon Gauge** (Oxanium, 500/600/700) — station names, panel titles, gauge
  numerals, key caps. A wide, angular technical face; it is what makes a heading
  read as a machined legend rather than a web font.
- **Odderon Panel** (Rajdhani, 600/700) — control captions and the few sentences
  a pilot must actually read. Condensed, large x-height, legible at 13 px.
- **Odderon Data** (Martian Mono, 400/500/700) — every measured value, unit,
  code, label suffix and status token. Tabular by construction.

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

**Nothing is composited above a live canvas.** No `backdrop-filter`, no
full-viewport gradient layer, no fixed overlay, no text shadow over the glass.
Any of them forces the browser's GPU process to redraw the scene every vsync even
when the game renders zero frames: measured on the tracking station, a
full-screen two-stop gradient took the idle gate from 0.05 % of a core to 4.3 %,
and a single text-shadow over a gauge to 3.3 %, while the app itself correctly
reported no frames and no scheduled work. Frame furniture therefore belongs to
the thing that draws the frame — the world shader's own vignette, the overlay
canvas's brackets, the plate's CSS bevel — never to a layer stacked on top of
it. `npm run test:idle` is the guard rail, and it samples CPU as well as
frames for exactly this reason. A dialog dims with a flat wash on an opaque
shell.

## 3. Routes

What the standard looks like on screen (1440×900 unless named otherwise), in
`docs/shots`:

| Screen | File |
| --- | --- |
| Title screen | `title.png` |
| Flight Deck, FDO | `flight-deck.png` |
| Flight Deck, ACD drawer | `academy.png` |
| Flight Deck, PCL commit console | `prediction.png` |
| Flight Deck, PRP Schlieren lab | `propulsion.png` |
| Flight Deck, VHC authority racks | `vehicle.png` |
| Tracking station | `observatory.png` |
| Flight Deck at 390×844 | `flight-deck-phone.png` |
| Tracking station at 390×844 | `observatory-phone.png` |
| Licence hatch | `dialog-license.png` |

These are checked-in references, shot from the built `web/` tree. Re-shoot them
whenever a route changes materially; a stale reference is worse than none.


| Route | Player-facing role | Shell |
| --- | --- | --- |
| `/` | Title screen: launch manifest and soft-key stack | `hud.css` + `title.css` |
| `/mission.html` | Flight Deck console; six stations | `hud.css` + `mission.css` |
| `/mission.html` Academy station | instructor's drawer beside the live flight board | same |
| `/observatory.html` | Earth–Moon tracking station | `hud.css` + `observatory.css` |

`hud.css` is the only shared sheet. It owns the eight `@font-face` rules, the
tokens, the `[hidden]` law, the control reset, the slider law, the dialog chrome,
focus, print and reduced motion. A route sheet owns that route's layout and
nothing else.

Deleting a shared sheet is where things break quietly, so two rules apply to
this file and are asserted by `npm run test:ui`:

- **A class markup or script applies must have a rule behind it.** When `ui.css`
  was deleted, eleven classes it owned were still being rendered — glossary
  terms, the licence cell grid, the secret block, the verification verdict, the
  dossier print frame. They did not error; they silently lost their presentation.
  The gate now collects every class present in the live DOM and fails if any has
  no rule in any loaded sheet. Two settings switches were also dead for the same
  reason: they wrote `reduced-effects` and `high-contrast` onto `<body>` with no
  rule behind either class, so they stored a preference and changed nothing.
- **A state class must reach the element that carries the colour.** The plan
  check toggled `is-bad` on the row while the value lived in a `<b>` inside it,
  so an empty or refused plan never turned red.

### Title screen

The camera plate (`./assets/title-orbit.jpg`) fills the frame. On it: a wordmark
plate high-left with mission state under it, the operation card bottom-right, a
frame/authority column above it, and the main menu as a **soft-key stack pinned
bottom-left** with function numbers and sub-captions. A ruled HUD rim and a
pitch ladder sit over the plate, and the plate is labelled as what it is — a
live external camera (`EXT CAM 03 · DOCKING APPROACH`, timecode). The status line
under the menu is the game's own state (`FOUNDATION 0.3 · LOCAL PRACTICE`),
never a copyright or site-map band.

The wordmark block carries no marketing strapline: the line under the title is
mission state (`Contract 101 staged · Window 03:00:00`), because a launch screen
inside a game reports the current operation, not the product's pitch. The
operation card, the frame column and the soft keys are plates on the artwork,
and the whole screen is read through a drawn HUD rim rather than a page margin.

Each soft key is a machined key on the plate with a cut corner, a function
number and a sub-caption, the primary key lit amber and armed. The stamp is
positioned from the top of the void cell, not of the frame — its children are
absolutely placed inside a grid area, so `top:16px` would print the kicker
through the wordmark at phone width.

### Flight Deck

A fixed grid: a top command strip, the console body, and a status line. The
console body is `grid-template-columns:64px minmax(0,1fr)` with a single
`minmax(0,1fr)` row, so the station rail is a **vertical rail docked to the left
edge of the frame, spanning its full height** — a hardware legend strip with a
number, a code and a name on each latching key, not a row of tabs. `.console-main` beside it holds the command strip and
`#station-surface`, the one scrollable well on the screen.

The rail is the physical keyboard of stations:

| Key | Station | Workspace |
| --- | --- | --- |
| FDO | Flight | orbit instrument + maneuver control column |
| ACD | Academy | instructor's drawer beside the live flight board |
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
2. **A station either shows or hands over.** The Academy station renders as a
   drawer bolted to the right of the live flight board (below it on a phone), so
   the lesson always runs against the real plan and the instrument keeps the
   frame instead of collapsing under prose. The drawer scrolls; the glass does
   not.

**A refused plan reports no numbers.** `updatePreview` returns early when the
plan cannot compile, and it must still re-read the plan check on the way out.
Without that call the check kept the last valid solution's delta-v and
propellant on screen beside the refusal — a healthy 117.2 kg next to "that needs
more fuel than this stage carries". A refused plan now reads `REFUSED` with the
figures cleared, and the gate asserts both the cleared value and the failure
styling.

Two geometry invariants are asserted, because both were broken by a rule that
looked harmless: the rail must reach the foot of `.console-body` within 2 px
(`align-self:stretch`, plus `.console-body > *{min-height:0}`), and every direct
child of the console body must be allowed to shrink (`min-height:0`), or the
rail's content pushes the frame taller than the viewport.

The station glass carries its own symbology on `#plan-hud`, painted by
`web/instrument.js` from `mission:plan` and `mission:flown`: a machined range
ring with bearing marks, an unnumbered altitude tape with apsis pennants, a burn
node rail, a reticle on the selected burn, and two corner stamps (commanded
delta-v, plan code and burn count). The plate's top edge is a 22 px labelling
strip, so a caption never lands on the orbit solution; the tape and stamps are
drawn *inside* the glass, never above it.

Height budgets are set for a 1280×720 laptop as the floor: the propulsion rack
must keep at least 180 px of cross-section (its readout band reflows, the
canvas never collapses) and must not overflow its own bezel; the academy well
must keep a scrollable reading window while the flight board stays ≥ 200 px. On
a phone the cross-section keeps `min(48vh,300px)` for the same reason: the
engine view scales with its own height, so a short well squeezes the nozzle
until the drag captions sit in the plume. Canvas captions are dropped to their
value alone (`O/F 2.00`, `Pc 1.50 MPa`) below 560 px of canvas width.

### Observatory

The optics own the frame edge to edge: `#world` and `#overlay` fill an absolute
scene, and the range and range-rate gauges are bolted Windows floating on the
glass at its top right, each with a value, a unit and an engraved tick rail. A
control rack docks right (layer rockers, L3 status, epoch well), the drag/zoom
rail and true-scale note sit at the bottom right of the optics, and a legend
strip runs across the scene's top edge. No intro block, no footer. The gate
asserts the scene is > 70 vw, the telemetry column < 30 vw, the optics reach the
top of the console, exactly four layers exist and the published range matches
the ephemeris.

At 760 px and under the control rack moves under the optics, and at 420 px the
two gauges become a bottom row of the scene so the range is never hidden behind
the frame. The drag/status chips move up off the foot so the scale-and-zoom rail
does not print through them.

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

The Flight Deck's canvas symbology lives in three modules that carry no state of
their own: `web/hud-layer.js` (primitives — `bezel`, `glass`, `consoleFrame`,
`altitudeTape`, `nodeRail`, `reticle`, `cornerReadout`, `trackingFrame`),
`web/instrument.js` (the demand-driven `#plan-hud` painter; it arms exactly one
frame and never starts a loop) and `web/rail.js` (mirrors station, authority,
plan and execute state into the frame lamps). A new canvas ornament belongs in
`hud-layer.js` and is called from a painter; nothing draws chrome inside
`app.js`'s `draw()`, which owns the world solution.

Classes owned by `hud.css` (do not restyle locally): `.panel`, `.panel-title`,
`.station-panel`, `.station-head`, `.plan-check`, `.plan-check-line`,
`.panel-code`, `.section-label`, `.field-label`, `.hint-line`, `.metric*`,
`.hud-chip`, `.status-line`, `.engine-readout`, `.prediction-grid`,
`.commit-*`, `.toggle-row`, `.switch`, `.rocker`, `.causal`, `.lens`,
`.onboarding-*`, `.profile-*`, `.verify-*`, `.prediction-table`, and all
`dialog` chrome.

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
3. Never add `@keyframes`, `setInterval`, a composited layer over a canvas,
   CDN assets, or a scrolling body.
4. Re-run `node --test tests/browser/idle-audit.test.js`, `npm run test:ui` and
   the browser gates (`npm run test:browser`, `npm run test:idle`) after any
   change to markup, shell, or control styling. `npm run test:ui` is the guard
   rail for this document: if it fails, the interface has regressed, not the
   test. `npm run test:idle` is the guard rail for the no-blur rule — it counts
   frames *and* samples browser CPU, so a styling change that re-arms the GPU
   shows up there even though the app reports a perfectly idle renderer.
5. Update this document if the standard itself changes.
