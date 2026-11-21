// Analytical WebGL2 Shaders for Odderon Checkpoint 1A v2 ("Shadows of a Summer Noon").
// Zero texture files (0 bytes). All lighting, Shoji Kumiko lattice shadows, Tyndall sun-shafts,
// room ambient occlusion, ceramic glaze, tatami weave, CRT graticule, and Shinkai cumulonimbus clouds are pure math.

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

// 4-octave FBM for crisp Shinkai cauliflower cumulonimbus edges (still ultra-fast on 2015 Intel UHD!)
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

  // Makoto Shinkai deep ultramarine-cerulean Summer Noon vs Golden Hour vs Starry Night
  vec3 noonZenith  = vec3(0.014, 0.12, 0.52);
  vec3 noonMid     = vec3(0.065, 0.33, 0.78);
  vec3 noonHorizon = vec3(0.36, 0.70, 0.95);

  vec3 duskZenith  = vec3(0.04, 0.07, 0.24);
  vec3 duskMid     = vec3(0.32, 0.18, 0.36);
  vec3 duskHorizon = vec3(1.15, 0.38, 0.12);

  vec3 nightZenith = vec3(0.006, 0.012, 0.032);
  vec3 nightMid    = vec3(0.014, 0.026, 0.062);
  vec3 nightHoriz  = vec3(0.030, 0.055, 0.110);

  float dayMix = smoothstep(0.18, 0.72, uWarmth);
  float duskMix = smoothstep(0.01, 0.36, uWarmth) * (1.0 - dayMix);
  float skyActive = clamp(dayMix + duskMix, 0.0, 1.0);

  vec3 zenCol = mix(nightZenith, mix(duskZenith, noonZenith, dayMix), skyActive);
  vec3 midCol = mix(nightMid,    mix(duskMid,    noonMid,    dayMix), skyActive);
  vec3 horCol = mix(nightHoriz,  mix(duskHorizon, noonHorizon, dayMix), skyActive);

  vec3 sky = mix(midCol, zenCol, smoothstep(0.08, 0.68, up));
  sky = mix(sky, horCol, pow(1.0 - up, 5.2));

  // Subtle stars at night in the upper sky
  if (uWarmth < 0.08 && rd.y > 0.08) {
    vec2 starCell = floor((rd.xz / (rd.y + 0.35)) * 140.0);
    float sh = hash21(starCell);
    if (sh > 0.992) {
      sky += vec3(0.75, 0.85, 1.0) * (sh - 0.992) * 95.0 * (1.0 - uWarmth * 12.0);
    }
  }

  // Mie solar aureole + blazing summer sun disc
  vec3 sunTint = mix(vec3(1.25, 0.44, 0.14), vec3(1.35, 1.22, 0.95), dayMix);
  sky += sunTint * (0.20 * pow(sunAmt, 7.0) + 1.85 * pow(sunAmt, 340.0)) * smoothstep(-0.04, 0.08, uSunDir.y);

  // Towering Sculpted Japanese Summer Cumulonimbus Clouds over the Pacific Horizon (+Z)
  if (rd.y > 0.008 && rd.z > 0.08) {
    vec2 uv = vec2(rd.x / max(rd.z, 0.28), rd.y * 2.05);
    // Three overlapping convective cloud towers rising behind Uchinoura Cape!
    float c1 = 1.0 - length((uv - vec2(-0.34, 0.27)) * vec2(1.40, 1.65));
    float c2 = 1.0 - length((uv - vec2( 0.02, 0.34)) * vec2(1.25, 1.50));
    float c3 = 1.0 - length((uv - vec2( 0.38, 0.23)) * vec2(1.35, 1.85));
    float silhouette = max(max(c1, c2), c3);
    float n = fbm4(uv * 6.4);
    float detail = fbm4(uv * 15.0 + vec2(3.1, -1.7));
    float field = silhouette + 0.38 * n + 0.14 * detail;
    float density = smoothstep(0.42, 0.60, field) * smoothstep(0.012, 0.055, rd.y);

    if (density > 0.01) {
      // Directional self-shadowing & crisp silver-ivory sunlit cauliflower tops!
      vec2 sunStep = normalize(vec2(uSunDir.x, max(uSunDir.y, 0.15))) * 0.038;
      float nLit = fbm4((uv + sunStep) * 6.4);
      float topShade = clamp(0.52 + (n - nLit) * 3.2 + (rd.y - 0.08) * 1.45, 0.0, 1.0);
      vec3 cloudBright = mix(vec3(1.28, 0.62, 0.34), vec3(1.38, 1.34, 1.26), dayMix);
      vec3 cloudBrightNight = vec3(0.07, 0.10, 0.17);
      vec3 cloudShadow = mix(vec3(0.18, 0.15, 0.30), vec3(0.20, 0.33, 0.58), dayMix);
      vec3 cloudShadowNight = vec3(0.025, 0.04, 0.08);

      vec3 cb = mix(cloudBrightNight, cloudBright, skyActive);
      vec3 cs = mix(cloudShadowNight, cloudShadow, skyActive);
      vec3 cCol = mix(cs, cb, smoothstep(0.15, 0.85, topShade));
      // Crisp bright rim edge
      float rim = (1.0 - smoothstep(0.43, 0.66, field)) * topShade * dayMix;
      cCol += vec3(0.30, 0.28, 0.22) * rim;
      sky = mix(sky, cCol, density * 0.96);
    }
  }

  // Refractive 343 m/s shockwave condensation ring expanding from Pad-1 (-14, 148)
  if (uShockFrac > 0.01 && uShockFrac < 1.15 && rd.z > 0.2) {
    float angDist = length(vec2(rd.x / max(rd.z, 0.2) - (-14.0 / 148.0), rd.y - 0.065));
    float ringR = uShockFrac * 0.58;
    float ring = exp(-pow((angDist - ringR) * 38.0, 2.0)) * (1.0 - uShockFrac * 0.78);
    sky += vec3(0.90, 0.95, 1.0) * max(ring, 0.0) * 0.32;
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
uniform float uPlumeGlow;   // 0..1 coastal rocket glare

// Analytical Window Sunbeam + Shoji Kumiko Lattice + Swaying Pine/Catenary Gobos!
// Centered window opening: x in [-1.25, +1.25], y in [0.77, 2.26], z = 1.97
vec2 computeIndoorSunMask(vec3 p) {
  if (uSunDir.z <= 0.04 || uSunDir.y <= 0.01) {
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
  // Center wooden mullion shadow
  if (abs(hit.x) < 0.026) {
    return vec2(0.0, 0.0);
  }

  // Shoji slides from Screen-Left (+1.25) toward Screen-Right (-1.25)
  float shojiWidth = 0.24 + clamp(uShojiClosed, 0.0, 1.0) * (2.50 - 0.24);
  float shojiLeftEdge = 1.25 - shojiWidth;

  if (hit.x >= shojiLeftEdge) {
    float gx = abs(fract((1.25 - hit.x) / 0.26) - 0.5);
    float gy = abs(fract((hit.y - 0.77) / 0.298) - 0.5);
    float barShadow = smoothstep(0.43, 0.47, max(gx, gy));
    return vec2(0.06 * (1.0 - barShadow), 0.62 * (1.0 - 0.45 * barShadow));
  } else {
    // Open coastal window: crisp catenary cable shadow + dappled coastal pine-bough gobo!
    float catenaryY = 1.92 - 0.14 * (1.0 - hit.x * hit.x * 0.42);
    float wireShadow = 1.0 - 0.62 * exp(-pow((hit.y - catenaryY) * 58.0, 2.0));
    float leafField = sin(hit.x * 13.0 + hit.y * 9.0) * cos(hit.x * 9.5 - hit.y * 14.0);
    float edgeCorner = smoothstep(0.72, 1.18, abs(hit.x)) * smoothstep(1.52, 2.15, hit.y);
    float leafGobo = (edgeCorner > 0.25 && leafField > 0.18) ? 0.30 : 1.0;
    return vec2(wireShadow * leafGobo, 0.06);
  }
}

// Analytical Room Corner & Furniture Contact Ambient Occlusion (0 extra passes, grounded depth!)
float computeRoomAO(vec3 p, vec3 n) {
  float wx = smoothstep(0.0, 0.38, 2.54 - abs(p.x));
  float wz = smoothstep(0.0, 0.38, 2.02 - abs(p.z));
  float wyFloor = smoothstep(0.0, 0.26, p.y);
  float wyCeil  = smoothstep(0.0, 0.32, 2.66 - p.y);
  float cornerAO = (0.68 + 0.32 * wx * wz) * (0.72 + 0.28 * wyFloor) * (0.80 + 0.20 * wyCeil);
  // Subtle under-desk / under-bed / toe-kick shadow darkening for downward/low surfaces
  float underDesk = (abs(p.x) < 1.16 && p.z > 1.24 && p.y < 0.68) ? 0.62 : 1.0;
  return cornerAO * underDesk * (0.90 + 0.10 * max(n.y, 0.0));
}

void main() {
  vec3 N = normalize(gl_FrontFacing ? vNormal : -vNormal);
  vec3 V = normalize(uCamPos - vWorldPos);
  vec3 baseCol = vAlbedo;
  int mat = int(vMat + 0.5);

  // Emissive materials
  if (mat == 6) {
    fragColor = vec4(baseCol, 1.0);
    return;
  }
  if (mat == 8) {
    // Analytical CRT Oscilloscope Graticule + Dual Live Telemetry Traces (or crisp LED indicator)
    if (vWorldPos.x > 0.68 && vWorldPos.x < 1.02 && vWorldPos.z > 1.42 && vWorldPos.z < 1.46) {
      vec2 uv = vec2((vWorldPos.x - 0.70) / 0.31, (vWorldPos.y - 0.81) / 0.24);
      float gridX = smoothstep(0.46, 0.50, abs(fract(uv.x * 6.0) - 0.5));
      float gridY = smoothstep(0.46, 0.50, abs(fract(uv.y * 4.0) - 0.5));
      float scan = 0.90 + 0.10 * sin(vWorldPos.y * 520.0);
      float tr1 = exp(-abs((uv.y - 0.58) - 0.16 * sin(uv.x * 18.8)) * 52.0);
      float tr2 = exp(-abs((uv.y - 0.30) - 0.07 * cos(uv.x * 31.4)) * 60.0);
      vec3 crt = vec3(0.02, 0.12, 0.13) * scan
        + vec3(0.06, 0.34, 0.32) * max(gridX, gridY)
        + vec3(0.35, 1.0, 0.88) * tr1 * 1.15
        + vec3(1.0, 0.78, 0.32) * tr2 * 0.75;
      fragColor = vec4(clamp(crt, 0.0, 1.0), 1.0);
      return;
    }
    // VHF OLED Frequency Window or crisp Breaker Status LED
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

  // Analytical zero-texture procedural surface micro-details
  float rough = 0.62;
  if (mat == 1) {
    // Hinoki / Keyaki / Walnut natural timber grain
    float grain = sin((vWorldPos.x * 36.0 + vWorldPos.y * 14.0) + sin(vWorldPos.z * 11.0) * 2.4) * 0.055;
    baseCol *= (1.0 + grain);
    rough = 0.38;
  } else if (mat == 2) {
    // Woven Igusa rush-grass Japanese Tatami weave
    float weave = sin(vWorldPos.z * 230.0) * 0.06 + sin(vWorldPos.x * 18.0) * 0.025;
    baseCol *= (1.0 + weave);
    rough = 0.68;
  } else if (mat == 3) {
    // Corrugated galvanized steel & Kawara roof tiles
    float corr = cos((vWorldPos.x + vWorldPos.z * 0.35) * 38.0);
    N = normalize(N + vec3(corr * 0.30, 0.0, 0.0));
    rough = 0.30;
  } else if (mat == 4) {
    // Star Citizen brushed anodized alloy
    float brush = sin((vWorldPos.x + vWorldPos.y * 2.0) * 120.0) * 0.02;
    baseCol *= (1.0 + brush);
    rough = 0.26;
  } else if (mat == 7) {
    // Deep Cobalt Pacific Ocean with crisp Shinkai sun-glitter waves!
    float w1 = sin(vWorldPos.x * 0.85 + vWorldPos.z * 0.55);
    float w2 = cos(vWorldPos.x * 0.42 - vWorldPos.z * 1.15);
    float w3 = sin(vWorldPos.x * 2.30 + vWorldPos.z * 1.90);
    N = normalize(vec3((w1 + 0.5 * w3) * 0.095, 1.0, (w2 - 0.5 * w3) * 0.095));
    baseCol *= (0.90 + 0.12 * w1 * w2);
    rough = 0.11;
  } else if (mat == 11) {
    // Glazed Ceramic / Unit-Bath Tile sheen
    float tileGrout = 1.0;
    if (vWorldPos.x < -0.38 && vWorldPos.z < -0.28) {
      float gx = smoothstep(0.018, 0.032, abs(fract(vWorldPos.x * 4.0) - 0.5));
      float gy = smoothstep(0.018, 0.032, abs(fract(vWorldPos.y * 4.0) - 0.5));
      float gz = smoothstep(0.018, 0.032, abs(fract(vWorldPos.z * 4.0) - 0.5));
      tileGrout = 0.90 + 0.10 * (gx * gy * gz);
    }
    baseCol *= tileGrout;
    rough = 0.18;
  } else if (mat == 12) {
    // Dappled summer pine needles, moss & coastal hillside foliage
    float leafVar = sin(vWorldPos.x * 9.0) * cos(vWorldPos.z * 9.0 + vWorldPos.y * 7.0);
    baseCol *= (0.88 + 0.18 * leafVar);
    rough = 0.58;
  }

  // =========================================================================
  // MAKOTO SHINKAI "SHADOWS OF A SUMMER NOON" LIGHTING ENGINE
  // High-contrast golden 5400K sunbeams vs cool cerulean-indigo indoor shadows!
  // =========================================================================
  float dayMix = smoothstep(0.14, 0.70, uWarmth);
  float duskMix = smoothstep(0.01, 0.35, uWarmth) * (1.0 - dayMix);
  vec3 sunColor = mix(vec3(1.65, 0.56, 0.18), vec3(2.35, 1.96, 1.38), dayMix) * smoothstep(-0.02, 0.10, uSunDir.y);

  bool indoor = (vWorldPos.z < 1.995 && abs(vWorldPos.x) < 2.58 && vWorldPos.y < 2.69);
  vec2 sunMask = indoor ? computeIndoorSunMask(vWorldPos) : vec2(1.0, 0.0);
  float ao = indoor ? computeRoomAO(vWorldPos, N) : 1.0;

  float ndl = max(dot(N, uSunDir), 0.0);
  float stylizedNdl = mix(ndl, smoothstep(0.03, 0.24, ndl), 0.38);
  vec3 directLight = sunColor * stylizedNdl * sunMask.x;

  // Translucent Golden Washi Subsurface Glow on Shoji Screen & Frosted Unit-Bath Glass!
  if (mat == 5) {
    float backLit = max(dot(-N, uSunDir), 0.28) * smoothstep(0.0, 0.35, uWarmth);
    directLight += sunColor * backLit * 0.55;
  }

  // Cool Shinkai sky shadow fill vs warm Tatami/Cedar bounce (intentionally crisp contrast!)
  float windowVis = indoor ? clamp((vWorldPos.z + 2.3) / 4.4, 0.30, 1.0) : 1.0;
  float shojiDim = indoor ? (1.0 - 0.38 * uShojiClosed) : 1.0;
  vec3 skyFill = mix(
    vec3(0.018, 0.028, 0.055),
    mix(vec3(0.10, 0.09, 0.20), vec3(0.11, 0.19, 0.34), dayMix),
    clamp(dayMix + duskMix, 0.0, 1.0)
  ) * (0.55 + 0.45 * max(N.y, 0.0));
  vec3 warmBounce = mix(
    vec3(0.012, 0.012, 0.018),
    vec3(0.14, 0.11, 0.065),
    dayMix
  ) * (0.45 + 0.55 * max(-N.y, 0.2));
  vec3 washiFill = vec3(1.0, 0.80, 0.46) * (sunMask.y * 0.48 + (indoor ? 0.11 * uShojiClosed * dayMix : 0.0));

  vec3 ambient = ((skyFill + warmBounce) * windowVis * shojiDim + washiFill) * ao;
  if (!indoor) {
    // Exterior sky hemisphere fill
    ambient = mix(vec3(0.02, 0.03, 0.06), vec3(0.20, 0.34, 0.54), dayMix) * (0.60 + 0.40 * max(N.y, 0.0));
  }

  // Interior Warm 2700K Architectural & Task Point Lights
  vec3 localLights = vec3(0.0);
  if (indoor) {
    if (uLampOn > 0.5) {
      // Top-Right Bonsai Table Washi Andon (-2.25, 0.76, 1.70)
      vec3 toAndon = vec3(-2.25, 0.76, 1.70) - vWorldPos;
      float dA2 = dot(toAndon, toAndon) + 0.26;
      localLights += vec3(1.0, 0.64, 0.26) * (max(dot(N, normalize(toAndon)), 0.18) * 0.42 / dA2);

      // Articulated Desk Lamp Shade Cone (-0.26, 1.06, 1.46)
      vec3 toDesk = vec3(-0.26, 1.06, 1.46) - vWorldPos;
      float dD2 = dot(toDesk, toDesk) + 0.14;
      localLights += vec3(1.0, 0.74, 0.36) * (max(dot(N, normalize(toDesk)), 0.10) * 0.34 / dD2);
    }
    // Top-Left L-Kitchen Warm Under-Cabinet Task Strips
    vec3 toGalley = vec3(1.98, 1.66, 1.45) - vWorldPos;
    float dG2 = dot(toGalley, toGalley) + 0.42;
    localLights += vec3(1.0, 0.70, 0.34) * (max(dot(N, normalize(toGalley)), 0.05) * 0.22 / dG2);

    // Bottom-Left Bed Warm Overhead Reading Strip
    vec3 toBed = vec3(1.48, 1.75, -1.46) - vWorldPos;
    float dB2 = dot(toBed, toBed) + 0.48;
    localLights += vec3(0.98, 0.64, 0.28) * (max(dot(N, normalize(toBed)), 0.05) * 0.22 / dB2);

    // Bottom-Right Unit-Bath Interior Ceiling & Vanity Mirror Light
    if (vWorldPos.x < -0.34 && vWorldPos.z < -0.10) {
      vec3 toBath = vec3(-1.42, 2.25, -1.25) - vWorldPos;
      float dT2 = dot(toBath, toBath) + 0.45;
      localLights += vec3(1.0, 0.92, 0.78) * (max(dot(N, normalize(toBath)), 0.20) * 0.58 / dT2);
    }
  }

  // Crisp Specular Highlights (wood polish, brushed alloy, porcelain, and sparkling Pacific Ocean glitter!)
  vec3 H = normalize(uSunDir + V);
  float specPow = mix(128.0, 14.0, rough);
  float spec = pow(max(dot(N, H), 0.0), specPow) * (1.0 - rough) * sunMask.x;
  vec3 specLight = sunColor * spec * (mat == 7 ? 2.6 : 0.32);

  vec3 litColor = baseCol * (ambient + directLight + localLights) + specLight;

  // Subtle Analytical Tyndall Volumetric Sun-Shaft Shimmer inside the Sunlit Room!
  // 2 ultra-lightweight ray march samples along camera->surface ray (zero extra draw calls!)
  if (indoor && dayMix > 0.25 && uSunDir.z > 0.08) {
    vec3 ray = vWorldPos - uCamPos;
    float rayLen = clamp(length(ray), 0.0, 4.5);
    vec2 m1 = computeIndoorSunMask(uCamPos + ray * 0.38);
    vec2 m2 = computeIndoorSunMask(uCamPos + ray * 0.72);
    float shaft = (m1.x + m2.x) * 0.5;
    litColor += vec3(1.0, 0.86, 0.56) * shaft * rayLen * 0.028 * dayMix * (1.0 - 0.80 * uShojiClosed);
  }

  // Gentle coastal aerial perspective haze on far headlands (preserves vivid blue ocean & crisp town!)
  if (!indoor && vWorldPos.z > 18.0) {
    float haze = 1.0 - exp(-(vWorldPos.z - 18.0) * 0.0018);
    vec3 hazeCol = mix(vec3(0.05, 0.08, 0.16), vec3(0.32, 0.64, 0.92), dayMix);
    hazeCol += vec3(1.0, 0.78, 0.45) * uPlumeGlow * 0.14;
    litColor = mix(litColor, hazeCol, clamp(haze, 0.0, 0.52));
  }

  vec3 x = max(litColor, vec3(0.0));
  vec3 mapped = (x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14);
  fragColor = vec4(pow(clamp(mapped, 0.0, 1.0), vec3(1.0 / 2.2)), 1.0);
}
`;
