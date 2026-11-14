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
* The spherical body radii, centers and path use the same spatial scale
  (Earth 6,378,137 m, Moon 1,737,400 m). Sub-eight-pixel diameters get
  screen-space locator reticles. Shapes are spheres, not reference ellipsoids
  or topography.
  Procedural colors, clouds, polar pigment, atmospheric halo and brushwork are
  artistic stand-ins—not a map of land, weather or lunar surface. The background
  has paper grain but no fabricated catalog of stars. The optional little
  rocket and plume are a **screen-space illustration**, not a spacecraft,
  engine simulation or a real position. The optional velocity arrow uses the
  actual instantaneous relative velocity, scaled to one day: it is a tangent,
  not a propagated path. When its projected length is under one screen pixel,
  the arrow is suppressed and labeled as almost along the view direction.
* Orbital elements, apsides, B-plane targeting and vehicle trajectories require
  later physics packages. This viewport does not fabricate those results.
  `web/overlay.js` provides reusable polyline, vector, apsis-marker and B-plane
  bullseye drawing primitives for callers that have genuine computed data;
  they do not calculate those inputs.

## Rendering and idle behavior

The first WebGL2 pass renders analytic spheres and illustrative geometry into
an offscreen color/normal framebuffer. Instanced brushstroke quads sample those
buffers, align to the projected surface-normal tangent and blend into a
persistent accumulation framebuffer. Batch 0 submits 60,000 coarse instances;
15 finer batches submit 96,000 each, for 1,500,000 instances. Strokes whose
samples lie in empty sky are discarded. This count is GPU *instances submitted*,
not a promise that every stroke produces visible fragments. The result is an
illustrative style, not measured surface data. A separate Canvas2D layer draws
labels and vectors. No external textures or 3D models are used.

The governor coalesces invalidations. While dragging it draws a single coarse
batch at half resolution; after a minimum 50 ms release debounce it rebuilds
at final resolution and schedules the 15 finite refinement frames. A new
interaction, resize, date change, context loss or visibility change cancels
or restarts stale work. When the last batch completes no further animation
frame is scheduled. High-density rendering is capped at 1.5 device pixels per
CSS pixel and 2048 pixels on the longest side. Overlay-only changes do not
repaint the world. Browser and GPU scheduling mean neither completion time
nor CPU/GPU usage is guaranteed.

## Verification and performance

`npm test` checks scheduling, scale, and the absence of recurring timers and CSS
keyframes. `npm run test:browser` exercises actual WebGL2 and, after batch 15,
checks that both animation callback and worker message counts stay unchanged
for two seconds. The application currently creates no workers. This is a
**deterministic no-scheduled-work check**, not a measurement of browser, OS,
or GPU utilization. The earlier Linux process-tree CPU test remains available
as an optional diagnostic (`npm run test:idle`) but is no longer a CI gate.
The finite progressive workload and target laptop still need performance
profiling; zero idle CPU/GPU cannot be promised.

## Research

* [MDN: WebGL best practices](https://developer.mozilla.org/en-US/docs/Web/API/WebGL_API/WebGL_best_practices) — resizing on demand, context loss, conservative GPU memory.
* [Kyprianidis et al., anisotropic Kuwahara filtering with polynomial weights](https://diglib.eg.org/server/api/core/bitstreams/3309663a-3134-44bc-9297-2fa33554277d/content) — tensor-aligned elliptical kernels, smoothly overlapping sectors, and variance-weighted means.
* [NASA Moon Phase and Libration](https://svs.gsfc.nasa.gov/5415/) — phases depend on the Sun–Moon–observer geometry; this view uses geometric light directions, not a photometric model.
