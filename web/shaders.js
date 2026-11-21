// Analytical WebGL2 (with WebGL1 fallback) Shaders for Odderon Checkpoint 1A.
// Zero texture files (0 bytes). All lighting, Shoji Kumiko lattice shadows, coastal tree/cable gobos,
// corrugated steel normals, tatami weave, CRT phosphor scanlines, and Shinkai cumulonimbus clouds are pure math.

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
uniform float uShockFrac; // 0..1 expanding shockwave ring from coastal pad

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

// Lightweight 3-octave FBM (PERFORMANCE.md Round 2 budget for 2015 Intel UHD)
float fbm3(vec2 p) {
  float v = 0.52 * noise2(p);
  p = p * 2.07 + vec2(1.7, -1.2);
  v += 0.30 * noise2(p);
  p = p * 2.03 + vec2(-2.3, 0.9);
  v += 0.18 * noise2(p);
  return v;
}

void main() {
  vec3 rd = normalize(
    uCamFwd
    + uCamRight * (vNdc.x * uAspect * uTanHalfFov)
    + uCamUp * (vNdc.y * uTanHalfFov)
  );

  float up = clamp(rd.y, -0.15, 1.0);
  float sunAmt = max(dot(rd, uSunDir), 0.0);

  // Shinkai Summer Noon cyan-cobalt sky vs Golden Hour / Twilight palette
  vec3 noonZenith  = vec3(0.05, 0.34, 0.78);
  vec3 noonHorizon = vec3(0.52, 0.82, 0.96);
  vec3 duskZenith  = vec3(0.08, 0.12, 0.30);
  vec3 duskHorizon = vec3(0.96, 0.44, 0.20);
  vec3 nightZenith = vec3(0.015, 0.025, 0.055);
  vec3 nightHoriz  = vec3(0.04, 0.07, 0.13);

  float dayMix = smoothstep(0.15, 0.75, uWarmth);
  float duskMix = smoothstep(0.0, 0.35, uWarmth) * (1.0 - dayMix);

  vec3 zenithCol = mix(nightZenith, mix(duskZenith, noonZenith, dayMix), clamp(dayMix + duskMix, 0.0, 1.0));
  vec3 horizCol  = mix(nightHoriz,  mix(duskHorizon, noonHorizon, dayMix), clamp(dayMix + duskMix, 0.0, 1.0));

  float hFactor = pow(1.0 - clamp(up, 0.0, 1.0), 2.7);
  vec3 sky = mix(zenithCol, horizCol, hFactor);

  // Mie solar aureole + crisp solar disc
  vec3 sunTint = mix(vec3(1.0, 0.48, 0.18), vec3(1.0, 0.95, 0.80), dayMix);
  sky += sunTint * (0.28 * pow(sunAmt, 8.0) + 1.35 * pow(sunAmt, 280.0)) * smoothstep(-0.04, 0.08, uSunDir.y);

  // Towering Japanese Summer Cumulonimbus Billows over the Pacific horizon (+Z)
  if (rd.y > 0.005 && rd.z > 0.05) {
    vec2 cloudUv = vec2(rd.x / max(rd.z, 0.25), rd.y * 2.15);
    // Sculpted towering anvil + billow silhouette
    float towerA = 1.0 - length((cloudUv - vec2(-0.32, 0.28)) * vec2(1.45, 1.75));
    float towerB = 1.0 - length((cloudUv - vec2( 0.28, 0.22)) * vec2(1.25, 1.95));
    float base   = max(towerA, towerB);
    float n      = fbm3(cloudUv * 5.6);
    float density = smoothstep(0.32, 0.62, base + 0.44 * n) * smoothstep(0.01, 0.06, rd.y);

    if (density > 0.01) {
      // Derivative-like rim illumination toward the sun
      float nShift = fbm3((cloudUv + uSunDir.xy * 0.045) * 5.6);
      float lit = clamp(0.58 + (n - nShift) * 2.6 + rd.y * 0.65, 0.0, 1.0);
      vec3 cloudBright = mix(vec3(1.0, 0.68, 0.42), vec3(1.02, 0.99, 0.94), dayMix);
      vec3 cloudShadow = mix(vec3(0.24, 0.22, 0.36), vec3(0.44, 0.60, 0.80), dayMix);
      vec3 cCol = mix(cloudShadow, cloudBright, lit);
      sky = mix(sky, cCol, density * 0.94);
    }
  }

  // Subtle refractive condensation ring from the 343 m/s Pad-1 static-fire shockwave!
  if (uShockFrac > 0.01 && uShockFrac < 1.15 && rd.z > 0.2) {
    vec2 padDir = normalize(vec2(-17.2, 148.0));
    vec2 rayHoriz = vec2(rd.x, rd.z);
    float angDist = length(vec2(rd.x / max(rd.z, 0.2) - (-17.2 / 148.0), rd.y - 0.06));
    float ringR = uShockFrac * 0.58;
    float ring = exp(-pow((angDist - ringR) * 36.0, 2.0)) * (1.0 - uShockFrac * 0.78);
    sky += vec3(0.85, 0.92, 1.0) * max(ring, 0.0) * 0.28;
    float _unused = padDir.x + rayHoriz.x;
    if (_unused > 1e9) sky.r += 0.0;
  }

  // Warm filmic tonemap
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

// Analytical window sunbeam + Shoji Kumiko lattice shadow + swaying tree/powerline gobos!
// Zero shadow maps (0 MB VRAM), crisp infinite-resolution edges!
vec2 computeIndoorSunMask(vec3 p) {
  // Returns vec2(directBeam, warmDiffusedWashi)
  if (uSunDir.z <= 0.04 || uSunDir.y <= 0.01) {
    return vec2(0.0, 0.0);
  }
  // Ray from indoor point p toward sun (+uSunDir) intersecting window plane z = 1.94
  float t = (1.94 - p.z) / uSunDir.z;
  if (t <= 0.0) {
    return vec2(1.0, 0.0); // Exterior point outside window!
  }
  vec2 hit = p.xy + uSunDir.xy * t;
  // Window opening bounds: x in [-1.53, +1.28], y in [0.77, 2.24]
  if (hit.x < -1.53 || hit.x > 1.28 || hit.y < 0.77 || hit.y > 2.24) {
    return vec2(0.0, 0.0);
  }
  // Center wooden mullion shadow (-0.14 .. -0.09)
  if (hit.x > -0.145 && hit.x < -0.085) {
    return vec2(0.0, 0.0);
  }

  float shojiWidth = 0.24 + clamp(uShojiClosed, 0.0, 1.0) * (2.81 - 0.24);
  float shojiRightEdge = -1.53 + shojiWidth;

  if (hit.x <= shojiRightEdge) {
    // Passed through Japanese Shoji screen: Kumiko wooden grid casts soft bars, washi diffuses gold light!
    float gx = abs(fract((hit.x + 1.53) / 0.28) - 0.5);
    float gy = abs(fract((hit.y - 0.77) / 0.294) - 0.5);
    float barShadow = smoothstep(0.43, 0.47, max(gx, gy));
    return vec2(0.08 * (1.0 - barShadow), 0.52 * (1.0 - 0.45 * barShadow));
  } else {
    // Open window glass: project analytical power-line catenary & coastal tree-leaf gobos!
    float catenaryY = 1.88 - 0.14 * (1.0 - hit.x * hit.x * 0.35);
    float wireShadow = 1.0 - 0.55 * exp(-pow((hit.y - catenaryY) * 55.0, 2.0));
    // Dappled coastal foliage gobo in upper-left corner of window
    float leafField = sin(hit.x * 11.0 + hit.y * 7.0) * cos(hit.x * 8.5 - hit.y * 13.0);
    float cornerDist = length(hit - vec2(-1.15, 2.05));
    float leafGobo = (cornerDist < 0.72 && leafField > 0.25) ? 0.35 : 1.0;
    return vec2(wireShadow * leafGobo, 0.08);
  }
}

void main() {
  vec3 N = normalize(vNormal);
  vec3 V = normalize(uCamPos - vWorldPos);
  vec3 baseCol = vAlbedo;
  int mat = int(vMat + 0.5);

  // Emissive materials (6 = Warm 2700K LED/Andon/China-Marker, 8 = CRT Vector Scope, 9 = Rocket Plume, 10 = Eidos AR)
  if (mat == 6) {
    fragColor = vec4(baseCol, 1.0);
    return;
  }
  if (mat == 8) {
    // Analytical CRT vector telemetry trace on desk oscilloscope!
    float scan = 0.88 + 0.12 * sin(vWorldPos.y * 420.0);
    float wave = exp(-abs((vWorldPos.y - 0.93) - 0.045 * sin(vWorldPos.x * 34.0)) * 95.0);
    vec3 crt = baseCol * scan + vec3(0.55, 1.0, 0.92) * wave * 0.85;
    fragColor = vec4(clamp(crt, 0.0, 1.0), 1.0);
    return;
  }
  if (mat == 9) {
    // Sunlit rocket exhaust & white condensation cloud
    float rim = 0.78 + 0.22 * max(N.y, 0.0);
    fragColor = vec4(clamp(baseCol * rim * 1.12, 0.0, 1.0), 1.0);
    return;
  }
  if (mat == 10) {
    // Eidos causal sensitivity vector pulse
    float pulse = 0.80 + 0.20 * sin((vWorldPos.x + vWorldPos.y + vWorldPos.z) * 18.0);
    fragColor = vec4(clamp(baseCol * pulse, 0.0, 1.0), 1.0);
    return;
  }

  // Analytical zero-texture procedural surface micro-detail
  float rough = 0.65;
  if (mat == 1) {
    // Hinoki / Cedar wood grain rings
    float grain = sin(vWorldPos.x * 42.0 + sin(vWorldPos.z * 9.0) * 2.5) * 0.045;
    baseCol *= (1.0 + grain);
    rough = 0.42;
  } else if (mat == 2) {
    // Woven Japanese Tatami rush grass
    float weave = sin(vWorldPos.z * 210.0) * 0.055 + sin(vWorldPos.x * 16.0) * 0.02;
    baseCol *= (1.0 + weave);
    rough = 0.72;
  } else if (mat == 3) {
    // Corrugated galvanized metal siding & roofs (sine-wave normal perturbation!)
    float corr = cos((vWorldPos.x + vWorldPos.z * 0.35) * 38.0);
    N = normalize(N + vec3(corr * 0.28, 0.0, 0.0));
    rough = 0.32;
  } else if (mat == 4) {
    // Star Citizen brushed hab alloy
    float seam = smoothstep(0.012, 0.022, abs(fract(vWorldPos.y * 1.5) - 0.5));
    baseCol *= (0.92 + 0.08 * seam);
    rough = 0.28;
  } else if (mat == 7) {
    // Sparkling Pacific Ocean sun-glitter waves!
    float waveA = sin(vWorldPos.x * 0.65 + vWorldPos.z * 0.42);
    float waveB = cos(vWorldPos.x * 0.31 - vWorldPos.z * 0.88);
    N = normalize(vec3(waveA * 0.08, 1.0, waveB * 0.08));
    rough = 0.14;
  }

  // Stylized Physically-Based Shinkai Lighting ("Shadows of a Summer Noon"):
  // 1. Direct 5500K warm amber solar illumination vs 2. Cool cyan-violet Rayleigh sky shadow bounce!
  float dayMix = smoothstep(0.12, 0.72, uWarmth);
  vec3 sunColor = mix(vec3(1.05, 0.46, 0.16), vec3(1.18, 1.02, 0.78), dayMix) * smoothstep(-0.02, 0.12, uSunDir.y);

  bool indoor = (vWorldPos.z < 1.945 && vWorldPos.x > -2.42 && vWorldPos.x < 2.42 && vWorldPos.y < 2.63);
  vec2 sunMask = indoor ? computeIndoorSunMask(vWorldPos) : vec2(1.0, 0.0);

  float ndl = max(dot(N, uSunDir), 0.0);
  // Subtle stylized anime-realism soft band on direct sunlight
  float stylizedNdl = mix(ndl, smoothstep(0.04, 0.28, ndl), 0.42);
  vec3 directLight = sunColor * stylizedNdl * sunMask.x;

  // Shoji Washi paper subsurface golden glow!
  if (mat == 5) {
    float backLit = max(dot(-N, uSunDir), 0.25) * smoothstep(0.0, 0.3, uWarmth);
    directLight += sunColor * backLit * 0.72;
  }

  // Cool cyan sky bounce from window (+Z / +Y) + warm green/tatami floor bounce (-Y)
  float windowVis = indoor ? clamp((vWorldPos.z + 2.2) / 4.2, 0.22, 1.0) : 1.0;
  float shojiDim = indoor ? (1.0 - 0.44 * uShojiClosed) : 1.0;
  vec3 skyBounce = mix(vec3(0.05, 0.07, 0.13), vec3(0.24, 0.40, 0.62), dayMix) * (0.55 + 0.45 * max(N.y, 0.0));
  vec3 groundBounce = mix(vec3(0.04, 0.04, 0.05), vec3(0.30, 0.27, 0.18), dayMix) * (0.5 + 0.5 * max(-N.y, 0.0));
  vec3 washiAmbient = vec3(1.0, 0.84, 0.56) * (sunMask.y * 0.42 + (indoor ? 0.16 * uShojiClosed * dayMix : 0.0));

  vec3 ambient = (skyBounce + groundBounce) * windowVis * shojiDim + washiAmbient;

  // Warm 2700K Japantown Shoji Floor Andon Lantern + Desk Task Lamp point lights!
  vec3 localLights = vec3(0.0);
  if (indoor && uLampOn > 0.5) {
    // Floor Andon at (2.02, 0.55, 0.78)
    vec3 toAndon = vec3(2.02, 0.55, 0.78) - vWorldPos;
    float dA2 = dot(toAndon, toAndon) + 0.28;
    float lA = max(dot(N, normalize(toAndon)), 0.15) / dA2;
    // Desk Lamp at (0.14, 1.04, 1.44)
    vec3 toDesk = vec3(0.14, 1.04, 1.44) - vWorldPos;
    float dD2 = dot(toDesk, toDesk) + 0.18;
    float lD = max(dot(N, normalize(toDesk)), 0.1) / dD2;
    localLights += vec3(1.0, 0.66, 0.28) * (lA * 0.52 + lD * 0.28);
  }
  // Bunk warm amber LED strip fill in back-left alcove
  if (indoor) {
    vec3 toBunk = vec3(-1.45, 1.74, -1.36) - vWorldPos;
    float dB2 = dot(toBunk, toBunk) + 0.45;
    localLights += vec3(0.95, 0.62, 0.25) * (max(dot(N, normalize(toBunk)), 0.0) * 0.16 / dB2);
  }

  // Blinn-Phong specular highlight on wood, alloy, and sparkling Pacific Ocean
  vec3 H = normalize(uSunDir + V);
  float specPow = mix(96.0, 12.0, rough);
  float spec = pow(max(dot(N, H), 0.0), specPow) * (1.0 - rough) * sunMask.x;
  vec3 specLight = sunColor * spec * (mat == 7 ? 1.85 : 0.35);

  vec3 litColor = baseCol * (ambient + directLight + localLights) + specLight;

  // Distant coastal aerial perspective haze for exterior objects (z > 12m)
  if (!indoor && vWorldPos.z > 8.0) {
    float haze = 1.0 - exp(-(vWorldPos.z - 8.0) * 0.0032);
    vec3 hazeCol = mix(vec3(0.12, 0.16, 0.28), vec3(0.58, 0.83, 0.96), dayMix);
    hazeCol += vec3(1.0, 0.78, 0.45) * uPlumeGlow * 0.12;
    litColor = mix(litColor, hazeCol, clamp(haze, 0.0, 0.72));
  }

  // ACES-inspired warm filmic tonemapping + gamma 2.2
  vec3 x = max(litColor, vec3(0.0));
  vec3 mapped = (x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14);
  fragColor = vec4(pow(clamp(mapped, 0.0, 1.0), vec3(1.0 / 2.2)), 1.0);
}
`;
