// Analytical WebGL2 Shaders for Odderon Checkpoint 1A v3 ("Sunlit Japanese Street & Hab #204").
// Zero texture files (0 bytes). All Kawara clay tile ridges, eave & catenary shadows, Shoji Kumiko gobos,
// warm Shikkui plaster bounce, ceramic glaze, tatami weave, CRT graticule, and summer clouds are pure math.

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

float fbm4(vec2 p) {
  float v = 0.50 * noise2(p);
  p = p * 2.04 + vec2(1.7, -1.2);
  v += 0.26 * noise2(p);
  p = p * 2.07 + vec2(-2.3, 0.9);
  v += 0.15 * noise2(p);
  p = p * 2.03 + vec2(0.8, 1.9);
  v += 0.09 * noise2(p);
  return v;
}

void main() {
  vec3 rd = normalize(
    uCamFwd
    + uCamRight * (vNdc.x * uAspect * uTanHalfFov)
    + uCamUp * (vNdc.y * uTanHalfFov)
  );

  float up = clamp(rd.y, 0.0, 1.0);
  float sunAmt = max(dot(rd, uSunDir), 0.0);

  // Warm Vivid Japanese Summer Sky Palette (matching stylized Japanese street reference!)
  vec3 noonZenith  = vec3(0.045, 0.24, 0.68);
  vec3 noonMid     = vec3(0.14,  0.46, 0.86);
  vec3 noonHorizon = vec3(0.52,  0.78, 0.96);

  vec3 duskZenith  = vec3(0.05, 0.09, 0.26);
  vec3 duskMid     = vec3(0.36, 0.20, 0.36);
  vec3 duskHorizon = vec3(1.18, 0.42, 0.14);

  vec3 nightZenith = vec3(0.008, 0.015, 0.036);
  vec3 nightMid    = vec3(0.016, 0.030, 0.068);
  vec3 nightHoriz  = vec3(0.035, 0.060, 0.115);

  float dayMix = smoothstep(0.18, 0.72, uWarmth);
  float duskMix = smoothstep(0.01, 0.36, uWarmth) * (1.0 - dayMix);
  float skyActive = clamp(dayMix + duskMix, 0.0, 1.0);

  vec3 zenCol = mix(nightZenith, mix(duskZenith, noonZenith, dayMix), skyActive);
  vec3 midCol = mix(nightMid,    mix(duskMid,    noonMid,    dayMix), skyActive);
  vec3 horCol = mix(nightHoriz,  mix(duskHorizon, noonHorizon, dayMix), skyActive);

  vec3 sky = mix(midCol, zenCol, smoothstep(0.08, 0.68, up));
  sky = mix(sky, horCol, pow(1.0 - up, 4.6));

  if (uWarmth < 0.08 && rd.y > 0.08) {
    vec2 starCell = floor((rd.xz / (rd.y + 0.35)) * 140.0);
    float sh = hash21(starCell);
    if (sh > 0.992) {
      sky += vec3(0.75, 0.85, 1.0) * (sh - 0.992) * 95.0 * (1.0 - uWarmth * 12.0);
    }
  }

  vec3 sunTint = mix(vec3(1.25, 0.46, 0.16), vec3(1.36, 1.24, 0.98), dayMix);
  sky += sunTint * (0.18 * pow(sunAmt, 7.0) + 1.75 * pow(sunAmt, 340.0)) * smoothstep(-0.04, 0.08, uSunDir.y);

  // Fluffy Sunlit Japanese Summer Cumulus & Cumulonimbus Clouds over the Mountains (+Z)
  if (rd.y > 0.02 && rd.z > 0.08) {
    vec2 uv = vec2(rd.x / max(rd.z, 0.28), rd.y * 1.95);
    float c1 = 1.0 - length((uv - vec2(-0.36, 0.38)) * vec2(1.30, 1.65));
    float c2 = 1.0 - length((uv - vec2( 0.04, 0.44)) * vec2(1.18, 1.50));
    float c3 = 1.0 - length((uv - vec2( 0.42, 0.34)) * vec2(1.30, 1.75));
    float silhouette = max(max(c1, c2), c3);
    float n = fbm4(uv * 6.0);
    float detail = fbm4(uv * 14.0 + vec2(3.1, -1.7));
    float field = silhouette + 0.38 * n + 0.14 * detail;
    float density = smoothstep(0.43, 0.60, field) * smoothstep(0.03, 0.10, rd.y);

    if (density > 0.01) {
      vec2 sunStep = normalize(vec2(uSunDir.x, max(uSunDir.y, 0.18))) * 0.038;
      float nLit = fbm4((uv + sunStep) * 6.0);
      float topShade = clamp(0.55 + (n - nLit) * 3.0 + (rd.y - 0.10) * 1.35, 0.0, 1.0);
      vec3 cloudBright = mix(vec3(1.28, 0.66, 0.38), vec3(1.42, 1.38, 1.30), dayMix);
      vec3 cloudShadow = mix(vec3(0.22, 0.18, 0.32), vec3(0.36, 0.50, 0.72), dayMix);
      vec3 cb = mix(vec3(0.07, 0.10, 0.17), cloudBright, skyActive);
      vec3 cs = mix(vec3(0.025, 0.04, 0.08), cloudShadow, skyActive);
      vec3 cCol = mix(cs, cb, smoothstep(0.15, 0.85, topShade));
      sky = mix(sky, cCol, density * 0.96);
    }
  }

  // Distant 343 m/s shockwave condensation ring expanding from far mountain Pad-1 (x=2.2, y=14.5, z=360)
  if (uShockFrac > 0.01 && uShockFrac < 1.15 && rd.z > 0.2) {
    float angDist = length(vec2(rd.x / max(rd.z, 0.2) - (2.2 / 360.0), rd.y - 0.042));
    float ringR = uShockFrac * 0.36;
    float ring = exp(-pow((angDist - ringR) * 48.0, 2.0)) * (1.0 - uShockFrac * 0.78);
    sky += vec3(0.95, 0.96, 1.0) * max(ring, 0.0) * 0.26;
  }

  vec3 mapped = (sky * (2.51 * sky + 0.03)) / (sky * (2.43 * sky + 0.59) + 0.14);
  fragColor = vec4(pow(clamp(mapped, 0.0, 1.0), vec3(1.0 / 2.2)), 1.0);
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
uniform float uShojiClosed; // 0..1
uniform float uLampOn;      // 0 or 1
uniform float uPlumeGlow;   // 0..1 distant rocket glare

// Analytical Window Sunbeam + Shoji Kumiko Lattice + Foreground Utility-Cable Gobos + Interior Wall Occlusion!
vec2 computeIndoorSunMask(vec3 p) {
  if (uSunDir.z <= 0.04 || uSunDir.y <= 0.01) {
    return vec2(0.0, 0.0);
  }
  // Occlude window sunlight behind solid Unit-Bath & Shelf divider walls!
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
    return vec2(0.08 * (1.0 - barShadow), 0.68 * (1.0 - 0.42 * barShadow));
  } else {
    // Subtle catenary cable shadow sweeping across sunlit desk & tatami!
    float catY1 = 1.94 - 0.13 * (1.0 - hit.x * hit.x * 0.35);
    float catY2 = 1.62 - 0.16 * (1.0 - hit.x * hit.x * 0.35);
    float w1 = 1.0 - 0.48 * exp(-pow((hit.y - catY1) * 60.0, 2.0));
    float w2 = 1.0 - 0.42 * exp(-pow((hit.y - catY2) * 55.0, 2.0));
    return vec2(w1 * w2, 0.08);
  }
}

float computeRoomAO(vec3 p, vec3 n) {
  float wx = smoothstep(0.0, 0.34, 2.54 - abs(p.x));
  float wz = smoothstep(0.0, 0.34, 2.02 - abs(p.z));
  float wyFloor = smoothstep(0.0, 0.22, p.y);
  float wyCeil  = smoothstep(0.0, 0.28, 2.66 - p.y);
  float cornerAO = (0.76 + 0.24 * wx * wz) * (0.78 + 0.22 * wyFloor) * (0.84 + 0.16 * wyCeil);
  float underDesk = (abs(p.x) < 1.14 && p.z > 1.26 && p.y < 0.68) ? 0.72 : 1.0;
  return cornerAO * underDesk * (0.92 + 0.08 * max(n.y, 0.0));
}

// Analytical Exterior Japanese Eave Overhang & Catenary Wire Shadows (matching image-8..12!)
float computeExteriorEaveShadow(vec3 p) {
  float sh = 1.0;
  // Crisp diagonal eave shadows under 1st-floor (y ~ -0.35) and 2nd-floor (y ~ 2.45) Kawara roofs (z ~ 10.4..12.2)!
  if (p.z > 10.2 && p.z < 12.2) {
    float diag = p.x * 0.07; // angled summer street shadow line like image-8 & image-10!
    float underUpperEave = smoothstep(1.65 + diag, 2.38 + diag, p.y) * (1.0 - smoothstep(2.44, 2.56, p.y));
    float underLowerEave = smoothstep(-1.18 + diag, -0.36 + diag, p.y) * (1.0 - smoothstep(-0.30, -0.15, p.y));
    sh *= (1.0 - 0.56 * max(underUpperEave, underLowerEave));
  }
  // Crisp utility-cable shadow lines across street asphalt & sidewalk (z in [4.5, 10.8])!
  if (p.y < -2.95 && p.z > 4.5 && p.z < 10.8) {
    float wz1 = 6.85 + 0.08 * p.x;
    float wz2 = 7.25 + 0.08 * p.x;
    sh *= (1.0 - 0.38 * exp(-pow((p.z - wz1) * 16.0, 2.0)));
    sh *= (1.0 - 0.32 * exp(-pow((p.z - wz2) * 18.0, 2.0)));
  }
  return sh;
}

void main() {
  vec3 N = normalize(gl_FrontFacing ? vNormal : -vNormal);
  vec3 V = normalize(uCamPos - vWorldPos);
  vec3 baseCol = vAlbedo;
  int mat = int(vMat + 0.5);

  if (mat == 6) {
    fragColor = vec4(baseCol, 1.0);
    return;
  }
  if (mat == 8) {
    // Analytical CRT Oscilloscope Graticule + Dual Live Telemetry Traces (crt_oscilloscope at x=0.80, y=0.755, z=1.63)
    if (vWorldPos.x > 0.72 && vWorldPos.x < 1.02 && vWorldPos.z > 1.42 && vWorldPos.z < 1.46) {
      vec2 uv = vec2((vWorldPos.x - 0.745) / 0.25, (vWorldPos.y - 0.833) / 0.21);
      float gridX = smoothstep(0.46, 0.50, abs(fract(uv.x * 6.0) - 0.5));
      float gridY = smoothstep(0.46, 0.50, abs(fract(uv.y * 4.0) - 0.5));
      float scan = 0.90 + 0.10 * sin(vWorldPos.y * 520.0);
      float tr1 = exp(-abs((uv.y - 0.58) - 0.16 * sin(uv.x * 18.8)) * 52.0);
      float tr2 = exp(-abs((uv.y - 0.30) - 0.07 * cos(uv.x * 31.4)) * 60.0);
      vec3 crt = vec3(0.02, 0.13, 0.14) * scan
        + vec3(0.07, 0.36, 0.34) * max(gridX, gridY)
        + vec3(0.35, 1.0, 0.88) * tr1 * 1.15
        + vec3(1.0, 0.78, 0.32) * tr2 * 0.75;
      fragColor = vec4(clamp(crt, 0.0, 1.0), 1.0);
      return;
    }
    float scan = 0.88 + 0.12 * sin(vWorldPos.y * 420.0);
    fragColor = vec4(clamp(baseCol * scan, 0.0, 1.0), 1.0);
    return;
  }
  if (mat == 9) {
    float rim = 0.78 + 0.22 * max(N.y, 0.0);
    fragColor = vec4(clamp(baseCol * rim * 1.15, 0.0, 1.0), 1.0);
    return;
  }
  if (mat == 10) {
    float pulse = 0.82 + 0.18 * sin((vWorldPos.x + vWorldPos.y + vWorldPos.z) * 18.0);
    fragColor = vec4(clamp(baseCol * pulse, 0.0, 1.0), 1.0);
    return;
  }

  bool indoor = (vWorldPos.z < 1.995 && abs(vWorldPos.x) < 2.58 && vWorldPos.y < 2.69);

  // Analytical zero-texture procedural surface micro-details
  float rough = 0.58;
  if (mat == 0 && !indoor && vWorldPos.z < 24.0) {
    // Painterly weathered Japanese stucco & warm asphalt subtle grain (image-9, 10)
    float plaster = sin(vWorldPos.x * 5.2 + sin(vWorldPos.y * 3.7)) * 0.028;
    baseCol *= (1.0 + plaster);
    rough = 0.72;
  } else if (mat == 1) {
    // Hinoki / Keyaki / Stained Cedar natural timber grain
    float grain = sin((vWorldPos.x * 34.0 + vWorldPos.y * 14.0) + sin(vWorldPos.z * 11.0) * 2.2) * 0.05;
    baseCol *= (1.0 + grain);
    rough = 0.40;
  } else if (mat == 2) {
    // Woven Igusa rush-grass Japanese Tatami weave
    float weave = sin(vWorldPos.z * 230.0) * 0.055 + sin(vWorldPos.x * 18.0) * 0.022;
    baseCol *= (1.0 + weave);
    rough = 0.66;
  } else if (mat == 3) {
    // Weathered corrugated galvanized iron siding (image-10, 11)
    float corr = cos((vWorldPos.z * 34.0 + vWorldPos.x * 8.0));
    N = normalize(N + vec3(0.0, 0.0, corr * 0.26));
    rough = 0.36;
  } else if (mat == 4) {
    // Brushed anodized alloy / chrome
    float brush = sin((vWorldPos.x + vWorldPos.y * 2.0) * 120.0) * 0.018;
    baseCol *= (1.0 + brush);
    rough = 0.26;
  } else if (mat == 11) {
    // Glazed Ceramic / Unit-Bath Tile sheen
    float tileGrout = 1.0;
    if (indoor && vWorldPos.x < -0.36 && vWorldPos.z < -0.24) {
      float gx = smoothstep(0.018, 0.032, abs(fract(vWorldPos.x * 4.0) - 0.5));
      float gy = smoothstep(0.018, 0.032, abs(fract(vWorldPos.y * 4.0) - 0.5));
      float gz = smoothstep(0.018, 0.032, abs(fract(vWorldPos.z * 4.0) - 0.5));
      tileGrout = 0.92 + 0.08 * (gx * gy * gz);
    }
    baseCol *= tileGrout;
    rough = 0.18;
  } else if (mat == 12) {
    // Sunlit pine bonsai, street foliage & green mountains
    float leafVar = sin(vWorldPos.x * 8.5) * cos(vWorldPos.z * 8.5 + vWorldPos.y * 6.5);
    baseCol *= (0.90 + 0.15 * leafVar);
    rough = 0.56;
  } else if (mat == 13) {
    // Traditional Japanese Kawara S-curve clay roof tiles (sangawara, image-8..12!)
    float wave = sin(vWorldPos.x * 22.0);
    float tier = smoothstep(0.0, 0.12, fract(vWorldPos.z * 2.8));
    N = normalize(N + vec3(cos(vWorldPos.x * 22.0) * 0.34, 0.0, 0.0));
    baseCol *= (0.88 + 0.12 * wave) * (0.88 + 0.12 * tier);
    rough = 0.28;
  }

  // =========================================================================
  // WARM JAPANESE STREET & SUNLIT HAB LIGHTING ENGINE
  // Warm golden-ivory sun + balanced warm taupe/slate shadow bounce (ZERO cold hospital-blue tint!)
  // =========================================================================
  float dayMix = smoothstep(0.14, 0.70, uWarmth);
  float duskMix = smoothstep(0.01, 0.35, uWarmth) * (1.0 - dayMix);
  vec3 sunColor = mix(vec3(1.55, 0.58, 0.20), vec3(1.92, 1.68, 1.28), dayMix) * smoothstep(-0.02, 0.10, uSunDir.y);

  vec3 directLight = vec3(0.0);
  float ao = 1.0;
  vec2 sunMask = vec2(1.0, 0.0);

  if (indoor) {
    sunMask = computeIndoorSunMask(vWorldPos);
    ao = computeRoomAO(vWorldPos, N);
    float ndl = max(dot(N, uSunDir), 0.0);
    float stylizedNdl = mix(ndl, smoothstep(0.02, 0.26, ndl), 0.32);
    directLight = sunColor * stylizedNdl * sunMask.x;
  } else {
    // Exterior Japanese street receives warm diagonal sun across BOTH top roofs AND street-facing facades (image-8..12)!
    vec3 streetSunDir = normalize(vec3(uSunDir.x - 0.38, max(uSunDir.y, 0.58), -0.52));
    float ndlExt = max(max(dot(N, uSunDir), dot(N, streetSunDir) * 0.92), 0.0);
    float eaveSh = computeExteriorEaveShadow(vWorldPos);
    directLight = sunColor * mix(ndlExt, smoothstep(0.04, 0.30, ndlExt), 0.35) * eaveSh;
  }

  // Translucent Golden Washi Subsurface Glow on Shoji Screen & Frosted Unit-Bath Door!
  if (mat == 5) {
    float backLit = max(dot(-N, uSunDir), 0.35) * smoothstep(0.0, 0.35, uWarmth);
    directLight += sunColor * backLit * 0.55;
  }

  // Warm-balanced indoor & exterior ambient (keeps cream Shikkui walls warm & natural!)
  float windowVis = indoor ? clamp((vWorldPos.z + 2.5) / 4.5, 0.46, 1.0) : 1.0;
  float shojiDim = indoor ? (1.0 - 0.26 * uShojiClosed) : 1.0;
  vec3 skyFill = mix(
    vec3(0.024, 0.028, 0.045),
    mix(vec3(0.14, 0.12, 0.18), vec3(0.24, 0.26, 0.31), dayMix),
    clamp(dayMix + duskMix, 0.0, 1.0)
  ) * (0.58 + 0.42 * max(N.y, 0.0));
  vec3 warmBounce = mix(
    vec3(0.016, 0.014, 0.018),
    vec3(0.22, 0.18, 0.12),
    dayMix
  ) * (0.50 + 0.50 * max(-N.y, 0.25));
  vec3 washiFill = vec3(1.0, 0.84, 0.54) * (sunMask.y * 0.42 + (indoor ? 0.12 * uShojiClosed * dayMix : 0.0));

  vec3 ambient = ((skyFill + warmBounce) * windowVis * shojiDim + washiFill) * ao;
  if (!indoor) {
    ambient = mix(vec3(0.025, 0.032, 0.055), vec3(0.32, 0.35, 0.42), dayMix) * (0.62 + 0.38 * max(N.y, 0.0));
  }

  // Interior Warm 2700K Architectural & Task Point Lights
  vec3 localLights = vec3(0.0);
  if (indoor) {
    if (uLampOn > 0.5) {
      vec3 toAndon = vec3(-2.25, 0.76, 1.70) - vWorldPos;
      float dA2 = dot(toAndon, toAndon) + 0.28;
      localLights += vec3(1.0, 0.68, 0.30) * (max(dot(N, normalize(toAndon)), 0.20) * 0.42 / dA2);

      vec3 toDesk = vec3(-0.26, 1.06, 1.46) - vWorldPos;
      float dD2 = dot(toDesk, toDesk) + 0.16;
      localLights += vec3(1.0, 0.76, 0.40) * (max(dot(N, normalize(toDesk)), 0.12) * 0.34 / dD2);
    }
    vec3 toGalley = vec3(1.98, 1.66, 1.45) - vWorldPos;
    float dG2 = dot(toGalley, toGalley) + 0.45;
    localLights += vec3(1.0, 0.74, 0.38) * (max(dot(N, normalize(toGalley)), 0.08) * 0.24 / dG2);

    vec3 toBed = vec3(1.48, 1.75, -1.46) - vWorldPos;
    float dB2 = dot(toBed, toBed) + 0.50;
    float bedAlcoveMask = smoothstep(0.32, 0.44, vWorldPos.x);
    localLights += vec3(0.98, 0.68, 0.34) * (max(dot(N, normalize(toBed)), 0.08) * 0.24 * bedAlcoveMask / dB2);

    // Warm bright Unit-Bath ceiling & backlit mirror illumination!
    if (vWorldPos.x < -0.35 && vWorldPos.z < -0.18) {
      vec3 toBath = vec3(-1.42, 2.30, -1.18) - vWorldPos;
      float dT2 = dot(toBath, toBath) + 0.42;
      localLights += vec3(1.0, 0.94, 0.82) * (max(dot(N, normalize(toBath)), 0.28) * 0.72 / dT2);
    }
  }

  // Soft Specular Fillets on Smooth Rounded Edges & Kawara Roof Tile Sheen
  vec3 specDir = indoor ? uSunDir : normalize(vec3(uSunDir.x - 0.25, max(uSunDir.y, 0.55), -0.35));
  vec3 H = normalize(specDir + V);
  float specPow = mix(96.0, 12.0, rough);
  float spec = pow(max(dot(N, H), 0.0), specPow) * (1.0 - rough) * (indoor ? sunMask.x : 1.0);
  vec3 specLight = sunColor * spec * 0.26;

  vec3 litColor = baseCol * (ambient + directLight + localLights) + specLight;

  // Subtle Analytical Warm Sun-Shaft Shimmer inside the Room
  if (indoor && dayMix > 0.25 && uSunDir.z > 0.08) {
    vec3 ray = vWorldPos - uCamPos;
    float rayLen = clamp(length(ray), 0.0, 4.5);
    vec2 m1 = computeIndoorSunMask(uCamPos + ray * 0.38);
    vec2 m2 = computeIndoorSunMask(uCamPos + ray * 0.72);
    float shaft = (m1.x + m2.x) * 0.5;
    litColor += vec3(1.0, 0.88, 0.62) * shaft * rayLen * 0.024 * dayMix * (1.0 - 0.80 * uShojiClosed);
  }

  // Atmospheric mountain perspective ONLY beyond the residential street (z > 65 m) so the street stays 100% crisp!
  if (!indoor && vWorldPos.z > 65.0) {
    float haze = 1.0 - exp(-(vWorldPos.z - 65.0) * 0.0028);
    vec3 hazeCol = mix(vec3(0.05, 0.08, 0.16), vec3(0.46, 0.72, 0.92), dayMix);
    hazeCol += vec3(1.0, 0.80, 0.48) * uPlumeGlow * 0.12;
    litColor = mix(litColor, hazeCol, clamp(haze, 0.0, 0.62));
  }

  vec3 x = max(litColor, vec3(0.0));
  vec3 mapped = (x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14);
  fragColor = vec4(pow(clamp(mapped, 0.0, 1.0), vec3(1.0 / 2.2)), 1.0);
}
`;
