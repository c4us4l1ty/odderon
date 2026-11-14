# WP 0.3: the Earth–Moon observatory

The observatory is a first, static, interactive visualization—not a flight
simulator. It works offline after its local files have loaded. It needs WebGL2,
WebAssembly, a modern browser, and a local HTTP server (not `file://`).

## Build and run

Install Rust's `wasm32-unknown-unknown` target, then from the repository root:

```sh
cargo build --locked --release -p odd-view --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/release/odd_view.wasm web/odd_view.wasm
python3 -m http.server 8000 --directory web
```

Open `http://localhost:8000/`. The wasm output is generated and ignored by Git;
rebuild it when Rust code or the packed ephemeris changes. In a remote preview,
bind the HTTP server to `0.0.0.0` and use the preview URL instead of localhost.

Drag or use arrow keys to rotate; scroll or use the buttons to zoom. Date
controls select a midnight **TDB**, not local time or UTC. The date range is
2020-01-01 through 2035-12-31 inclusive. The whole bundled ephemeris covers
2020-01-01 inclusive through 2036-01-01 exclusive TDB. See
[ephemeris.md](ephemeris.md) for sources, numerical checks and limitations.

## What is real and what is an illustration?

* Earth and Moon centers, Moon velocity and sunlight direction come from the
  JPL DE440s geometric states in `odd-ephem`. The Earth-to-Moon line is
  projected orthographically, as are the position samples of the Moon relative
  to Earth. The path shows half-day samples over up to 14 days on either side
  of the selected date, clipped at data boundaries. Solid means past, dashed
  means future; it is a sampled historical ephemeris, not a calculated
  spacecraft orbit or a predicted uncertainty interval.
* Body orientations use the approximate IAU models described in the ephemeris
  notes, **not** measured Earth orientation parameters or the DE440 lunar
  principal-axes frame. Light is a simple diffuse model without eclipses or
  physical atmosphere scattering.
* Each radius is enlarged 7.5 times while centers and path remain to the same
  scale; body shapes are spheres, not reference ellipsoids or topography.
  Procedural colors, clouds, polar pigment, atmospheric halo and brushwork are
  artistic stand-ins—not a map of land, weather or lunar surface. The background
  has paper grain but no fabricated catalog of stars. The optional little
  rocket and plume are a **screen-space illustration**, not a spacecraft,
  engine simulation or a real position. The optional velocity arrow uses the
  actual instantaneous relative velocity, scaled to one day: it is a tangent,
  not a propagated path.
* Orbital elements, apsides, B-plane targeting and vehicle trajectories require
  later physics packages. This viewport does not fabricate those results.
  `web/overlay.js` provides reusable polyline, vector, apsis-marker and B-plane
  bullseye drawing primitives for callers that have genuine computed data;
  they do not calculate those inputs.

## Rendering and idle behavior

The first WebGL2 pass renders analytic spheres and illustrative geometry into
an offscreen color/normal framebuffer. A second pass computes a Gaussian-smoothed RGB structure tensor, extracts its
orientation and anisotropy, and applies an eight-sector polynomial-weighted
anisotropic Kuwahara filter; procedural normal-tangent pigment strokes are
shader-generated. Its small fixed sample radius is a quality/performance
choice, not the 1.5 million instanced strokes proposed in brainstorming. A separate Canvas2D layer
renders sharp labels, samples and optional vector independently of the filter.
No external textures or 3D models are used. WebGL errors, unsupported WebGL2,
and lost/restored contexts have visible handling.

The governor schedules at most one animation frame at a time, and no repeating
animation or timer when settled. While dragging it draws at half linear
resolution; on release it schedules one final-quality pass after a **minimum**
50 ms debounce. Browser scheduling and GPU work can delay that pass; 50 ms
is not a real-time deadline. High-density rendering is capped at 1.5 device
pixels per CSS pixel and 2048 pixels on the longest side to bound memory/cost.
The cached compositor image stays visible until invalidated. Resizing, zoom,
keyboard input, date, overlay changes and context restoration wake the renderer;
hiding the document cancels pending work.

## Verification and performance

```sh
npm ci
npm test                       # governor and overlay primitive tests
npm run test:browser           # Chrome/Chromium and built web/odd_view.wasm
npm run test:idle              # Linux-only, after camera drag
# CHROME_BIN=/path/to/chromium npm run test:idle
```

The browser test uses real WebGL2, checks the 2026-01-01 Earth–Moon distance,
5.2 seconds with no application frames/pending work, drag and final-resolution
rendering, dates, toggles, keyboard, context restoration, WebGL errors and mobile
layout. It checks the filter on constant/noisy/high-contrast test images. It
logs Chrome main-thread task time; this alone is **not** total browser CPU.

The separate Linux idle gate opens the page, calibrates `/proc` accounting with
a busy browser loop, rotates/releases the camera, waits for the settled render,
finishes GPU work and measures ten seconds with no interaction. It adds up
scheduler execution time for all threads in the browser's descendant process
tree, including GPU and renderer processes, expressed as a percentage of one
CPU core. It fails if the process or thread set changes, if any app frames or
work remain scheduled, or if the measured CPU is not below 0.2%. It excludes the
Node test runner, external browser processes, hardware GPU utilization and
other system processes; it is a headless, software-rendered measurement, not a
physical-device profiler. The sample must be repeated on the target laptop with
a normal visible browser and GPU to establish its device-specific result.
Zero CPU and sub-0.2% on all devices are not guaranteed.

## Research

* [MDN: WebGL best practices](https://developer.mozilla.org/en-US/docs/Web/API/WebGL_API/WebGL_best_practices) — resizing on demand, context loss, conservative GPU memory.
* [Kyprianidis et al., anisotropic Kuwahara filtering with polynomial weights](https://diglib.eg.org/server/api/core/bitstreams/3309663a-3134-44bc-9297-2fa33554277d/content) — tensor-aligned elliptical kernels, smoothly overlapping sectors, and variance-weighted means.
* [NASA Moon Phase and Libration](https://svs.gsfc.nasa.gov/5415/) — phases depend on the Sun–Moon–observer geometry; this view uses geometric light directions, not a photometric model.
