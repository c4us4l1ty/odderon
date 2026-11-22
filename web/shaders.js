// Analytical WebGL2 Shaders for Odderon Checkpoint 1A v4 ("Summer Noon — Hab #204 & Machiya Street").
// Zero texture files (0 bytes). Tuned for deep cinematic Summer Noon dynamic range:
// rich Kominka roasted cedar, aged sand-ochre stucco, deep eave & dappled foliage gobo shadows,
// saturated teal-cerulean summer sky, towering cumulus clouds, and zero cel/toon banding.

export const SKY_VERT_GL2 = `#version 300 es
precision highp float;
const vec2 POS[3] = vec2[3](
  vec2(-1.0, -1.0),
  vec2( 3.0, -1.0),
  vec2(-1.0,  3.0)
);
out vec2 vNdc;
void main() {
  vNdc = POS[gl_VertexID];
  gl_Position = vec4(vNdc, 0.9999, 1.0);
}
`;

// ---- NEW SKY FRAGMENT (spliced into shaders.js by tools/… no, by hand) ----
export const SKY_FRAG_GL2 = `#version 300 es
precision mediump float;
in vec2 vNdc;
out vec4 fragColor;

uniform vec3 uCamFwd;
uniform vec3 uCamRight;
uniform vec3 uCamUp;
uniform float uTanHalfFov;
uniform float uAspect;
uniform vec3 uSunDir;
uniform float uWarmth;
uniform float uShockFrac;

float hash21(vec2 p) {
  p = fract(p * vec2(123.34, 456.21));
  p += dot(p, p + 45.32);
  return fract(p.x * p.y);
}

float noise2(vec2 p) {
  vec2 i = floor(p);
  vec2 f = fract(p);
  vec2 u = f * f * (3.0 - 2.0 * f);
  float a = hash21(i);
  float b = hash21(i + vec2(1.0, 0.0));
  float c = hash21(i + vec2(0.0, 1.0));
  float d = hash21(i + vec2(1.0, 1.0));
  return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

float fbm3(vec2 p) {
  float v = 0.54 * noise2(p);
  p = p * 2.07 + vec2(1.7, -1.2);
  v += 0.29 * noise2(p);
  p = p * 2.11 + vec2(-2.3, 0.9);
  v += 0.17 * noise2(p);
  return v;
}

// Narkowicz ACES approximation — the same curve the world uses, so cloud
// highlights and lit facades roll off together instead of clipping separately.
vec3 aces(vec3 x) {
  return clamp((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14), 0.0, 1.0);
}

void main() {
  vec3 rd = normalize(
    uCamFwd
    + uCamRight * (vNdc.x * uAspect * uTanHalfFov)
    + uCamUp * (vNdc.y * uTanHalfFov)
  );

  float up = clamp(rd.y, 0.0, 1.0);
  float sunAmt = max(dot(rd, uSunDir), 0.0);

  float dayMix = smoothstep(0.18, 0.72, uWarmth);
  float duskMix = smoothstep(0.01, 0.36, uWarmth) * (1.0 - dayMix);
  float skyActive = clamp(dayMix + duskMix, 0.0, 1.0);
  float sunAbove = smoothstep(-0.05, 0.10, uSunDir.y);

  // ---------------------------------------------------------------------
  // Rayleigh-ish gradient. The three stops are the *linear* values read off
  // the reference frame: a cobalt zenith, a cerulean mid-sky and a humid
  // pale-cyan horizon band. Falling off with exp() (not smoothstep) keeps the
  // saturation high well up the sky, which is what makes an anime noon read
  // as "deep" instead of "washed".
  // ---------------------------------------------------------------------
  vec3 zenN  = vec3(0.008, 0.048, 0.235);
  vec3 midN  = vec3(0.030, 0.165, 0.500);
  vec3 horN  = vec3(0.145, 0.340, 0.545);
  vec3 zenD  = vec3(0.030, 0.048, 0.150);
  vec3 midD  = vec3(0.180, 0.105, 0.235);
  vec3 horD  = vec3(0.880, 0.330, 0.110);
  vec3 zenNt = vec3(0.004, 0.008, 0.024);
  vec3 midNt = vec3(0.011, 0.020, 0.048);
  vec3 horNt = vec3(0.024, 0.042, 0.082);

  vec3 zen = mix(zenNt, mix(zenD, zenN, dayMix), skyActive);
  vec3 mid = mix(midNt, mix(midD, midN, dayMix), skyActive);
  vec3 hor = mix(horNt, mix(horD, horN, dayMix), skyActive);

  vec3 sky = mix(mid, zen, smoothstep(0.10, 0.95, up));
  sky = mix(sky, hor, exp(-up * 15.0));

  // Stars, only when the sun is really down.
  if (uWarmth < 0.08 && rd.y > 0.08) {
    vec2 starCell = floor((rd.xz / (rd.y + 0.35)) * 140.0);
    float sh = hash21(starCell);
    if (sh > 0.992) {
      sky += vec3(0.75, 0.85, 1.0) * (sh - 0.992) * 60.0 * (1.0 - uWarmth * 12.0);
    }
  }

  // ---------------------------------------------------------------------
  // Sun: a tight disc, a warm aureole, and a very wide forward-scatter veil.
  // Three terms, because a single pow() always looks like a lens flare.
  // ---------------------------------------------------------------------
  vec3 sunTint = mix(vec3(1.10, 0.42, 0.15), vec3(1.20, 1.02, 0.80), dayMix);
  float disc = pow(sunAmt, 2400.0) * 6.0;
  float aureole = pow(sunAmt, 90.0) * 0.55;
  float veil = pow(sunAmt, 5.0) * 0.052;
  sky += sunTint * (disc + aureole + veil) * sunAbove;
  sky += vec3(0.55, 0.66, 0.85) * veil * 0.5 * skyActive;

  // ---------------------------------------------------------------------
  // Cumulonimbus on a horizontal cloud deck (proper perspective: the deck is
  // projected as rd.xz/rd.y, so distant towers converge at the horizon
  // instead of smearing like the old azimuth-space hack).
  // ---------------------------------------------------------------------
  vec3 cloudLit = mix(vec3(0.95, 0.48, 0.26), vec3(1.00, 0.98, 0.93), dayMix);
  vec3 cloudShade = mix(vec3(0.10, 0.09, 0.17), vec3(0.155, 0.245, 0.410), dayMix);
  vec3 cb = mix(vec3(0.020, 0.026, 0.048), cloudLit, skyActive);
  vec3 cs = mix(vec3(0.008, 0.010, 0.020), cloudShade, skyActive);

  if (rd.y > 0.008) {
    float hy = max(rd.y, 0.020);
    vec2 deck = rd.xz / hy;                       // cloud-deck coordinates
    vec2 q = deck * 0.95;
    // Domain warp: this is what turns fbm mush into discrete billows.
    vec2 warp = vec2(fbm3(q * 0.55 + vec2(3.1, 1.7)), fbm3(q * 0.55 + vec2(9.7, 4.3))) - 0.5;
    vec2 qw = q + warp * 2.2;

    // Three scales: the grouping of the towers, the billow shape, and a fine
    // octave used to ERODE the rim. Without the erosion every cloud has the
    // same soft edge and the whole deck reads as out-of-focus smoke.
    float mass = fbm3(qw * 0.30 + vec2(11.0, 5.0));
    float body = fbm3(qw * 0.92 + vec2(3.0, 8.0));
    float detail = fbm3(qw * 4.20 + vec2(21.0, 7.0));
    float field = mass * 0.68 + body * 0.34;
    field -= (1.0 - smoothstep(0.28, 0.78, body)) * 0.14;
    field += (detail - 0.5) * 0.30;
    float density = smoothstep(0.495, 0.550, field);
    density *= smoothstep(0.010, 0.075, rd.y);

    if (density > 0.004) {
      // Shadowing: compare the field with itself offset toward the sun. Cheap,
      // and it gives the "top of the tower glows, base goes blue" structure.
      vec2 sunStep = vec2(uSunDir.x, uSunDir.z) * 0.075 * (1.0 / max(uSunDir.y, 0.25));
      float lit = fbm3((qw + sunStep * 1.2) * 0.92 + vec2(3.0, 8.0));
      float thick = smoothstep(0.50, 0.86, field);
      float selfShade = clamp(0.62 + (body - lit) * 8.0 - thick * 0.30, 0.0, 1.0);
      // Low elevations see the flat, self-shadowed bases of the towers.
      selfShade *= mix(0.45, 1.0, smoothstep(0.02, 0.34, rd.y));
      vec3 cCol = mix(cs, cb, selfShade);

      // Silver lining where the billow thins toward the solar side.
      float lining = smoothstep(0.500, 0.545, field) * (1.0 - smoothstep(0.555, 0.640, field));
      cCol += cb * lining * 0.60 * pow(max(sunAmt, 0.0), 1.4);

      // Horizon haze eats the far cloud deck.
      vec3 hazeCol = mix(vec3(0.02, 0.03, 0.06), vec3(0.34, 0.50, 0.66), dayMix);
      cCol = mix(cCol, hazeCol, smoothstep(0.16, 0.02, rd.y) * 0.55);

      sky = mix(sky, cCol, density * 0.97);
    }
  }

  // Distant 343 m/s shockwave condensation ring expanding from far mountain Pad-1
  if (uShockFrac > 0.01 && uShockFrac < 1.15 && rd.z > 0.2) {
    float angDist = length(vec2(rd.x / max(rd.z, 0.2) - (-58.0 / 380.0), rd.y - 0.048));
    float ringR = uShockFrac * 0.34;
    float ring = exp(-pow((angDist - ringR) * 48.0, 2.0)) * (1.0 - uShockFrac * 0.78);
    sky += vec3(0.92, 0.95, 1.0) * max(ring, 0.0) * 0.24;
  }

  // ---------------------------------------------------------------------
  // Same filmic film as the world pass: ACES -> sRGB -> a touch of highlight
  // warm / shadow cool so sky and buildings grade together.
  // ---------------------------------------------------------------------
  vec3 mapped = aces(sky * 1.02);
  mapped = pow(mapped, vec3(1.0 / 2.2));
  float l = dot(mapped, vec3(0.2126, 0.7152, 0.0722));
  mapped += vec3(0.020, 0.014, -0.012) * (1.0 - smoothstep(0.0, 0.45, l));
  mapped = clamp(mix(vec3(l), mapped, 1.10), 0.0, 1.0);
  fragColor = vec4(mapped, 1.0);
}
`;

export const WORLD_VERT_GL2 = `#version 300 es
precision highp float;
layout(location = 0) in vec3 aPos;
layout(location = 1) in vec3 aNormal;
layout(location = 2) in vec4 aColorMat;

uniform mat4 uViewProj;

out vec3 vWorldPos;
out vec3 vNormal;
out vec3 vAlbedo;
flat out float vMat;

void main() {
  vWorldPos = aPos;
  vNormal = aNormal;
  vAlbedo = aColorMat.rgb;
  vMat = aColorMat.w;
  gl_Position = uViewProj * vec4(aPos, 1.0);
}
`;

export const WORLD_FRAG_GL2 = `#version 300 es
precision highp float;

in vec3 vWorldPos;
in vec3 vNormal;
in vec3 vAlbedo;
flat in float vMat;

out vec4 fragColor;

uniform vec3 uCamPos;
uniform vec3 uSunDir;
uniform float uWarmth;
uniform float uShojiClosed;
uniform float uEntryDoorOpen;
uniform float uLampOn;
uniform float uPlumeGlow;
uniform float uDebugMode;  // 0 = beauty; tools/screenshot.mjs passes ?debug=N for AOVs

// Analytical Window Sunbeam + Shoji Kumiko Lattice + Foreground Utility-Cable Gobos + Interior Wall Occlusion
vec2 computeIndoorSunMask(vec3 p) {
  if (uSunDir.z <= 0.04 || uSunDir.y <= 0.01) {
    return vec2(0.0, 0.0);
  }
  if (p.x < -0.36 && p.z < -0.28) {
    float tBath = (-0.27 - p.z) / uSunDir.z;
    if (tBath > 0.0) {
      vec2 hitBath = p.xy + uSunDir.xy * tBath;
      if (hitBath.x < -1.34 || hitBath.x > -0.60 || hitBath.y > 2.08) {
        return vec2(0.0, 0.0);
      }
    }
  }
  if (p.x > -0.36 && p.x < 0.36 && p.z < -1.20) {
    return vec2(0.0, 0.0);
  }
  float t = (1.97 - p.z) / uSunDir.z;
  if (t <= 0.0) {
    return vec2(1.0, 0.0);
  }
  vec2 hit = p.xy + uSunDir.xy * t;
  if (hit.x < -1.25 || hit.x > 1.25 || hit.y < 0.77 || hit.y > 2.26) {
    return vec2(0.0, 0.0);
  }
  if (abs(hit.x) < 0.024) {
    return vec2(0.0, 0.0);
  }

  float shojiWidth = 0.24 + clamp(uShojiClosed, 0.0, 1.0) * (2.50 - 0.24);
  float shojiLeftEdge = 1.25 - shojiWidth;

  if (hit.x >= shojiLeftEdge) {
    float gx = abs(fract((1.25 - hit.x) / 0.26) - 0.5);
    float gy = abs(fract((hit.y - 0.77) / 0.298) - 0.5);
    float barShadow = smoothstep(0.43, 0.47, max(gx, gy));
    return vec2(0.06 * (1.0 - barShadow), 0.52 * (1.0 - 0.42 * barShadow));
  } else {
    float catY1 = 1.94 - 0.13 * (1.0 - hit.x * hit.x * 0.35);
    float catY2 = 1.62 - 0.16 * (1.0 - hit.x * hit.x * 0.35);
    float w1 = 1.0 - 0.58 * exp(-pow((hit.y - catY1) * 60.0, 2.0));
    float w2 = 1.0 - 0.50 * exp(-pow((hit.y - catY2) * 55.0, 2.0));
    return vec2(w1 * w2, 0.05);
  }
}

float computeRoomAO(vec3 p, vec3 n) {
  float wx = smoothstep(0.0, 0.48, 2.54 - abs(p.x));
  float wz = smoothstep(0.0, 0.48, 2.03 - abs(p.z));
  float wyFloor = smoothstep(0.0, 0.30, p.y);
  float wyCeil  = smoothstep(0.0, 0.38, 2.68 - p.y);
  float cornerAO = (0.58 + 0.42 * wx * wz) * (0.64 + 0.36 * wyFloor) * (0.68 + 0.32 * wyCeil);
  float underDesk = (abs(p.x) < 1.14 && p.z > 1.26 && p.y < 0.68) ? 0.56 : 1.0;
  return cornerAO * underDesk * (0.86 + 0.14 * max(n.y, 0.0));
}

// High-Contrast Summer Noon Architectural Eave, Balcony, Stair & Dappled Foliage Gobo Shadows (image-18 & image-15!)
// Fine gobos only: dappled zelkova canopy where the trees actually stand, and
// the utility pole / overhead-wire stripes on the ground plane. No per-storey
// y-band "eave shadows" any more - those ignored the sun and read as dirt;
// the real eaves are now geometric boxes in streetSunVis().
float computeExteriorEaveShadow(vec3 p) {
  float g = 1.0;
  float treeZone = clamp(smoothstep(-3.9, -6.4, p.x) + smoothstep(3.2, 5.8, p.x), 0.0, 1.0);
  if (treeZone > 0.002) {
    float f = sin(p.x * 1.55 - p.y * 1.35 + p.z * 0.75) * cos(p.x * 0.95 + p.y * 1.75)
            + 0.48 * sin(p.x * 3.8 + p.y * 3.1 - p.z * 1.6);
    g *= 1.0 - 0.50 * smoothstep(-0.05, 0.55, f) * treeZone;
  }
  if (p.y < -2.88) {
    float poleLine = (p.x - 1.08) + (p.z - 5.35) * 0.62;
    g *= 1.0 - 0.55 * exp(-pow(poleLine * 7.5, 2.0));
    g *= 1.0 - 0.34 * exp(-pow((p.z - (3.88 - 0.11 * p.x)) * 24.0, 2.0));
    g *= 1.0 - 0.30 * exp(-pow((p.z - (6.68 + 0.08 * p.x)) * 20.0, 2.0));
    g *= 1.0 - 0.26 * exp(-pow((p.z - (7.14 + 0.08 * p.x)) * 22.0, 2.0));
  }
  return clamp(g, 0.0, 1.0);
}


// ===========================================================================
// SUMMER-NOON SURFACE & LIGHT ENGINE
// ---------------------------------------------------------------------------
// Everything below is analytic: no texture fetch, no shadow map, no UBO upload
// (PERFORMANCE.md I5/I7, VISION.md Law 5). Three ideas carry the look:
//
//  1. VALUE STRUCTURE FIRST. A single warm key (≈2.6× albedo) against a cool,
//     low sky fill (≈0.15×) gives the ~10:1 lit/shadow ratio the reference
//     frame lives on. Ambient is deliberately weak; depth comes from shadow,
//     not from global brightness.
//  2. MATERIAL MICRO-SURFACE. A triplanar height field (board seams, kawara
//     ridges, corrugation, plaster mottle, aggregate) modulates albedo *and*
//     tilts the normal from its analytic derivative, which is free.
//  3. ONE FILM. ACES → sRGB → a cool-shadow/warm-highlight grade, shared with
//     the sky pass so clouds and walls roll off together.
// ===========================================================================
float hash21(vec2 p) {
  p = fract(p * vec2(123.34, 456.21));
  p += dot(p, p + 45.32);
  return fract(p.x * p.y);
}

float vnoise(vec2 p) {
  vec2 i = floor(p);
  vec2 f = fract(p);
  vec2 u = f * f * (3.0 - 2.0 * f);
  float a = hash21(i);
  float b = hash21(i + vec2(1.0, 0.0));
  float c = hash21(i + vec2(0.0, 1.0));
  float d = hash21(i + vec2(1.0, 1.0));
  return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

float fbm2(vec2 p) {
  return 0.62 * vnoise(p) + 0.38 * vnoise(p * 2.13 + vec2(4.7, -2.1));
}

float fbm3s(vec2 p) {
  return 0.55 * vnoise(p) + 0.29 * vnoise(p * 2.07 + vec2(1.7, -1.2))
       + 0.16 * vnoise(p * 4.11 + vec2(-2.3, 0.9));
}

// Cheap triplanar basis: pick the dominant normal axis, return planar UV plus
// the two world tangents that correspond to it.
vec2 planarUV(vec3 p, vec3 n, out float major, out vec3 tU, out vec3 tV) {
  vec3 a = abs(n);
  if (a.y >= a.x && a.y >= a.z) {
    major = 1.0; tU = vec3(1.0, 0.0, 0.0); tV = vec3(0.0, 0.0, 1.0);
    return vec2(p.x, p.z);
  }
  if (a.x >= a.z) {
    major = 0.0; tU = vec3(0.0, 0.0, 1.0); tV = vec3(0.0, 1.0, 0.0);
    return vec2(p.z, p.y);
  }
  major = 2.0; tU = vec3(1.0, 0.0, 0.0); tV = vec3(0.0, 1.0, 0.0);
  return vec2(p.x, p.y);
}

vec3 aces(vec3 x) {
  return clamp((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14), 0.0, 1.0);
}


// ===========================================================================
// ANALYTIC SUN OCCLUSION
// ---------------------------------------------------------------------------
// The old model painted architectural shadow with hand-tuned x/y/z bands, so a
// facade was "in shade" whether or not the sun was actually behind its eave,
// and the sun direction could drift without the shadows following. Instead we
// ray-test the coarse machiya massing (the same volumes town.rs builds) against
// the *real* light vector. Five-ish slab tests, no shadow map, no depth
// texture, no extra draw call — and the shadows rotate correctly through the
// day (PERFORMANCE.md I7, VISION.md Law 6).
//
// Boxes are intentionally coarse: an eave is one slab, a house is one volume.
// Detail inside a shaded band comes from the gobo function below, not from more
// boxes.
// ===========================================================================
float boxOcclusion(vec3 p, vec3 L, vec3 bmin, vec3 bmax) {
  // Points on (or inside) a box are never shadowed by it.
  if ((p.x > bmin.x && p.x < bmax.x && p.y > bmin.y && p.y < bmax.y &&
       p.z > bmin.z && p.z < bmax.z)) {
    return 0.0;
  }
  vec3 ro = p + L * 0.03;
  vec3 Ls = sign(L) * max(abs(L), vec3(1e-5));
  vec3 inv = 1.0 / Ls;
  vec3 t0 = (bmin - ro) * inv;
  vec3 t1 = (bmax - ro) * inv;
  vec3 tn3 = min(t0, t1);
  vec3 tf3 = max(t0, t1);
  float tn = max(max(tn3.x, tn3.y), tn3.z);
  float tf = min(min(tf3.x, tf3.y), tf3.z);
  return (tf > max(tn, 0.0)) ? 1.0 : 0.0;
}

// Coarse street massing, ±Z machiya rows and their eaves (see town.rs §2A/2B).
// Coarse street massing, +-Z machiya rows and their eaves. Boxes come straight
// from town.rs (near row: facade z = 2.22, bodies extend to -Z; far row:
// facades z = 11.45 / 11.75, bodies extend to +Z). Anything that is not a
// building or roof slab is left to the fine gobo, so shadow direction follows
// the art sun instead of being painted on.
float streetSunVis(vec3 p, vec3 L) {
  if (p.y > 3.9 || abs(p.x) > 34.0 || p.z < -12.0 || p.z > 30.0) return 1.0;
  float occ = 0.0;
  // ---- near row: bodies (facades at z = 2.22) ----
  occ = max(occ, boxOcclusion(p, L, vec3(-3.10, -3.10, -4.30), vec3(3.10, 2.56, 2.24)));   // Uchinoura Maison
  occ = max(occ, boxOcclusion(p, L, vec3(-13.30, -3.10, -4.20), vec3(-5.70, 2.36, 2.24))); // House #3
  occ = max(occ, boxOcclusion(p, L, vec3(2.80, -3.10, -2.52), vec3(8.50, 3.00, 2.44)));    // corrugated annex
  // ---- near row: eaves / hisashi slabs (thin, they only shade what is below) ----
  occ = max(occ, boxOcclusion(p, L, vec3(-3.20, 2.30, 2.10), vec3(3.20, 2.58, 3.42)));     // 2F kawara eave
  occ = max(occ, boxOcclusion(p, L, vec3(-3.20, -0.30, 2.10), vec3(3.20, -0.12, 3.30)));   // 1F hisashi
  occ = max(occ, boxOcclusion(p, L, vec3(-5.52, 1.32, -0.38), vec3(-2.60, 2.18, 1.82)));   // gallery canopy
  occ = max(occ, boxOcclusion(p, L, vec3(-13.40, 2.30, 2.10), vec3(-5.60, 2.62, 2.62)));   // House #3 eave
  // ---- far row: bodies (facades at z = 11.45 / 11.75) ----
  occ = max(occ, boxOcclusion(p, L, vec3(-0.30, -3.10, 11.42), vec3(7.80, 2.34, 18.25)));  // House #1
  occ = max(occ, boxOcclusion(p, L, vec3(-10.95, -3.10, 11.72), vec3(-3.55, 2.34, 18.35))); // House #2
  occ = max(occ, boxOcclusion(p, L, vec3(8.05, -3.10, 10.85), vec3(13.80, 3.30, 17.20)));  // far annex
  // ---- far row: eaves overhanging the street (-Z side) ----
  occ = max(occ, boxOcclusion(p, L, vec3(-0.40, 2.28, 10.55), vec3(7.90, 2.56, 18.25)));   // House #1 2F eave
  occ = max(occ, boxOcclusion(p, L, vec3(-0.40, -0.32, 10.50), vec3(7.90, -0.14, 18.25))); // House #1 1F hisashi
  occ = max(occ, boxOcclusion(p, L, vec3(-11.05, 2.28, 10.85), vec3(-3.45, 2.56, 18.35))); // House #2 2F eave
  occ = max(occ, boxOcclusion(p, L, vec3(-11.05, -0.32, 10.80), vec3(-3.45, -0.14, 18.35)));// House #2 1F hisashi
  return 1.0 - occ;
}


void main() {
  vec3 N0 = normalize(gl_FrontFacing ? vNormal : -vNormal);
  vec3 V = normalize(uCamPos - vWorldPos);
  int mat = int(vMat + 0.5);
  vec3 baseCol = vAlbedo;

  // ---- Emissive / special materials -------------------------------------
  if (mat == 6) {
    // Warm 2700 K paper lantern / andon. Slight paper fibre mottle so it reads
    // as washi rather than a flat glowing cylinder.
    vec2 luv = vec2(vWorldPos.x + vWorldPos.z, vWorldPos.y);
    float fibre = fbm2(luv * 26.0);
    float wrapDark = 0.86 + 0.14 * fibre;
    vec3 em = baseCol * wrapDark * 1.55 + vec3(0.30, 0.16, 0.05);
    vec3 e = aces(em);
    fragColor = vec4(pow(e, vec3(1.0 / 2.2)), 1.0);
    return;
  }
  if (mat == 8) {
    if (vWorldPos.x > 0.72 && vWorldPos.x < 1.02 && vWorldPos.z > 1.42 && vWorldPos.z < 1.46) {
      vec2 uv = vec2((vWorldPos.x - 0.745) / 0.25, (vWorldPos.y - 0.833) / 0.21);
      float gridX = smoothstep(0.46, 0.50, abs(fract(uv.x * 6.0) - 0.5));
      float gridY = smoothstep(0.46, 0.50, abs(fract(uv.y * 4.0) - 0.5));
      float scan = 0.90 + 0.10 * sin(vWorldPos.y * 520.0);
      float tr1 = exp(-abs((uv.y - 0.58) - 0.16 * sin(uv.x * 18.8)) * 52.0);
      float tr2 = exp(-abs((uv.y - 0.30) - 0.07 * cos(uv.x * 31.4)) * 60.0);
      vec3 crt = vec3(0.015, 0.09, 0.10) * scan
        + vec3(0.05, 0.26, 0.25) * max(gridX, gridY)
        + vec3(0.32, 0.96, 0.84) * tr1 * 1.05
        + vec3(0.96, 0.72, 0.28) * tr2 * 0.70;
      fragColor = vec4(clamp(crt, 0.0, 1.0), 1.0);
      return;
    }
    float scan = 0.88 + 0.12 * sin(vWorldPos.y * 420.0);
    fragColor = vec4(clamp(baseCol * scan, 0.0, 1.0), 1.0);
    return;
  }
  if (mat == 9) {
    float rim = 0.78 + 0.22 * max(N0.y, 0.0);
    fragColor = vec4(clamp(baseCol * rim * 1.12, 0.0, 1.0), 1.0);
    return;
  }
  if (mat == 10) {
    float pulse = 0.82 + 0.18 * sin((vWorldPos.x + vWorldPos.y + vWorldPos.z) * 18.0);
    fragColor = vec4(clamp(baseCol * pulse, 0.0, 1.0), 1.0);
    return;
  }

  bool indoor = (vWorldPos.z < 1.995 && vWorldPos.z > -2.10 && abs(vWorldPos.x) < 2.58
                 && vWorldPos.y > -0.06 && vWorldPos.y < 2.695);
  float dist = length(uCamPos - vWorldPos);
  float detailFade = 1.0 - smoothstep(9.0, 30.0, dist);

  // ---- Triplanar micro-surface ------------------------------------------
  float major;
  vec3 tU, tV;
  vec2 uv = planarUV(vWorldPos, N0, major, tU, tV);
  vec2 dH = vec2(0.0);          // height gradient in UV space
  float bumpAmt = 0.0;
  float rough = 0.62;

  if (mat == 0) {
    // Plaster / concrete / asphalt all share MAT_MATTE and are told apart by
    // world height and by how flat the surface is.
    bool ground = (!indoor && vWorldPos.y < -2.95);
    if (ground) {
      // Weathered asphalt & poured sidewalk aggregate.
      float agg = fbm3s(uv * 26.0);
      baseCol *= (0.78 + 0.24 * agg);
      float patchy = fbm2(uv * 0.55 + 3.0);
      baseCol *= 0.92 + 0.16 * patchy;
      // Slab joints on the sidewalk, 1.4 m on centre.
      if (vWorldPos.y > -3.02) {
        float jx = abs(fract(uv.x / 1.4) - 0.5);
        float jz = abs(fract(uv.y / 1.4) - 0.5);
        float joint = 1.0 - smoothstep(0.48, 0.5, max(jx, jz));
        baseCol *= 1.0 - 0.30 * joint;
        dH += vec2(jx, jz) * joint * 2.0;
      }
      dH += (vec2(vnoise(uv * 26.0), vnoise(uv * 26.0 + 7.0)) - 0.5) * 0.35;
      rough = 0.86;
      bumpAmt = 0.05 * detailFade;
    } else if (indoor) {
      float trowel = fbm2(uv * 5.0 + 11.0);
      baseCol *= 0.97 + 0.06 * trowel;
      rough = 0.74;
      bumpAmt = 0.02 * detailFade;
    } else {
      // Aged sand-lime plaster: coarse mottling, long vertical rain streaks,
      // drip stains under the eaves and rising damp at the plinth.
      float mottle = fbm3s(uv * 1.25 + 4.0);
      float coarse = fbm2(uv * 0.42 + 21.0);
      baseCol *= 0.82 + 0.30 * mottle + 0.14 * coarse;
      baseCol *= vec3(0.94, 0.92, 0.87);          // warm cream, not white paper

      float streakU = (vWorldPos.x + vWorldPos.z) * 6.0;
      float streak = fbm2(vec2(streakU, vWorldPos.y * 0.22 + 5.0));
      baseCol *= 1.0 - 0.26 * smoothstep(0.46, 0.80, streak);

      // Drip staining directly under each storey's eave line.
      float underEave = fract((vWorldPos.y + 3.06) / 3.02);
      baseCol *= mix(0.62, 1.0, smoothstep(0.0, 0.16, underEave));

      float damp = 1.0 - smoothstep(-3.02, -2.10, vWorldPos.y);
      baseCol *= mix(1.0, 0.60, damp * damp);
      baseCol = mix(baseCol, baseCol * vec3(0.86, 0.96, 0.90), damp * 0.7);

      // Sparse hairline cracks / render patch seams.
      float crack = smoothstep(0.485, 0.5, abs(fract(fbm2(uv * 0.9) * 6.0) - 0.5));
      baseCol *= 1.0 - 0.22 * crack * (0.4 + 0.6 * detailFade);

      dH += vec2(vnoise(uv * 9.0), vnoise(uv * 9.0 + 3.0)) * (mottle - 0.5) * 1.1;
      rough = 0.86;
      bumpAmt = 0.06 * detailFade;
    }
  } else if (mat == 1) {
    // Roasted cedar / hinoki boards. Walls get vertical boards (shouji-ita),
    // floors and ceilings get boards running along +X.
    bool wall = (major != 1.0);
    float across = wall ? uv.x : uv.y;
    float along = wall ? uv.y : uv.x;
    float bw = wall ? 0.185 : 0.30;
    float boardF = across / bw;
    float bi = floor(boardF);
    float f = fract(boardF);
    float tone = hash21(vec2(bi, wall ? 3.0 : 17.0)) - 0.5;
    float seam = smoothstep(0.0, 0.055, f) * smoothstep(1.0, 0.945, f);

    // Grain streaks run along the board: high frequency across, low along.
    float grain = fbm3s(vec2(along * 1.7, across * 34.0) + bi * 7.3);

    baseCol *= vec3(0.90, 0.80, 0.72);          // palette: darker, redder cedar
    baseCol *= (1.0 + tone * 0.26) * (0.46 + 0.54 * seam);
    baseCol *= 0.84 + 0.30 * grain;
    // Long silvered weathering streaks: sun-greyed in the middle of a board,
    // dark where water runs at the seams.
    float weather = fbm2(vec2(along * 0.9, across * 6.0) + 31.0);
    baseCol *= 0.90 + 0.20 * weather;
    // Sun-bleaching and warm red shift where the timber faces the sky.
    baseCol = mix(baseCol, baseCol * vec3(1.14, 1.03, 0.86), clamp(N0.y, 0.0, 1.0) * 0.55);
    baseCol *= 0.94 + 0.12 * smoothstep(-3.0, 2.4, vWorldPos.y);

    // Analytic sawtooth derivative -> board edges catch the sun.
    float dhAcross = -sin(f * 6.28318) * 0.9;
    if (wall) dH.x = dhAcross * 1.0; else dH.y = dhAcross * 1.0;
    rough = 0.52;
    bumpAmt = 0.075 * detailFade;
  } else if (mat == 2) {
    // Igusa tatami: rush weave, fine across, coarse along.
    float weave = sin(uv.x * 210.0) * 0.5 + sin(uv.y * 22.0) * 0.5;
    baseCol *= 1.0 + weave * 0.05;
    float band = smoothstep(0.02, 0.08, abs(fract(uv.y / 1.79) - 0.5));
    baseCol *= 0.90 + 0.10 * band;
    rough = 0.72;
    bumpAmt = 0.02 * detailFade;
  } else if (mat == 3) {
    // Galvanised corrugated siding: 80 mm ribs, rust bloom at the fixings.
    float rib = sin(uv.x * 78.0);
    float rust = smoothstep(0.62, 1.02, fbm2(uv * vec2(2.2, 1.1) + 9.0));
    float rustH = 1.0 - smoothstep(-3.05, -1.4, vWorldPos.y);
    baseCol *= vec3(0.74, 0.84, 1.0);            // galvanised blue-grey
    baseCol *= 0.74 + 0.30 * rib;
    baseCol = mix(baseCol, vec3(0.22, 0.135, 0.085), rust * (0.16 + 0.28 * rustH));
    baseCol *= 0.92 + 0.16 * fbm2(vec2(uv.x * 3.0, uv.y * 0.6));
    dH.x = cos(uv.x * 78.0) * 0.55;
    rough = 0.40;
    bumpAmt = 0.09 * detailFade;
  } else if (mat == 4) {
    float brush = sin((uv.x + uv.y * 2.0) * 120.0) * 0.5 + fbm2(uv * 9.0) * 0.5;
    baseCol *= 0.96 + brush * 0.06;
    rough = 0.26;
  } else if (mat == 5) {
    // Washi: long kozo fibres, warm transparency.
    float fibre = fbm2(vec2(uv.x * 6.0, uv.y * 40.0));
    baseCol *= 0.97 + 0.07 * fibre;
    rough = 0.88;
  } else if (mat == 11) {
    if (indoor && vWorldPos.x < -0.36 && vWorldPos.z < -0.24) {
      float gx = smoothstep(0.018, 0.032, abs(fract(vWorldPos.x * 4.0) - 0.5));
      float gy = smoothstep(0.018, 0.032, abs(fract(vWorldPos.y * 4.0) - 0.5));
      float gz = smoothstep(0.018, 0.032, abs(fract(vWorldPos.z * 4.0) - 0.5));
      baseCol *= 0.90 + 0.10 * (gx * gy * gz);
    }
    baseCol *= 0.96 + 0.08 * fbm2(uv * 30.0);
    rough = 0.18;
  } else if (mat == 12) {
    // Foliage: two-tone leaf clusters; the tip colour shift is done in lighting
    // (subsurface) so grass and canopy share one model.
    float leafVar = fbm3s(vec2(uv.x * 7.0, uv.y * 9.0));
    baseCol *= 0.72 + 0.55 * leafVar;
    baseCol = mix(baseCol, baseCol * vec3(1.20, 1.10, 0.72), smoothstep(0.55, 1.0, leafVar) * 0.55);
    rough = 0.66;
  } else if (mat == 13) {
    // Kawara: rounded sangawara ridges down the slope, stepped courses across.
    float ridgePhase = uv.x * 20.5;
    float ridge = sin(ridgePhase);
    if (major == 1.0) {
      // Roof plane: ridges run along Z, courses along X.
      ridgePhase = vWorldPos.x * 20.5;
      ridge = sin(ridgePhase);
      float course = fract(vWorldPos.z * 3.4);
      float courseShade = 0.78 + 0.22 * smoothstep(0.0, 0.22, course);
      baseCol *= (0.82 + 0.22 * ridge) * courseShade;
      dH = vec2(cos(ridgePhase) * 0.75, (0.5 - course) * 0.35);
    } else {
      baseCol *= 0.86 + 0.18 * ridge;
      dH.x = cos(ridgePhase) * 0.75;
    }
    float tileTone = hash21(floor(vec2(uv.x * 3.2, uv.y * 3.2)));
    baseCol *= (0.92 + 0.16 * tileTone) * 0.90;
    rough = 0.30;
    bumpAmt = 0.10 * (0.35 + 0.65 * detailFade);
  }

  // Apply the analytic bump.
  vec3 N = normalize(N0 - (dH.x * tU + dH.y * tV) * bumpAmt);

  // =========================================================================
  // LIGHT STRUCTURE
  // =========================================================================
  float dayMix = smoothstep(0.14, 0.70, uWarmth);
  float duskMix = smoothstep(0.01, 0.35, uWarmth) * (1.0 - dayMix);
  vec3 sunColor = mix(vec3(1.34, 0.46, 0.15), vec3(1.60, 1.24, 0.86), dayMix)
                * smoothstep(-0.02, 0.10, uSunDir.y);

  vec3 dbgL = uSunDir;
  vec3 directLight = vec3(0.0);
  vec3 ambient = vec3(0.0);
  float ao = 1.0;
  float shadowVis = 1.0;
  vec2 sunMask = vec2(1.0, 0.0);

  if (indoor) {
    sunMask = computeIndoorSunMask(vWorldPos);
    shadowVis = sunMask.x;
    ao = computeRoomAO(vWorldPos, N);
    float ndl = max(dot(N, uSunDir), 0.0);
    float wrap = clamp((ndl + 0.10) / 1.10, 0.0, 1.0);
    directLight = sunColor * 3.20 * wrap * shadowVis;

    // Cool window-driven sky fill + warm timber bounce, both occluded.
    float shojiDim = 1.0 - 0.38 * uShojiClosed;
    vec3 skyFill = mix(vec3(0.075, 0.090, 0.135), vec3(0.215, 0.265, 0.395), dayMix);
    vec3 bounceFill = mix(vec3(0.048, 0.034, 0.025), vec3(0.270, 0.195, 0.118), dayMix);
    float upness = 0.5 + 0.5 * N.y;
    ambient = (mix(bounceFill, skyFill, upness) * (0.55 + 0.45 * (0.5 + 0.5 * N.y))
              + vec3(0.90, 0.72, 0.44) * (sunMask.y * 0.30)) * ao * shojiDim;
    ambient *= mix(0.78, 1.0, shadowVis);
  } else {
    // One coherent "art sun" that keeps the raking angle of the reference even
    // when the ephemeris puts the sun overhead at solar noon: the *elevation*
    // is compressed, the azimuth (which side of the street is lit) is real.
    // The street runs along X, so a sun that is merely *above* the ridge line
    // drops the whole canyon into shade. Bias the art sun toward the street
    // axis (-X) so light rakes along the facades the way it does in the
    // reference frame, while keeping the real azimuth's sign and the real
    // elevation bracketed. The interior keeps the true ephemeris vector.
    // The art sun is raking (azimuth biased along the street, -X) and its
    // Z-lean follows the surface: a facade that faces +Z is lit from +Z, one
    // that faces -Z from -Z, and ground/roofs pick the row they are nearest to.
    // Blending two opposite unit vectors would cancel the Z component and put
    // an almost-vertical sun over a north-south street, which is what made the
    // whole canyon read as night. Keep |z| fixed and only flip its sign.
    float zLean;
    if (abs(N0.z) > 0.25) {
      zLean = sign(N0.z);
    } else {
      zLean = mix(1.0, -1.0, smoothstep(5.2, 7.6, vWorldPos.z));
    }
    float sunElev = clamp(uSunDir.y, 0.40, 0.60);
    vec3 nearSun = normalize(vec3(-0.66, sunElev, 0.56));
    vec3 farSun  = normalize(vec3(-0.66, sunElev, -0.56));
    vec3 artSun  = normalize(vec3(-0.66, sunElev, 0.56 * zLean));
    // Which of the two "art suns" a surface sees is chosen by its *normal*,
    // not by where it is: a facade that faces +Z is lit by the +Z sun, one that
    // faces -Z by the -Z sun, and a roof (N.z ~ 0) gets the blend, i.e. a
    // raking side light. That keeps both sides of a narrow street sunlit —
    // physically impossible with one sun, but it is exactly what the reference
    // frame does, and it removes the "half the street is night" failure.
    vec3 L = artSun;
    dbgL = L;
    // Real eave/building occlusion from the massing, then the fine gobos
    // (foliage dapple, overhead wires, poles, stair treads) on top.
    shadowVis = streetSunVis(vWorldPos, L) * computeExteriorEaveShadow(vWorldPos);
    shadowVis = clamp(shadowVis, 0.0, 1.0);

    float ndl = max(dot(N, L), 0.0);
    float wrap = clamp((ndl + 0.03) / 1.03, 0.0, 1.0);
    directLight = sunColor * 1.30 * wrap * shadowVis;

    // Ground proximity AO on the street + a soft soffit term so eaves and the
    // underside of balconies go genuinely dark.
    float streetAO = (abs(vWorldPos.z) < 26.0)
      ? (0.52 + 0.48 * smoothstep(-3.06, -2.30, vWorldPos.y)) : 1.0;
    float soffit = mix(0.55, 1.0, smoothstep(-0.35, 0.55, N.y));
    ao = streetAO * soffit;

    float upness = clamp(N.y * 0.5 + 0.5, 0.0, 1.0);
    vec3 skyFill = mix(vec3(0.130, 0.170, 0.310), vec3(0.400, 0.500, 0.780), dayMix);
    vec3 groundFill = mix(vec3(0.070, 0.058, 0.048), vec3(0.240, 0.200, 0.150), dayMix);
    ambient = mix(groundFill, skyFill, upness) * ao;
    // Shade is dimmer than sun, but it is still *sky*, so keep most of the fill:
    // the reference's shaded timber is a deep blue-brown, never black.
    ambient *= mix(0.80, 1.0, shadowVis);
    // Down-facing soffits look at the sunlit street, not at the sky: without
    // this fill every eave underside renders as a flat black bar.
    ambient = mix(ambient, ambient * 1.85 + groundFill * 0.55 * dayMix,
                 smoothstep(-0.25, -0.75, N.y));

    // Backlit foliage / grass translucency: the reference's grass is a
    // different colour from its shadowed neighbours because the sun is *in it*.
    if (mat == 12) {
      float thin = pow(clamp(1.0 - abs(dot(N, V)), 0.0, 1.0), 1.2);
      float scatter = 0.42 + 0.58 * thin;
      // Sun *inside* the blade (subsurface), plus a green ambient floor so
      // grass in street shade still reads as grass instead of black wire.
      directLight += vec3(0.62, 0.95, 0.14) * scatter * dayMix * (0.30 + 0.70 * shadowVis) * 0.72;
      directLight += vec3(0.98, 0.94, 0.40) * max(N.y, 0.0) * shadowVis * dayMix * 0.22;
      ambient += vec3(0.052, 0.112, 0.018) * dayMix * ao * (0.55 + 0.45 * shadowVis);
    }
  }

  // Translucent washi / frosted glass self-glow when backlit.
  if (mat == 5) {
    float backLit = max(dot(-N, uSunDir), 0.30) * smoothstep(0.0, 0.35, uWarmth);
    directLight += sunColor * backLit * (indoor ? 0.30 : 0.18);
  }

  // ---- Specular: tight on glazed kawara and alloy, broad on plaster ------
  vec3 extArtSun = normalize(vec3(-0.66, clamp(uSunDir.y, 0.40, 0.60),
      0.56 * (abs(N0.z) > 0.25 ? sign(N0.z)
              : mix(1.0, -1.0, smoothstep(5.2, 7.6, vWorldPos.z)))));
  vec3 H = normalize((indoor ? uSunDir : extArtSun) + V);
  float specPow = mix(160.0, 12.0, rough);
  float spec = pow(max(dot(N, H), 0.0), specPow) * (1.0 - rough) * shadowVis;
  vec3 specLight = sunColor * spec * (mat == 13 ? 0.55 : 0.16);

  // ---- Cool sky rim, only where the sun is not ---------------------------
  float rim = pow(clamp(1.0 - abs(dot(N, V)), 0.0, 1.0), 3.0);
  vec3 rimLight = mix(vec3(0.010, 0.014, 0.028), vec3(0.045, 0.070, 0.125), dayMix)
                * rim * (1.0 - shadowVis) * 0.55;

  vec3 litColor = baseCol * (ambient + directLight) + specLight + rimLight;

  // ---- Interior sunbeam shafts -------------------------------------------
  if (indoor && dayMix > 0.25 && uSunDir.z > 0.08) {
    vec3 ray = vWorldPos - uCamPos;
    float rayLen = clamp(length(ray), 0.0, 4.5);
    vec2 m1 = computeIndoorSunMask(uCamPos + ray * 0.38);
    vec2 m2 = computeIndoorSunMask(uCamPos + ray * 0.72);
    float shaft = (m1.x + m2.x) * 0.5;
    litColor += vec3(1.0, 0.86, 0.56) * shaft * rayLen * 0.030 * dayMix * (1.0 - 0.82 * uShojiClosed);
  }

  // ---- Aerial perspective ------------------------------------------------
  if (!indoor && abs(vWorldPos.z) > 30.0) {
    float haze = 1.0 - exp(-(abs(vWorldPos.z) - 22.0) * 0.0090);
    vec3 hazeCol = mix(vec3(0.016, 0.026, 0.060), vec3(0.150, 0.280, 0.430), dayMix);
    hazeCol += vec3(1.0, 0.74, 0.40) * uPlumeGlow * 0.12;
    litColor = mix(litColor, hazeCol, clamp(haze, 0.0, 0.72));
  }

  // ---- One film -----------------------------------------------------------
  // Reference frame is a low-key image: mean luma 0.29, a quarter of it below
  // 0.09 and almost nothing above 0.70. Shoot for that, not for a bright
  // product render, then add a gentle S so the shaded half stays "ink".
  vec3 mapped = aces(max(litColor, vec3(0.0)) * 0.52);
  mapped = pow(mapped, vec3(1.0 / 2.2));
  mapped = pow(mapped, vec3(1.10));
  mapped = mix(mapped, mapped * mapped * (3.0 - 2.0 * mapped), 0.22);

  float luma = dot(mapped, vec3(0.2126, 0.7152, 0.0722));
  // Vibrance: push saturated mids harder than neutrals, then a little global
  // chroma so foliage and tiles read as pigment rather than grey plastic.
  float sat = max(max(mapped.r, mapped.g), mapped.b) - min(min(mapped.r, mapped.g), mapped.b);
  mapped = mix(vec3(luma), mapped, 1.08 + 0.62 * (1.0 - sat));
  // Split tone: cool violet shade, warm amber light — the two-temperature
  // grade the reference frame is built on.
  mapped += vec3(-0.004, 0.010, 0.030) * (1.0 - smoothstep(0.0, 0.45, luma));
  mapped *= mix(vec3(0.975, 0.990, 1.030), vec3(1.045, 1.005, 0.945), smoothstep(0.30, 1.0, luma));
  // Lifted black point. ACES has a quadratic toe, so an eave soffit lit only by
  // sky fill lands at 0.01 and reads as a hole punched in the wall. Print film
  // and the reference frame both sit their blacks near 0.03.
  mapped = mapped * 0.975 + vec3(0.012, 0.011, 0.014);
  mapped = clamp(mapped, 0.0, 1.0);
  fragColor = vec4(mapped, 1.0);

  // Dev-only AOVs for art iteration (never set in normal play).
  if (uDebugMode > 0.5) {
    vec3 dbg;
    if (uDebugMode < 1.5)       dbg = vec3(shadowVis);                                   // sun visibility
    else if (uDebugMode < 2.5)  dbg = vec3(ao);                                          // ambient occlusion
    else if (uDebugMode < 3.5)  dbg = vec3(float(mat) / 13.0);                           // material id
    else if (uDebugMode < 4.5)  dbg = vec3(directLight / (directLight + ambient + 1e-4)); // sun share
    else if (uDebugMode < 5.5)  dbg = vec3(baseCol.r, baseCol.g, baseCol.b);             // albedo × detail
    else if (uDebugMode < 6.5)  dbg = N0 * 0.5 + 0.5;                                     // world normal
    else if (uDebugMode < 7.5)  dbg = vec3(streetSunVis(vWorldPos, dbgL));                // box occlusion only
    else if (uDebugMode < 8.5)  dbg = vec3(computeExteriorEaveShadow(vWorldPos));         // gobos only
    else                        dbg = dbgL * 0.5 + 0.5;                                   // art sun vector
    fragColor = vec4(dbg, 1.0);
  }
}
`;
