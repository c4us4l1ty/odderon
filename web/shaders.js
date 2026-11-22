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

  // Deep Photographic Japanese Summer Noon Cobalt-Cerulean Sky Palette (image-18 & image-15 reference!)
  vec3 noonZenith  = vec3(0.014, 0.110, 0.420);
  vec3 noonMid     = vec3(0.042, 0.255, 0.585);
  vec3 noonHorizon = vec3(0.240, 0.485, 0.700);

  vec3 duskZenith  = vec3(0.035, 0.065, 0.195);
  vec3 duskMid     = vec3(0.280, 0.145, 0.260);
  vec3 duskHorizon = vec3(0.980, 0.340, 0.110);

  vec3 nightZenith = vec3(0.005, 0.010, 0.026);
  vec3 nightMid    = vec3(0.012, 0.022, 0.050);
  vec3 nightHoriz  = vec3(0.026, 0.045, 0.088);

  float dayMix = smoothstep(0.18, 0.72, uWarmth);
  float duskMix = smoothstep(0.01, 0.36, uWarmth) * (1.0 - dayMix);
  float skyActive = clamp(dayMix + duskMix, 0.0, 1.0);

  vec3 zenCol = mix(nightZenith, mix(duskZenith, noonZenith, dayMix), skyActive);
  vec3 midCol = mix(nightMid,    mix(duskMid,    noonMid,    dayMix), skyActive);
  vec3 horCol = mix(nightHoriz,  mix(duskHorizon, noonHorizon, dayMix), skyActive);

  vec3 sky = mix(midCol, zenCol, smoothstep(0.06, 0.62, up));
  sky = mix(sky, horCol, pow(1.0 - up, 5.2));

  if (uWarmth < 0.08 && rd.y > 0.08) {
    vec2 starCell = floor((rd.xz / (rd.y + 0.35)) * 140.0);
    float sh = hash21(starCell);
    if (sh > 0.992) {
      sky += vec3(0.75, 0.85, 1.0) * (sh - 0.992) * 90.0 * (1.0 - uWarmth * 12.0);
    }
  }

  vec3 sunTint = mix(vec3(1.10, 0.40, 0.14), vec3(1.15, 0.98, 0.72), dayMix);
  sky += sunTint * (0.10 * pow(sunAmt, 9.0) + 1.35 * pow(sunAmt, 380.0)) * smoothstep(-0.04, 0.08, uSunDir.y);

  // 360-Degree Towering Japanese Summer Cumulonimbus Clouds (image-18 -Z street view AND image-15 +Z window vista!)
  if (rd.y > 0.015) {
    float az = atan(rd.x, rd.z);
    // Project smoothly onto panoramic sky coordinates without seam artifacts!
    vec2 uv = vec2(az * 0.62, rd.y * 1.92);

    // Sculpted +Z vista towers (around az = 0) AND towering -Z summer cumulonimbus anvils behind Uchinoura Maison (az ~ +-PI)!
    float dBack = min(abs(az - 3.02), abs(az + 3.26));
    float dBackL = min(abs(az + 2.48), abs(az - 3.80));
    float dBackR = min(abs(az - 2.38), abs(az + 3.90));

    float cPos1 = 1.0 - length(vec2((az + 0.11) * 1.95, (uv.y - 0.34) * 1.55));
    float cPos2 = 1.0 - length(vec2((az - 0.14) * 2.05, (uv.y - 0.44) * 1.58));
    float cPos3 = 1.0 - length(vec2((az + 0.34) * 2.10, (uv.y - 0.38) * 1.70));

    float cNeg1 = 1.04 - length(vec2(dBack  * 1.45, (uv.y - 0.52) * 1.28));
    float cNeg2 = 0.98 - length(vec2(dBackL * 1.55, (uv.y - 0.38) * 1.45));
    float cNeg3 = 0.96 - length(vec2(dBackR * 1.60, (uv.y - 0.42) * 1.48));
    float cSide1 = 0.92 - length(vec2((abs(az) - 1.45) * 1.75, (uv.y - 0.36) * 1.65));

    float silhouette = max(max(max(cPos1, cPos2), max(cPos3, cSide1)), max(max(cNeg1, cNeg2), cNeg3));
    if (silhouette > -0.10) {
      vec2 noiseP = vec2(sin(az) * 1.85 + cos(az) * 0.95, uv.y * 1.45) * 3.8;
      float n = fbm4(noiseP);
      float detail = fbm4(noiseP * 2.45 + vec2(3.1, -1.7));
      float field = silhouette + 0.40 * n + 0.15 * detail;
      float density = smoothstep(0.44, 0.60, field) * smoothstep(0.02, 0.09, rd.y);

      if (density > 0.008) {
        vec2 sunStep = vec2(-0.035, 0.048);
        float nLit = fbm4(noiseP + sunStep * 4.2);
        float topShade = clamp(0.52 + (n - nLit) * 3.8 + (rd.y - 0.10) * 1.55, 0.0, 1.0);
        float silverRim = smoothstep(0.44, 0.52, field) * (1.0 - smoothstep(0.54, 0.68, field));
        vec3 cloudBright = mix(vec3(1.10, 0.54, 0.30), vec3(1.16, 1.11, 1.02), dayMix);
        vec3 cloudShadow = mix(vec3(0.14, 0.12, 0.22), vec3(0.16, 0.27, 0.44), dayMix);
        vec3 cb = mix(vec3(0.05, 0.07, 0.12), cloudBright, skyActive);
        vec3 cs = mix(vec3(0.018, 0.028, 0.055), cloudShadow, skyActive);
        vec3 cCol = mix(cs, cb, smoothstep(0.14, 0.82, topShade));
        cCol += cb * silverRim * 0.24 * dayMix;
        sky = mix(sky, cCol, density * 0.97);
      }
    }
  }

  // Distant 343 m/s shockwave condensation ring expanding from far mountain Pad-1 (-58, 17.8, 380)
  if (uShockFrac > 0.01 && uShockFrac < 1.15 && rd.z > 0.2) {
    float angDist = length(vec2(rd.x / max(rd.z, 0.2) - (-58.0 / 380.0), rd.y - 0.048));
    float ringR = uShockFrac * 0.34;
    float ring = exp(-pow((angDist - ringR) * 48.0, 2.0)) * (1.0 - uShockFrac * 0.78);
    sky += vec3(0.92, 0.95, 1.0) * max(ring, 0.0) * 0.24;
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
uniform float uShojiClosed;
uniform float uEntryDoorOpen;
uniform float uLampOn;
uniform float uPlumeGlow;

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
float computeExteriorEaveShadow(vec3 p) {
  float sh = 1.0;
  float raking = p.x * 0.075;

  // A. +Z Street Row (House #1 & House #2 seen from Hab #204 window, z in [10.1, 12.6])
  if (p.z > 10.1 && p.z < 12.6) {
    float underUpperEave = smoothstep(1.48 + raking, 2.34 + raking, p.y) * (1.0 - smoothstep(2.42, 2.54, p.y));
    float underLowerEave = smoothstep(-2.05 + raking, -0.36 + raking, p.y) * (1.0 - smoothstep(-0.30, -0.14, p.y));
    sh *= (1.0 - 0.88 * max(underUpperEave, underLowerEave));

    float leafField = sin(p.x * 1.35 + p.y * 1.15) * cos(p.x * 0.85 - p.y * 1.65)
      + 0.45 * sin(p.x * 3.4 + p.y * 2.9);
    float edgeCanopy = smoothstep(5.2, 8.6, p.x) + smoothstep(-5.8, -8.8, p.x);
    float dapple = smoothstep(0.05, 0.52, leafField + edgeCanopy * 0.55) * clamp(edgeCanopy + 0.22, 0.0, 1.0);
    sh *= (1.0 - 0.72 * dapple);
  }

  // B. -Z Street Row: Uchinoura Maison, Corrugated Steel Annex, Sister Kominka Machiya & Bus Stop (image-18!)
  if (p.z > -0.6 && p.z < 5.05) {
    // 1. Deep Kawara 2F roof-eave shadow band across Uchinoura Maison 2F plaster & shutters (x in [-3.1, 3.1])
    if (p.x > -3.12 && p.x < 3.12 && p.z > 2.05 && p.z < 3.10) {
      float eave2F = smoothstep(1.72 - raking, 2.52 - raking, p.y) * (1.0 - smoothstep(2.74, 2.84, p.y));
      sh *= (1.0 - 0.86 * eave2F);
    }
    // 2. Dramatic 2F Wooden Balcony + 1F Kawara Hisashi Eave overhang shadow across 1F roasted-cedar wall & noren!
    if (p.x > -3.92 && p.x < 2.86 && p.z > 2.24 && p.z < 3.18 && p.y < -0.10) {
      float hisashi1F = smoothstep(-2.35 - raking * 0.8, -0.55 - raking * 0.8, p.y) * (1.0 - smoothstep(2.96, 3.18, p.z));
      sh *= (1.0 - 0.88 * hisashi1F);
    }
    // 3. Deep canopy & deck shadow under 2F Breezeway Gallery & Entry Doorway (x in [-5.50, -2.64])
    if (p.x > -5.50 && p.x < -2.64 && p.z > -0.35 && p.z < 1.75) {
      float underCanopy = smoothstep(1.35, 2.15, p.y) * (1.0 - smoothstep(2.28, 2.40, p.y));
      float underDeck = 1.0 - smoothstep(-0.22, -0.05, p.y);
      sh *= (1.0 - 0.84 * max(underCanopy, underDeck));
    }
    // 4. Sister Kominka Machiya (#3, x in [-13.4, -5.7]) 2F & 1F Kawara eave shadows + Bus Stop roof shade!
    if (p.x > -13.4 && p.x < -5.7 && p.z > 1.15 && p.z < 3.30) {
      float sisUpper = smoothstep(1.45 - raking, 2.28 - raking, p.y) * (1.0 - smoothstep(2.42, 2.56, p.y));
      float sisLower = smoothstep(-2.15 - raking, -0.45 - raking, p.y) * (1.0 - smoothstep(-0.28, -0.12, p.y));
      sh *= (1.0 - 0.88 * max(sisUpper, sisLower));
    }
    if (p.x > -7.85 && p.x < -5.85 && p.z > 3.72 && p.z < 4.78 && p.y < -0.82) {
      float underBusRoof = smoothstep(-2.65, -1.15, p.y);
      sh *= (1.0 - 0.78 * underBusRoof);
    }
    // 5. Diagonal Dappled Zelkova Foliage Gobo cast across Corrugated Annex (x > 2.9, image-18 left!) & West Trees (x < -4.4)!
    float leafGobo = sin(p.x * 1.55 - p.y * 1.35 + p.z * 0.75) * cos(p.x * 0.95 + p.y * 1.75)
      + 0.48 * sin(p.x * 3.8 + p.y * 3.1 - p.z * 1.6);
    float eastTreeZone = smoothstep(2.9, 6.2, p.x) * smoothstep(-2.9, 2.4, p.y);
    float westTreeZone = smoothstep(-4.1, -7.8, p.x);
    float canopyMask = clamp(eastTreeZone * 0.88 + westTreeZone * 0.92, 0.0, 1.0);
    float gobo = smoothstep(-0.08, 0.44, leafGobo) * canopyMask;
    sh *= (1.0 - 0.80 * gobo);

    // 6. Crisp diagonal Staircase & Wooden Balustrade shadow cast onto near sidewalk & 1F corner!
    if (p.y < -2.75 && p.x > -5.55 && p.x < -3.15 && p.z > 1.55 && p.z < 4.75) {
      float treadStripe = smoothstep(0.15, 0.45, abs(fract((p.z + p.x * 0.18) * 4.4) - 0.5));
      sh *= mix(0.22, 0.72, treadStripe);
    }
  }

  // C. Ground-Plane Shadows on Near Sidewalk, Asphalt Road & Far Sidewalk (building cast + utility pole + crisp overhead wires!)
  if (p.y < -2.90 && p.z > 2.35 && p.z < 11.4) {
    float farBuildingCast = smoothstep(8.35, 9.85, p.z + p.x * 0.05);
    sh *= (1.0 - 0.82 * farBuildingCast);

    // Diagonal utility-pole shadow across near sidewalk & asphalt from pole at (1.08, 5.35)
    float poleLine = (p.x - 1.08) + (p.z - 5.35) * 0.62;
    if (p.z < 5.42 && p.z > 2.40) {
      sh *= (1.0 - 0.76 * exp(-pow(poleLine * 7.5, 2.0)));
    }
    // Crisp overhead power-line & service-drop shadows slicing across sunlit concrete & asphalt (image-18!)
    float wNear = 3.88 - 0.11 * p.x;
    float wz1   = 6.68 + 0.08 * p.x;
    float wz2   = 7.14 + 0.08 * p.x;
    sh *= (1.0 - 0.62 * exp(-pow((p.z - wNear) * 24.0, 2.0)));
    sh *= (1.0 - 0.58 * exp(-pow((p.z - wz1) * 20.0, 2.0)));
    sh *= (1.0 - 0.52 * exp(-pow((p.z - wz2) * 22.0, 2.0)));

    // Dappled tree-canopy pools on the near sidewalk & curb (x < -4.2 and x > 3.6, image-18 foreground!)
    float groundLeaf = sin(p.x * 1.45 + p.z * 1.65) * cos(p.x * 1.05 - p.z * 1.35)
      + 0.42 * sin(p.x * 3.6 + p.z * 3.2);
    float roadEdgeTrees = smoothstep(-3.8, -7.2, p.x) + smoothstep(3.4, 6.8, p.x);
    float roadDapple = smoothstep(0.02, 0.46, groundLeaf) * clamp(roadEdgeTrees, 0.0, 1.0);
    sh *= (1.0 - 0.78 * roadDapple);
  }
  return clamp(sh, 0.06, 1.0);
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
    float rim = 0.78 + 0.22 * max(N.y, 0.0);
    fragColor = vec4(clamp(baseCol * rim * 1.12, 0.0, 1.0), 1.0);
    return;
  }
  if (mat == 10) {
    float pulse = 0.82 + 0.18 * sin((vWorldPos.x + vWorldPos.y + vWorldPos.z) * 18.0);
    fragColor = vec4(clamp(baseCol * pulse, 0.0, 1.0), 1.0);
    return;
  }

  bool indoor = (vWorldPos.z < 1.995 && vWorldPos.z > -2.10 && abs(vWorldPos.x) < 2.58 && vWorldPos.y > -0.06 && vWorldPos.y < 2.695);

  // Photographic Procedural Material Micro-Surface & Weathered Patina (0 texture bytes!)
  float rough = 0.62;
  if (mat == 0) {
    if (!indoor && vWorldPos.z < 26.0) {
      // Aged Japanese sand-lime stucco mottling & street moisture patina near ground
      float mottle = sin(vWorldPos.x * 3.8 + cos(vWorldPos.y * 3.1)) * 0.055
        + sin(vWorldPos.x * 13.0 - vWorldPos.y * 9.5) * 0.025;
      float dampBase = smoothstep(-3.08, -2.35, vWorldPos.y);
      baseCol *= (0.96 + mottle) * (0.76 + 0.24 * dampBase);
      rough = 0.82;
    } else if (indoor) {
      float trowel = sin(vWorldPos.x * 6.5 + cos(vWorldPos.y * 5.2 + vWorldPos.z * 6.0)) * 0.022;
      baseCol *= (1.0 + trowel);
      rough = 0.76;
    }
  } else if (mat == 1) {
    // Roasted Yakisugi / Hinoki / Keyaki timber plank seams + coffered ceiling plank grain
    float grainCoord = (abs(N.y) > 0.5)
      ? (vWorldPos.x * 28.0 + vWorldPos.z * 14.0 + sin(vWorldPos.z * 9.0) * 2.0)
      : (vWorldPos.x * 38.0 + vWorldPos.y * 16.0 + sin(vWorldPos.z * 12.0) * 2.4);
    float grain = sin(grainCoord) * 0.075;
    float plank = (abs(N.y) > 0.5)
      ? (0.93 + 0.07 * smoothstep(0.03, 0.09, abs(fract(vWorldPos.x * 4.5) - 0.5)))
      : ((!indoor) ? (0.92 + 0.08 * smoothstep(0.04, 0.12, abs(fract(vWorldPos.x * 5.5) - 0.5))) : 1.0);
    baseCol *= (1.0 + grain) * plank;
    rough = 0.44;
  } else if (mat == 2) {
    // Woven Igusa rush-grass Japanese Tatami weave
    float weave = sin(vWorldPos.z * 230.0) * 0.065 + sin(vWorldPos.x * 18.0) * 0.025;
    baseCol *= (1.0 + weave);
    rough = 0.70;
  } else if (mat == 3) {
    // Corrugated galvanized steel annex siding with subtle rust/weathering streaks (image-15 far left!)
    float corr = cos(vWorldPos.x * 26.0 + vWorldPos.z * 26.0);
    N = normalize(N + vec3(corr * 0.30, 0.0, -corr * 0.18));
    float rust = clamp(sin(vWorldPos.x * 4.7) * cos(vWorldPos.y * 2.3), 0.0, 1.0);
    baseCol = mix(baseCol, vec3(0.28, 0.18, 0.12), rust * 0.24);
    rough = 0.38;
  } else if (mat == 4) {
    float brush = sin((vWorldPos.x + vWorldPos.y * 2.0) * 120.0) * 0.020;
    baseCol *= (1.0 + brush);
    rough = 0.28;
  } else if (mat == 11) {
    float tileGrout = 1.0;
    if (indoor && vWorldPos.x < -0.36 && vWorldPos.z < -0.24) {
      float gx = smoothstep(0.018, 0.032, abs(fract(vWorldPos.x * 4.0) - 0.5));
      float gy = smoothstep(0.018, 0.032, abs(fract(vWorldPos.y * 4.0) - 0.5));
      float gz = smoothstep(0.018, 0.032, abs(fract(vWorldPos.z * 4.0) - 0.5));
      tileGrout = 0.90 + 0.10 * (gx * gy * gz);
    }
    baseCol *= tileGrout;
    rough = 0.20;
  } else if (mat == 12) {
    float leafVar = sin(vWorldPos.x * 8.5) * cos(vWorldPos.z * 8.5 + vWorldPos.y * 6.5);
    baseCol *= (0.86 + 0.18 * leafVar);
    rough = 0.62;
  } else if (mat == 13) {
    // Dark Charcoal Kawara S-curve clay roof tiles (sangawara, image-15!)
    float wave = sin(vWorldPos.x * 22.0);
    float tier = smoothstep(0.0, 0.14, fract(vWorldPos.z * 2.8));
    N = normalize(N + vec3(cos(vWorldPos.x * 22.0) * 0.36, 0.0, 0.0));
    baseCol *= (0.84 + 0.16 * wave) * (0.85 + 0.15 * tier);
    rough = 0.26;
  }

  // =========================================================================
  // PHOTOGRAPHIC "SUMMER NOON" HIGH-CONTRAST LIGHTING ENGINE (image-18 & image-15!)
  // Coherent single-sun 3D form shading + golden highlights + deep indigo/espresso shadows!
  // =========================================================================
  float dayMix = smoothstep(0.14, 0.70, uWarmth);
  float duskMix = smoothstep(0.01, 0.35, uWarmth) * (1.0 - dayMix);
  vec3 sunColor = mix(vec3(1.32, 0.44, 0.14), vec3(1.58, 1.22, 0.80), dayMix) * smoothstep(-0.02, 0.10, uSunDir.y);

  vec3 directLight = vec3(0.0);
  float ao = 1.0;
  vec2 sunMask = vec2(1.0, 0.0);
  float extShadow = 1.0;

  if (indoor) {
    sunMask = computeIndoorSunMask(vWorldPos);
    extShadow = sunMask.x;
    ao = computeRoomAO(vWorldPos, N);
    float ndl = max(dot(N, uSunDir), 0.0);
    directLight = sunColor * ndl * extShadow;
  } else {
    // Coherent single directional sun per block — NEVER lighting two opposite faces of the same 3D prop!
    vec3 nearSun = normalize(vec3(-0.38, max(uSunDir.y, 0.68), max(uSunDir.z, 0.46)));
    vec3 farSun  = normalize(vec3(-0.38, max(uSunDir.y, 0.68), -0.44));
    vec3 activeExtSun = normalize(mix(nearSun, farSun, smoothstep(8.2, 9.8, vWorldPos.z)));
    float ndlExt = max(dot(N, activeExtSun), 0.0);
    extShadow = computeExteriorEaveShadow(vWorldPos);

    // Rich ground & corner contact AO on sidewalks, plinths & eaves
    float streetAO = (abs(vWorldPos.z) < 26.0)
      ? (0.64 + 0.36 * smoothstep(-3.06, -2.52, vWorldPos.y))
      : 1.0;
    ao = streetAO;
    directLight = sunColor * ndlExt * extShadow;

    // Sunlit Summer Grass & Leaf Subsurface Golden-Chartreuse Translucency (image-18 foreground grass!)
    if (mat == 12 && dayMix > 0.1) {
      float grassRim = pow(clamp(1.0 - abs(dot(N, V)), 0.0, 1.0), 1.4) + 0.45 * max(N.y, 0.0);
      directLight += vec3(0.68, 0.92, 0.14) * grassRim * extShadow * dayMix * 0.55;
    }
  }

  // Translucent Golden Washi Subsurface Glow on Shoji Screen, Chouchin Lanterns & Frosted Door
  if (mat == 5) {
    float backLit = max(dot(-N, uSunDir), 0.28) * smoothstep(0.0, 0.35, uWarmth);
    directLight += sunColor * backLit * (indoor ? 0.38 : 0.22);
  }

  // Grounded cinematic shadow ambient (deep cobalt-slate sky fill + subtle warm wood/stone bounce — ~13:1 contrast!)
  float windowVis = indoor ? clamp((vWorldPos.z + 2.3) / 4.4, 0.32, 1.0) : 1.0;
  float shojiDim = indoor ? (1.0 - 0.34 * uShojiClosed) : 1.0;
  vec3 skyFill = mix(
    vec3(0.010, 0.014, 0.026),
    mix(vec3(0.055, 0.048, 0.078), vec3(0.068, 0.086, 0.112), dayMix),
    clamp(dayMix + duskMix, 0.0, 1.0)
  ) * (0.48 + 0.52 * max(N.y, 0.0));
  vec3 warmBounce = mix(
    vec3(0.012, 0.010, 0.014),
    vec3(0.115, 0.088, 0.054),
    dayMix
  ) * (0.42 + 0.85 * max(-N.y, 0.18));
  vec3 washiFill = vec3(0.95, 0.76, 0.44) * (sunMask.y * 0.24 + (indoor ? 0.06 * (1.0 - 0.5 * uShojiClosed) * dayMix : 0.0));

  vec3 ambient = ((skyFill + warmBounce) * windowVis * shojiDim + washiFill) * ao;
  if (!indoor) {
    // Deep rich summer shadow ambient: cool cerulean sky bounce from above + subtle warm sunlit-asphalt bounce from below!
    vec3 extSky = mix(vec3(0.008, 0.013, 0.026), vec3(0.036, 0.058, 0.098), dayMix) * (0.38 + 0.62 * max(N.y, 0.0));
    vec3 extGround = mix(vec3(0.004, 0.004, 0.007), vec3(0.026, 0.020, 0.012), dayMix) * (0.35 + 0.65 * max(-N.y, 0.12));
    ambient = (extSky + extGround) * ao;
  }

  // Interior Warm 2700K Architectural & Task Point Lights + Coffered Ceiling Cove Wash
  vec3 localLights = vec3(0.0);
  if (indoor) {
    localLights += vec3(1.0, 0.80, 0.48) * (0.09 * smoothstep(2.10, 2.68, vWorldPos.y));
    if (uLampOn > 0.5) {
      vec3 toAndon = vec3(-2.25, 0.76, 1.70) - vWorldPos;
      float dA2 = dot(toAndon, toAndon) + 0.30;
      localLights += vec3(1.0, 0.66, 0.28) * (max(dot(N, normalize(toAndon)), 0.16) * 0.28 / dA2);

      vec3 toDesk = vec3(-0.26, 1.06, 1.46) - vWorldPos;
      float dD2 = dot(toDesk, toDesk) + 0.18;
      localLights += vec3(1.0, 0.74, 0.38) * (max(dot(N, normalize(toDesk)), 0.10) * 0.24 / dD2);
    }
    vec3 toGalley = vec3(1.98, 1.66, 1.45) - vWorldPos;
    float dG2 = dot(toGalley, toGalley) + 0.50;
    localLights += vec3(1.0, 0.70, 0.34) * (max(dot(N, normalize(toGalley)), 0.06) * 0.15 / dG2);

    vec3 toBed = vec3(1.48, 1.75, -1.46) - vWorldPos;
    float dB2 = dot(toBed, toBed) + 0.55;
    float bedAlcoveMask = smoothstep(0.32, 0.44, vWorldPos.x);
    localLights += vec3(0.96, 0.64, 0.30) * (max(dot(N, normalize(toBed)), 0.06) * 0.15 * bedAlcoveMask / dB2);

    if (vWorldPos.x < -0.35 && vWorldPos.z < -0.18) {
      vec3 toBath = vec3(-1.42, 2.52, -1.18) - vWorldPos;
      float dT2 = dot(toBath, toBath) + 0.38;
      localLights += vec3(1.0, 0.92, 0.78) * (max(dot(N, normalize(toBath)), 0.30) * 0.62 / dT2);
    }
    // Natural coastal sky spill pouring through the open -X Apartment Entry Doorway into the genkan!
    if (uEntryDoorOpen > 0.04 && dayMix > 0.05) {
      vec3 toEntry = vec3(-2.62, 1.15, 0.70) - vWorldPos;
      float dE2 = dot(toEntry, toEntry) + 0.42;
      localLights += vec3(0.76, 0.88, 1.0) * (uEntryDoorOpen * dayMix * max(dot(N, normalize(toEntry)), 0.18) * 0.48 / dE2);
    }
  }

  // Physical Micro-Facet Specular Glints on Kawara Tiles, Corrugated Steel & Rounded Wood Fillets
  vec3 extSpecSun = normalize(mix(
    normalize(vec3(-0.38, max(uSunDir.y, 0.68), max(uSunDir.z, 0.46))),
    normalize(vec3(-0.38, max(uSunDir.y, 0.68), -0.44)),
    smoothstep(8.2, 9.8, vWorldPos.z)
  ));
  vec3 specDir = indoor ? uSunDir : extSpecSun;
  vec3 H = normalize(specDir + V);
  float specPow = mix(110.0, 14.0, rough);
  float spec = pow(max(dot(N, H), 0.0), specPow) * (1.0 - rough) * extShadow;
  vec3 specLight = sunColor * spec * 0.24;

  vec3 litColor = baseCol * (ambient + directLight + localLights) + specLight;

  // Subtle Analytical Golden Sunbeam Shaft inside the Room
  if (indoor && dayMix > 0.25 && uSunDir.z > 0.08) {
    vec3 ray = vWorldPos - uCamPos;
    float rayLen = clamp(length(ray), 0.0, 4.5);
    vec2 m1 = computeIndoorSunMask(uCamPos + ray * 0.38);
    vec2 m2 = computeIndoorSunMask(uCamPos + ray * 0.72);
    float shaft = (m1.x + m2.x) * 0.5;
    litColor += vec3(1.0, 0.84, 0.52) * shaft * rayLen * 0.014 * dayMix * (1.0 - 0.82 * uShojiClosed);
  }

  // Summer coastal atmospheric perspective on distant +Z AND -Z forested mountain ridges (|z| > 62 m)
  if (!indoor && abs(vWorldPos.z) > 62.0) {
    float haze = 1.0 - exp(-(abs(vWorldPos.z) - 62.0) * 0.0026);
    vec3 hazeCol = mix(vec3(0.03, 0.05, 0.10), vec3(0.18, 0.39, 0.58), dayMix);
    hazeCol += vec3(1.0, 0.76, 0.42) * uPlumeGlow * 0.12;
    litColor = mix(litColor, hazeCol, clamp(haze, 0.0, 0.54));
  }

  vec3 x = max(litColor, vec3(0.0));
  vec3 mapped = (x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14);
  if (!indoor) {
    // Crisp Summer-Noon filmic contrast & sunlit vibrance (image-18 golden highs + deep espresso shadows!)
    mapped = pow(clamp(mapped, 0.0, 1.0), vec3(1.07));
    float luma = dot(mapped, vec3(0.2126, 0.7152, 0.0722));
    mapped = clamp(mix(vec3(luma), mapped, 1.0 + 0.14 * dayMix), 0.0, 1.0);
  }
  fragColor = vec4(pow(clamp(mapped, 0.0, 1.0), vec3(1.0 / 2.2)), 1.0);
}
`;
