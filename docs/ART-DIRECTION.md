# Art direction — how the summer-noon street is actually built

The target is the reference frame in `.artifacts/reference-summer-noon.png`
("Shadows of a Summer Noon"): a sunlit Japanese machiya street, raking warm sun,
deep cyan sky with towering cumulus, muted timber and stucco, saturated grass,
dappled shade, depth haze.

The doctrine (`docs/VISION.md` §3, `docs/PERFORMANCE.md`, `IMPORTANT-2.md`) rules
out the usual tools: no textures, no models, no shadow maps, no extra draw
calls, no per-frame allocation, and everything must hold 60 fps on an integrated
GPU. So the look has to come from **light ratios, value structure, palette and
geometry** — in that order. This file records the model that is now implemented
so nobody re-derives it from scratch.

## 1. One "art sun", two Z leans

`sun_state_uchinoura()` gives a real ephemeris vector, but at solar noon that
vector is nearly vertical over a north–south street, which drops the whole
canyon into shade and makes every facade black. The world pass therefore
post-processes it into an **art sun**: the elevation is clamped to 0.40–0.60 and
the azimuth is biased toward `-X` (along the street) so light rakes down the
facades. Its Z component keeps a fixed magnitude and only its *sign* follows the
surface:

| surface | Z lean | why |
| --- | --- | --- |
| facade facing +Z (`N0.z > 0.25`) | + | lit from the street side |
| facade facing -Z | − | ditto |
| ground / roofs | flips near `z ≈ 6.4` | each half of the street is shaded by the row it faces |

Blending two opposite unit suns instead of flipping the sign cancels the Z
component and puts an almost-vertical sun back — that failure mode looked like
"half the street is night".

The interior keeps the true ephemeris vector; it drives the window sunbeam mask.

## 2. Shadows are geometry, not paint

`streetSunVis()` ray-tests the camera-ray position against a dozen boxes that
mirror the real massing in `crates/odderon-geom/src/town.rs`:

* near row: facade `z = 2.22`, bodies extend to `-Z`;
* far row: facades `z = 11.45` / `11.75`, bodies extend to `+Z`;
* thin slabs for each 2F kawara eave and 1F hisashi, so the shade under an eave
  is the depth of its overhang divided by the sun's tangent.

If `town.rs` geometry moves, these boxes must move with it. There is no shadow
map and there never will be at this budget.

`computeExteriorEaveShadow()` keeps only the *fine* gobos: dappled zelkova
canopy inside the tree footprints, plus the utility-pole and overhead-wire
stripes on the ground plane. The old per-storey y-band "eave shadow" ramps are
gone — they ignored the sun direction and read as dirt.

## 3. Fill light is the sky

A vertical surface in shade on a clear day still receives a lot of light from
the sky, and a soffit receives it from the sunlit street below. Both are
modelled explicitly:

```
skyFill    day = (0.40, 0.50, 0.78)      // hemispherical blue
groundFill day = (0.24, 0.20, 0.15)      // warm concrete bounce
ambient *= mix(0.66, 1.0, shadowVis)     // shade is dimmer, never black
ambient  = mix(ambient, ambient*1.85 + groundFill*0.55, soffitMask)
```

Without these two terms, ACES' quadratic toe crushes sky-lit timber to `0.01`
and every eave reads as a hole punched in the wall.

## 4. Grade to the reference's histogram, not to taste

`tools/art-compare.mjs` prints the distribution of every view next to the
reference frame. The reference is a low-key image:

| | mean | p25 | p50 | p95 | dark<0.09 | hot>0.95 | sat |
| --- | --- | --- | --- | --- | --- | --- | --- |
| reference | 0.288 | 0.092 | 0.228 | 0.684 | 39 % | 0 % | 0.444 |
| 05 street | 0.305 | 0.141 | 0.255 | 0.625 | 29 % | 0 % | 0.437 |

The film chain is: `aces(lit * 0.52)` → sRGB → gamma 1.10 → 22 % S-curve →
vibrance `1.05 + 0.34·(1-sat)` → split tone (cool violet shade, warm amber
light) → lifted black point `x·0.975 + 0.012`.

Matching the *mean* while carrying 15 % dark pixels is the classic failure: it
produces pastel product renders. Match the distribution.

## 5. Sky

An analytic deck (`rd.xz / max(rd.y, 0.02)`) with three noise scales: tower
grouping, billow shape, and a fine octave that **erodes** the rim (that erosion
is what gives a cumulus silhouette instead of fog). The field is biased so
coverage peaks 15–40° above the horizon, which is where the reference's towers
stand over the ridge line. Bases self-shadow toward the sun sample and darken at
low elevation; crowns keep a silver lining.

## 6. Things that look wrong and are therefore deliberate

* **Grass is geometry.** Five tapered, two-segment blades per tuft on a
  golden-angle fan, dark root to lit tip, double-sided. That is 8 triangles a
  blade and the reason the static budget is 64 k instead of 30 k
  (`crates/odderon-geom/src/lib.rs`).
* **The meadow is one box.** It is a backdrop; it is muted (`0.072, 0.132,
  0.055`) and gets the distance haze so it recedes instead of glowing.
* **Interiors are dark.** A 25 m² room with one window is mostly below 0.09
  luma, and that is the point — the window is the brightest thing in the game.

## 7. How to iterate

`tools/README.md`. Short version: build the wasm, run `server.mjs`, then
`node tools/screenshot.mjs` and `node tools/art-compare.mjs`. Use
`?debug=4` (sun share) to find out why a surface is black — it separates "no
sun" from "no light at all".
