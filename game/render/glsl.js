/* Odderon — shaders.
 *
 * Two full-screen-ish passes and six draws for the whole room (docs/PERFORMANCE.md I5, I7):
 *   1. an SDF scene pass, ray-marched from the camera, which draws the room, the pipe, the bed and
 *      the window as *analytic geometry* (no meshes, no textures, no assets)
 *   2. a grade/tonemap pass
 *
 * Everything that would be a texture in another engine is a function here: corrugated siding, roof
 * tiles, weld beads, dust in the light, tree and power-line shadows (analytic gobos), and the sky.
 */
(function (global) {
  'use strict';

  var VERT = [
    '#version 300 es',
    'in vec2 aPos;',
    'out vec2 vUV;',
    'void main(){ vUV = aPos*0.5+0.5; gl_Position = vec4(aPos,0.0,1.0); }'
  ].join('\n');

  /* ------------------------------------------------------------------ scene
   * Camera is a person standing in a 25 m2 room. Units are metres.
   * Room: x in [-2.6, 2.6] (window wall at +z), z in [-2.4, 2.4], y in [0, 2.5]
   */
  var SCENE = [
    '#version 300 es',
    'precision highp float;',
    'in vec2 vUV;',
    'out vec4 fragColor;',

    'uniform vec2  uRes;',
    'uniform float uTime;',
    'uniform vec3  uCamPos;',
    'uniform vec3  uCamFwd;',
    'uniform vec3  uCamRight;',
    'uniform vec3  uCamUp;',
    'uniform float uFov;',

    // sun / sky, supplied by the kernel (world.js -> light())
    'uniform vec3  uSunDir;',
    'uniform vec3  uSunCol;',
    'uniform vec3  uSkyZenith;',
    'uniform vec3  uSkyHorizon;',
    'uniform float uSunElev;',
    'uniform float uDay;',
    'uniform float uCurtain;',    // 1 = open, 0.05 = closed
    'uniform float uACOn;',
    'uniform float uRate;',
    'uniform float uHour;',
    'uniform float uPeak;',       // epistemic peak glow, 0..1
    'uniform float uWarmth;',     // golden-hour weight, 0..1

    'float hash11(float p){ p = fract(p*0.1031); p *= p+33.33; p *= p+p; return fract(p); }',
    'float hash21(vec2 p){ vec3 p3=fract(vec3(p.xyx)*0.1031); p3+=dot(p3,p3.yzx+33.33); return fract((p3.x+p3.y)*p3.z); }',
    'float noise(vec2 p){',
    '  vec2 i=floor(p), f=fract(p); vec2 u=f*f*(3.0-2.0*f);',
    '  return mix(mix(hash21(i),hash21(i+vec2(1,0)),u.x), mix(hash21(i+vec2(0,1)),hash21(i+vec2(1,1)),u.x), u.y);',
    '}',

    // ---------- SDF primitives ----------
    'float sdBox(vec3 p, vec3 b){ vec3 q=abs(p)-b; return length(max(q,0.0))+min(max(q.x,max(q.y,q.z)),0.0); }',
    'float sdCyl(vec3 p, float r, float h){ vec2 d=abs(vec2(length(p.xz),p.y))-vec2(r,h); return min(max(d.x,d.y),0.0)+length(max(d,0.0)); }',

    // Analytic gobos: projected ellipses (leaves) + a catenary (power line), along the sun azimuth.
    // This is the whole of shadowing in Odderon -- there is not a shadow map anywhere.
    'float gobo(vec3 wp){',
    '  float s = 0.0;',
    '  vec2 p = wp.xz;',
    '  float az = atan(uSunDir.x, uSunDir.z);',
    '  mat2 R = mat2(cos(az),-sin(az),sin(az),cos(az));',
    '  vec2 q = R*p;',
    '  // five leaf ellipses, drifting very slowly in the breeze',
    '  for(int i=0;i<5;i++){',
    '    float fi=float(i);',
    '    vec2 c = vec2(sin(fi*2.3)*1.6+uTime*0.008*sin(fi), cos(fi*1.7)*1.9);',
    '    vec2 d = (q-c)/vec2(0.42,0.30);',
    '    float e = length(d);',
    '    s = max(s, smoothstep(1.0,0.82,e));',
    '  }',
    '  // the power line: a catenary y = a*cosh(x/a) projected onto the floor',
    '  float line = abs(q.y - (0.9 + 0.06*cosh(q.x*1.2)));',
    '  s = max(s, 1.0-smoothstep(0.012,0.028,line));',
    '  return s;',
    '}',

    // ---------- the room ----------
    // The room is a box seen from inside. Distance to the walls (positive, inside-out).
    'float roomD(vec3 p){',
    '  vec3 b = vec3(2.6,2.5,2.4);',
    '  vec3 q = abs(p - vec3(0.0,1.25,0.0));',
    '  vec3 d = b - q;',
    '  return min(min(d.x,d.y),d.z);',
    '}',
    // Props are SDFs. They are the only meshed things in the room, and they are three primitives.
    'vec2 props(vec3 p){',
    '  vec2 res = vec2(1e9, 0.0);',
    '  float d;',
    '  d = sdBox(p-vec3(-1.55,0.74,0.55), vec3(0.55,0.03,0.32));  if(d<res.x) res=vec2(d,2.0);',
    '  d = sdBox(p-vec3( 1.45,0.30,0.95), vec3(0.62,0.28,1.05));  if(d<res.x) res=vec2(d,3.0);',
    '  d = sdCyl(vec3(p.x-2.44, p.y-1.92, p.z), 0.055, 2.2);       if(d<res.x) res=vec2(d,4.0);',
    '  return res;',
    '}',

    // Is this point inside the window opening? The window is a hole in the +z wall, not a box in
    // front of it -- modelling it as a box was a real bug: it could never be the nearest hit.
    'bool inWindow(vec3 p){',
    '  return p.z > 2.36 && abs(p.x) < 0.85 && abs(p.y-1.42) < 0.62;',
    '}',

    'vec3 normalAt(vec3 p, float mat){',
    '  if (mat > 0.5 && mat < 1.5) {',
    '    // wall normal: the axis of largest |p - centre| relative to the box half-extents',
    '    vec3 q = abs(p - vec3(0.0,1.25,0.0)) / vec3(2.6,2.5,2.4);',
    '    vec3 e = step(q.yzx, q.xyz) * step(q.zxy, q.xyz);',
    '    vec3 s = sign(p - vec3(0.0,1.25,0.0));',
    '    return -normalize(e*s);',
    '  }',
    '  vec2 h = vec2(0.0015, 0.0);',
    '  return normalize(vec3(props(p+h.xyy).x-props(p-h.xyy).x,',
    '                         props(p+h.yxy).x-props(p-h.yxy).x,',
    '                         props(p+h.yyx).x-props(p-h.yyx).x));',
    '}',

    // Corrugated siding: a sine perturbation of the normal, in the shader. Zero geometry.
    'vec3 corrugate(vec3 n, vec3 p){',
    '  if (abs(n.x) < 0.5) return n;',
    '  float s = 0.020;',
    '  n.y += s*cos(p.z*34.0)*0.06;',
    '  n.z += s*cos(p.y*34.0)*0.03;',
    '  return normalize(n);',
    '}',

    // March the props; hit the walls analytically. Returns material in .y (0 = sky).
    'vec2 trace(vec3 ro, vec3 rd){',
    '  float t = 0.02;',
    '  for(int i=0;i<72;i++){',
    '    vec2 h = props(ro + rd*t);',
    '    if (h.x < 0.0018) return vec2(t, h.y);',
    '    t += max(h.x*0.85, 0.004);',
    '    if (t > 20.0) break;',
    '  }',
    '  // analytic: where does the ray leave the room? (box seen from inside)',
    '  vec3 b = vec3(2.6,2.5,2.4);',
    '  vec3 o = ro - vec3(0.0,1.25,0.0);',
    '  vec3 inv = 1.0/rd;',
    '  vec3 tm = max((-b-o)*inv, (b-o)*inv);',
    '  float tw = min(min(tm.x,tm.y),tm.z);',
    '  if (tw < 0.0 || tw > 20.0) return vec2(1e9, 0.0);',
    '  vec3 hp = ro + rd*tw;',
    '  if (inWindow(hp)) return vec2(tw, 5.0);',   // the aperture: sky, dimmed by the curtain
    '  return vec2(tw, 1.0);',
    '}',

    'float shadow(vec3 p, float k){',
    '  // analytic gobo multiply -- sun occlusion without any depth buffer',
    '  float g = gobo(p);',
    '  return mix(1.0, 0.25, g*k);',
    '}',

    // The sky is a function. Rayleigh-ish gradient plus an SDF cumulus bank; no cubemap, no asset.
    'vec3 sky(vec3 rd){',
    '  float h = clamp(rd.y*0.5+0.5, 0.0, 1.0);',
    '  vec3 c = mix(uSkyHorizon, uSkyZenith, pow(h,0.7));',
    '  float sun = max(dot(rd, uSunDir), 0.0);',
    '  c += uSunCol * pow(sun, 220.0) * 3.0;',
    '  c += uSunCol * pow(sun, 8.0) * 0.12 * uDay;',
    '  // cumulus: a few billows, 4 octaves, only above the horizon',
    '  if (rd.y > 0.02) {',
    '    vec2 q = rd.xz/(rd.y+0.08)*0.7;',
    '    float f = 0.0, a = 0.5;',
    '    for(int i=0;i<4;i++){ f += a*noise(q); q*=2.02; a*=0.5; }',
    '    f = smoothstep(0.48,0.72,f);',
    '    vec3 cloud = mix(vec3(0.55,0.58,0.64), vec3(1.0), smoothstep(0.5,0.9,f));',
    '    cloud += uSunCol*pow(sun,4.0)*0.5;',
    '    c = mix(c, cloud, f*0.85*uDay);',
    '  }',
    '  return c;',
    '}',

    'void main(){',
    '  vec2 uv = (gl_FragCoord.xy - 0.5*uRes)/uRes.y;',
    '  vec3 rd = normalize(uCamFwd + uCamRight*uv.x*uFov + uCamUp*uv.y*uFov);',
    '  vec3 ro = uCamPos;',

    '  vec2 hit = trace(ro, rd);',
    '  float t = hit.x; float mat = hit.y;',
    '  vec3 col;',
    '  if (mat < 0.5) {',
    '    col = sky(rd);',
    '  } else {',
    '    vec3 p = ro + rd*min(t, 19.0);',
    '    vec3 n = normalAt(p, mat);',
    '    if (mat > 3.5) n = corrugate(n, p);',
    '    vec3 alb;',
    '    if (mat < 1.5)      alb = vec3(0.86,0.80,0.70);',           // plaster
    '    else if (mat < 2.5) alb = vec3(0.42,0.30,0.20);',           // desk: wood
    '    else if (mat < 3.5) alb = vec3(0.78,0.76,0.72);',           // bed
    '    else if (mat < 4.5) alb = vec3(0.22,0.24,0.16);',           // the pipe: painted steel
    '    else                alb = vec3(1.0);',                      // window aperture
    '    if (mat < 4.5) alb *= 0.92 + 0.16*noise(p.xz*14.0 + p.y*3.0);',
    '    float dif = max(dot(n, uSunDir), 0.0);',
    '    float sh  = shadow(p, smoothstep(0.0,0.35,uSunElev));',
    '    vec3 lit = alb * (uSunCol*dif*sh*1.6',
    '              + uSkyZenith*0.55',
    '              + vec3(0.32,0.26,0.18)*0.30*max(-n.y,0.0));',
    '    lit *= mix(0.72, 1.0, smoothstep(0.0, 0.55, p.y));',        // contact darkening
    '    col = lit;',
    '    if (mat > 4.5) {',
    // the window: what is outside, seen through the glass, dimmed by the curtain
    '      vec3 out_ = sky(normalize(vec3(rd.x, rd.y*0.7 - 0.05, 0.55)));',
    '      col = out_ * uCurtain * 1.5 + vec3(0.04);',
    '    }',
    '  }',
    // volumetric dust in the sunbeams: a cheap analytic shaft, gated by the sun angle
    '  float shaft = 0.0;',
    '  if (uDay > 0.1) {',
    '    vec3 sp = ro + rd*1.2;',
    '    shaft = exp(-abs(sp.x+0.35)*1.4) * exp(-abs(sp.y-1.75)*2.2) * (0.55+0.45*noise(sp.xy*3.0+uTime*0.03));',
    '    shaft *= smoothstep(0.55,0.9,dot(uSunDir, vec3(0.3,0.75,-0.58)));',
    '  }',
    '  col += uSunCol*shaft*0.22*uDay*uCurtain;',
    // golden-hour weighting from the kernel, not from the shader's own opinion',
    '  col = mix(col, col*vec3(1.28,0.92,0.68), uWarmth*0.65);',
    '  // the epistemic-peak glow: a warm bloom on the frame edges, one frame, never a UI element',
    '  float vig = 1.0 - length(uv)*0.55;',
    '  col += uPeak * vec3(0.16,0.11,0.05) * (1.0-vig) * 2.0;',
    '  col *= vig;',
    // tone: mild filmic, then gamma. One pass.
    '  col = col/(col+vec3(0.85));',
    '  col = pow(col, vec3(1.0/2.2));',
    '  fragColor = vec4(col,1.0);',
    '}'
  ].join('\n');

  var G = { VERT: VERT, SCENE: SCENE };
  if (typeof module !== 'undefined' && module.exports) module.exports = G;
  global.ODGlsl = G;
})(typeof globalThis !== 'undefined' ? globalThis : this);
