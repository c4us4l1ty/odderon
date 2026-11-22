# `tools/` — art iteration harness

Everything here is dev-only. Nothing in this folder ships, and the game never
imports it. It exists so a human or an agent can *look* at the renderer, compare
it against the reference frame, and get numbers back — the loop that
`docs/VISION.md` and `IMPORTANT-2.md` assume you are running.

```
node tools/screenshot.mjs                # all views -> .artifacts/screenshots/
node tools/screenshot.mjs --only 05-street-meadow
node tools/art-compare.mjs               # luma/sat report + side-by-side sheets
```

## 1. Prerequisites

* A built wasm: `bash scripts/build-wasm.sh` (needs `$HOME/.local/rust/bin` on
  `PATH`; prints `[Odderon] Build complete.`).
* The static server: `node server.mjs` (listens on `0.0.0.0:3000`).
* Chromium: `ensure-chromium.mjs` downloads the `@sparticuz/chromium` build into
  `tools/.cache/` on first use (`npm install` in this folder first). The
  container has no `libnss3`/`libnspr4`, so `chromium-nss-shim/` builds stub
  shared objects from source that satisfy ld.so without implementing NSS. See
  that folder's README before touching it.

## 2. Views

`views.json` is the canonical camera list. Coordinates are metres, `y = 0` is
the 2F Hab #204 floor, the sidewalk is at `y = -3.06`; `yaw` is degrees with
`0 = +Z`, `90 = +X`; `pitch` is degrees, positive is up. Add a view here rather
than hard-coding a pose in a script — `art-compare.mjs` reads the same file.

## 3. How capture works

`web/main.js` exposes, in dev only (`?capture=1`):

```
?capture=1&hud=0&cam=x,y,z,yawDeg,pitchDeg&sun=x,y,z&warmth=0.65&debug=N
globalThis.__odderonCapture = { pose(), frame(), freeze(), thaw() }
```

`freeze()` stops the render-on-demand loop so a screenshot can be taken; this
matters because SwiftShader never completes an unattended `page.screenshot`
(`timeout: 120000` is still required).

`debug=N` renders an AOV instead of the beauty frame:

| N | channel |
|---|---------|
| 1 | sun visibility (shadow term) |
| 2 | ambient occlusion |
| 3 | material id |
| 4 | sun share `direct / (direct + ambient)` |
| 5 | albedo × detail |
| 6 | world normal |
| 7 | street massing boxes only |
| 8 | fine gobos only |
| 9 | art-sun vector |

Mode 4 is the one that finds "why is this wall black": it separates *no sun*
from *no light at all*. Mode 7/8 splits the massing-shadow term from the
procedural gobo term when one of them misbehaves.

## 3b. Why the shots are reproducible

A capture is meant to be a pure function of its URL, which means the world is
not allowed to drift between runs:

* capture mode holds the world clock still (`dtMs = 0` every frame), so the sim
  sits at its boot state no matter how long the page took to load: the sun angle
  and the shuttle bus used to drift by a few hundred milliseconds between runs;
* capture mode ignores the presenter's park catch-up (`wakePresenter` normally
  fast-forwards the sim across a parked spell), so the shuttle never jumps onto
  its 90 s schedule mid-shot;
* capture mode does not schedule the T = 9 s static fire, which is wall-clock
  timed and used to drop a 300 m plume into some shots and not others;
* `freeze()` paints its own frame before returning, because a parked page keeps
  whatever the compositor last held.

Two runs of the same view are byte-identical. If that ever stops being true,
one of the three rules above has been broken.

## 4. Reading `art-compare.mjs`

It downsamples every view to 640x360 and prints mean / p5 / p25 / p50 / p75 /
p95 luma, the share of pixels below 0.15 ("dark") and above 0.85 ("hot"), mean
saturation, and the warm/cool balance, next to the same numbers for
`.artifacts/reference-summer-noon.png`. Side-by-side sheets land in
`.artifacts/compare/`.

Match the *distribution*, not the mean: the reference is a low-key frame
(mean 0.29, 39 % dark, 0 % hot). A render that hits the mean but has 15 % dark
looks flat and pastel no matter how good the albedo is.

## 5. Gotchas

* GLSL ES 3.00 reserved words: `patch`, `sample`, `filter`, … A single backtick
  inside a JS template literal silently truncates `web/shaders.js`.
* All noise helpers (`fbm2`, `fbm3`, `vnoise`, `hash21`, `aces`, `planarUV`)
  live near the top of `WORLD_FRAG_GL2`. Replacing a function body with a
  slice-based regex has deleted them before — edit by brace-matched function
  span, and re-check the page loads after every shader edit.
* `pkill -f "node server.mjs"` inside a compound bash command kills the shell.
