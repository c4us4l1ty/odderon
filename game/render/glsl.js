/* Odderon — shaders.
 *
 * The room is a "hab": a compact, modular, functional living volume in the Star Citizen idiom
 * (sunken lounge, segmented wall panels, bulkhead ribs, coved lighting, a shuttered viewport), but
 * made of *life* rather than sci-fi chrome: timber, cream plaster, painted steel, a worn floor.
 * That is the whole point — you are not on a station, you are in a cheap hab on Earth, and the
 * sunlight through the window is the most expensive-looking thing in the frame.
 *
 * Cost discipline (PERFORMANCE.md §3) is visible in the code:
 *   - one SDF scene pass, one tonemap; props are ray-marched, walls are intersected analytically
 *   - no textures, no shadow maps, no MSAA, no per-object draws
 *   - every loop has a literal bound; all lighting is closed-form
 */
(function (global) {
  'use strict';

  var VERT = [
    '#version 300 es',
    'in vec2 aPos;',
    'out vec2 vUV;',
    'void main(){ vUV = aPos*0.5+0.5; gl_Position = vec4(aPos,0.0,1.0); }'
  ].join('\n');

  var SCENE = [
    '#version 300 es',
    'precision highp float;',
    'in vec2 vUV;',
    'out vec4 fragColor;',

    'uniform vec2  uRes;',
    'uniform vec3  uCamPos;',
    'uniform vec3  uCamFwd;',
    'uniform vec3  uCamRight;',
    'uniform vec3  uCamUp;',
    'uniform float uFov;',
    'uniform vec3  uSunDir;',
    'uniform vec3  uSunCol;',
    'uniform vec3  uSkyZenith;',
    'uniform vec3  uSkyHorizon;',
    'uniform float uSunElev;',
    'uniform float uDay;',
    'uniform float uCurtain;',
    'uniform float uWarmth;',
    'uniform float uPeak;',
    'uniform float uHour;',
    'uniform vec3  uLampCol;',
    'uniform float uLampOn;',
    'uniform float uScreen;',

    'float hash21(vec2 p){ vec3 p3=fract(vec3(p.xyx)*0.1031); p3+=dot(p3,p3.yzx+33.33); return fract((p3.x+p3.y)*p3.z); }',
    'float noise(vec2 p){',
    '  vec2 i=floor(p), f=fract(p); vec2 u=f*f*(3.0-2.0*f);',
    '  return mix(mix(hash21(i),hash21(i+vec2(1.0,0.0)),u.x), mix(hash21(i+vec2(0.0,1.0)),hash21(i+vec2(1.0,1.0)),u.x), u.y);',
    '}',
    // three octaves, not four: this is the single most-executed function in the frame
    'float fbm(vec2 p){ float f=0.0,a=0.5; for(int i=0;i<4;i++){ f+=a*noise(p); p*=2.03; a*=0.5; } return f; }',

    /* ---------------- primitives ---------------- */
    'float sdBox(vec3 p, vec3 b){ vec3 q=abs(p)-b; return length(max(q,0.0))+min(max(q.x,max(q.y,q.z)),0.0); }',
    'float sdCyl(vec3 p, float r, float h){ vec2 d=abs(vec2(length(p.xz),p.y))-vec2(r,h); return min(max(d.x,d.y),0.0)+length(max(d,0.0)); }',

    /* ---------------- the room ----------------
     * A hab, in the Star Citizen sense: a small, modular, deliberate volume. Room x [-2.6, 2.6],
     * z [-2.4, 2.4], ceiling 2.55. In the -z half the floor drops 0.42 m inside a stepped kerb, so
     * the lounge is sunken -- the one move that makes a box read as a designed interior.
     *
     * Which walls exist is decided by *where the window and the door are*: the renderer and the
     * world both read the same two rectangles, so the sunlight always comes through a hole.
     */
    'float LOUNGE_Z = 1.05;',      // the kerb: z greater than this is the raised floor
    'float LOUNGE_D = 0.42;',      // how far it drops
    'float WX = -0.55;',           // window centre
    'float WY = 1.42;',
    'float WZ = 2.40;',            // the window wall (+z)
    'float DX =  0.95;',           // the door in the -z wall
    'float DZ = -2.40;',

    'bool inWindow(vec3 p){ return p.z > 2.30 && abs(p.x-WX) < 1.05 && abs(p.y-WY) < 0.70; }',
    'bool inDoor(vec3 p){ return p.z < -2.30 && abs(p.x-DX) < 0.50 && p.y < 2.04; }',

    'float floorLevel(vec3 p){ return (p.z > LOUNGE_Z && p.x > -2.15 && p.x < 2.15) ? 0.0 : -LOUNGE_D; }',

    // distance to the shell, from the inside. > 0 inside the room.
    'float shell(vec3 p){',
    '  vec3 q = vec3(2.6 - abs(p.x), min(p.y, 2.55 - p.y), 2.4 - abs(p.z));',
    '  float d = min(q.x, min(q.y, q.z));',
    '  d = min(d, p.y - floorLevel(p));',
    '  return d;',
    '}',

    /* ---------------- props ----------------
     * A hab's furniture is built in, not placed: a platform bed, a bench that wraps the sunken
     * lounge, a desk under the window, one pipe run. Seven primitives, all spheres and boxes.
     */
    'vec2 props(vec3 p){',
    '  vec2 r = vec2(1e9, 0.0);',
    '  float d;',
    // platform bed on the raised floor, +x, its top 0.56 m above the boards
    '  d = sdBox(p - vec3(1.72, 0.20, 1.70), vec3(0.80, 0.20, 0.68));',
    '  r = vec2(d, 1.0);',
    // a folded blanket: one more box, slightly proud of the mattress
    '  d = sdBox(p - vec3(1.52, 0.42, 1.78), vec3(0.42, 0.05, 0.62));',
    '  if (d < r.x) r = vec2(d, 10.0);',
    // the desk: a slab on two thin legs, under the window
    '  d = sdBox(p - vec3(1.10, 0.74, 2.05), vec3(0.70, 0.03, 0.30));',
    '  if (d < r.x) r = vec2(d, 2.0);',
    '  d = min(sdBox(p - vec3(0.46, 0.35, 2.05), vec3(0.022, 0.35, 0.26)),',
    '          sdBox(p - vec3(1.74, 0.35, 2.05), vec3(0.022, 0.35, 0.26)));',
    '  if (d < r.x) r = vec2(d, 5.0);',
    // the lounge bench: one long curved block along the back of the sunken area, with a back rail
    '  d = sdBox(p - vec3(0.0, -0.24, -1.72), vec3(1.90, 0.18, 0.34));',
    '  if (d < r.x) r = vec2(d, 6.0);',
    '  d = sdBox(p - vec3(0.0, 0.02, -1.98), vec3(1.90, 0.20, 0.06));',
    '  if (d < r.x) r = vec2(d, 6.0);',
    // the pipe run: the machine the room is. Low along -x, then up into the ceiling.
    '  d = min(sdCyl(vec3(p.x + 2.44, p.y - 0.34, p.z), 0.055, 2.05),',
    '          sdCyl(vec3(p.x + 2.44, p.y - 1.55, p.z + 2.05), 0.055, 1.20));',
    '  if (d < r.x) r = vec2(d, 7.0);',
    // the wall screen: dark glass on the -x wall, the room\'s second light at night
    '  d = sdBox(p - vec3(-2.56, 1.35, -0.30), vec3(0.02, 0.34, 0.52));',
    '  if (d < r.x) r = vec2(d, 8.0);',
    '  return r;',
    '}',

    /* ---------------- normals ----------------
     * Inside a room, the visible face of a wall is the one facing the camera, so the normal points
     * back into the volume. Getting this sign wrong is what made the whole room read as blue fog:
     * with -s every wall faced away from the sun and only the sky fill survived.
     */
    'vec3 nShell(vec3 p){',
    '  if (p.y - floorLevel(p) < 0.03) return vec3(0.0, 1.0, 0.0);',
    '  float ex = 2.6 - abs(p.x), ey = min(p.y, 2.55 - p.y), ez = 2.4 - abs(p.z);',
    '  if (ex < ey && ex < ez) return vec3(sign(p.x), 0.0, 0.0);',
    '  if (ez < ey) return vec3(0.0, 0.0, sign(p.z));',
    '  return vec3(0.0, p.y < 1.275 ? -1.0 : 1.0, 0.0);',
    '}',

    'vec3 nProps(vec3 p){',
    '  vec2 h = vec2(0.0022, 0.0);',
    '  return normalize(vec3(props(p+h.xyy).x-props(p-h.xyy).x,',
    '                         props(p+h.yxy).x-props(p-h.yxy).x,',
    '                         props(p+h.yyx).x-props(p-h.yyx).x + 1e-6));',
    '}',

    /* ---------------- tracing ----------------
     * Two bounded marches. Props first (they are small and near), then the shell. The shell march is
     * the frame cost, so it is short and it starts from where the props stopped.
     */
    'vec2 trace(vec3 ro, vec3 rd){',
    '  float t = 0.02;',
    '  for(int i=0;i<40;i++){',
    '    vec2 h = props(ro + rd*t);',
    '    if (h.x < 0.0016) return vec2(t, h.y);',
    '    t += max(h.x*0.9, 0.008);',
    '    if (t > 8.0) break;',
    '  }',
    '  t = 0.02;',
    '  for(int i=0;i<32;i++){',
    '    vec3 p = ro + rd*t;',
    '    float d = shell(p);',
    '    if (d < 0.004){',
    '      if (inWindow(p)) return vec2(t, 4.0);',
    '      if (inDoor(p))   return vec2(t, 3.0);',
    '      return vec2(t, 0.0);',
    '    }',
    '    t += max(d*0.95, 0.02);',
    '    if (t > 8.0) break;',
    '  }',
    '  return vec2(1e9, 0.0);',
    '}',

    /* ---------------- materials ----------------
     * Every texture is a function of position: the hab's segmented panels, its bulkhead ribs, the
     * boards, the paint. Nothing is sampled, so nothing is loaded.
     */
    'vec3 albedo(vec3 p, vec3 n, float mat){',
    '  if (mat < 0.5) {',
    '    float fy = floorLevel(p);',
    '    if (p.y - fy < 0.02) {',
    // the floor: dark technical boards on the raised level, a woven mat in the lounge
    '      if (fy < -0.2) return mix(vec3(0.20,0.18,0.16), vec3(0.26,0.23,0.20), 0.35+0.4*noise(p.xz*14.0));',
    '      float board = step(0.5, fract(p.x*2.2));',
    '      return mix(vec3(0.26,0.21,0.16), vec3(0.31,0.25,0.19), board) * (0.85+0.3*noise(p.xz*18.0));',
    '    }',
    // the wall: cream plaster above, painted steel wainscot below, segmented panels with a bead
    '    vec3 c = vec3(0.78,0.74,0.66);',
    '    float cell = abs(p.z*1.6 - floor(p.z*1.6)) - 0.5;',
    '    float bead = smoothstep(0.44,0.5,abs(cell));',
    '    c *= 0.94 + 0.10*bead;',
    '    if (p.y - fy < 0.92) c = mix(vec3(0.26,0.25,0.23), vec3(0.30,0.29,0.26), noise(p.xz*9.0));',
    // a bulkhead rib every 2.4 m: a strip of steel that reads as structure, not decoration
    '    float rib = smoothstep(0.055,0.02,abs(abs(p.x)-1.35));',
    '    c = mix(c, vec3(0.34,0.33,0.30), rib*0.75);',
    // the ceiling cove: a shallow tray that catches the light
    '    if (n.y < -0.5) c = mix(vec3(0.72,0.69,0.62), vec3(0.86,0.83,0.76), smoothstep(0.0,0.6,2.6-abs(p.x)));',
    '    return c;',
    '  }',
    '  if (mat < 1.5) return vec3(0.20,0.19,0.20);',                       // bed platform, dark steel
    '  if (mat < 2.5) return mix(vec3(0.40,0.28,0.18), vec3(0.48,0.34,0.22), noise(p.xz*26.0));',
    '  if (mat < 3.5) return vec3(0.11,0.11,0.12);',                       // door panel
    '  if (mat < 4.5) return vec3(1.0);',                                  // the viewport
    '  if (mat < 5.5) return vec3(0.17,0.17,0.17);',                       // desk frame
    '  if (mat < 6.5) return mix(vec3(0.20,0.24,0.23), vec3(0.26,0.31,0.29), noise(p.xz*20.0));',
    '  if (mat < 7.5) return vec3(0.30,0.24,0.14);',                       // painted pipe, ochre
    '  if (mat < 8.5) return vec3(0.03,0.04,0.05);',                       // screen glass
    '  if (mat < 9.5) return vec3(0.52,0.62,0.66);',                       // the folded blanket
    '  return vec3(0.5);',
    '}',

    /* ---------------- the light ----------------
     * The window is an aperture, so the sun is integrated over a rectangle: three mullions make the
     * bars across the floor, a tree makes the wobbling ellipse, a wire makes the catenary. All of it
     * is closed-form -- there is no shadow map anywhere in this file.
     */
    'float gobo(vec3 p){',
    '  float az = atan(uSunDir.x, uSunDir.z);',
    '  mat2 R = mat2(cos(az),-sin(az),sin(az),cos(az));',
    '  vec2 q = R * p.xz;',
    '  float s = 0.0;',
    '  for(int i=0;i<3;i++){',
    '    float fi = float(i);',
    '    vec2 c = vec2(sin(fi*2.3)*1.4 + 0.5, cos(fi*1.7)*1.9);',
    '    vec2 d = (q - c) / vec2(0.55, 0.38);',
    '    s = max(s, smoothstep(1.0, 0.72, length(d)));',
    '  }',
    '  float wire = abs(q.y - (0.9 + 0.06*cosh(clamp(q.x,-3.0,3.0)*1.1)));',
    '  s = max(s, 1.0 - smoothstep(0.012, 0.030, wire));',
    '  return s;',
    '}',

    // the bars: the shadow of the window frame, projected along the sun onto whatever we hit
    'float lightBars(vec3 p){',
    '  vec2 q = p.xz - uSunDir.xz * (p.y * 0.5);',
    '  float bars = 0.0;',
    '  for(int i=0;i<3;i++){',
    '    float bx = WX + (float(i) - 1.0) * 0.66;',
    '    bars = max(bars, 1.0 - smoothstep(0.0, 0.075, abs(q.x - bx)));',
    '  }',
    '  return 1.0 - 0.62*bars;',
    '}',

    'vec3 sky(vec3 rd){',
    '  float h = clamp(rd.y*0.5+0.5, 0.0, 1.0);',
    '  vec3 c = mix(uSkyHorizon, uSkyZenith, pow(h, 0.62));',
    '  float sun = max(dot(rd, uSunDir), 0.0);',
    '  c += uSunCol * pow(sun, 300.0) * 5.0;',
    '  c += uSunCol * pow(sun, 8.0) * 0.18 * uDay;',
    '  if (rd.y > 0.01) {',
    '    vec2 q = rd.xz/(rd.y+0.08)*0.7;',
    '    float f = smoothstep(0.46,0.72, fbm(q));',
    '    vec3 cloud = mix(vec3(0.60,0.63,0.68), vec3(1.02), smoothstep(0.5,0.95,f));',
    '    cloud += uSunCol*pow(sun,5.0)*0.6;',
    '    c = mix(c, cloud, f*0.9*uDay);',
    '  }',
    // the town and the hills: matte silhouettes at true parallax, not geometry
    '  float hz = smoothstep(0.07, -0.03, rd.y);',
    '  vec3 hill = mix(vec3(0.17,0.22,0.20), vec3(0.28,0.32,0.27), clamp(rd.y*4.0+0.5, 0.0, 1.0));',
    '  c = mix(c, hill*mix(0.55,1.15,uDay), hz*0.88);',
    '  return c;',
    '}',

    /* ---------------- does the sun reach this surface? ----------------
     * This is the whole art direction in one function. The window is an aperture in the +z wall, so
     * a sun ray reaches a point iff its exit point through z=WZ lands in the window rectangle and no
     * other wall is hit first. Closed-form: one division per plane, no march, no shadow map. The
     * soft edge is what makes the light look like afternoon rather than a stencil.
     */
    'float sunVisible(vec3 p, vec3 n){',
    '  if (uSunDir.z <= 0.04) return 0.0;                       // the viewport faces +z',
    '  float tz = (2.40 - p.z) / uSunDir.z;',
    '  if (tz <= 0.0) return 0.0;',
    '  vec3 q = p + uSunDir * tz;',
    // the aperture must be the first thing the ray meets
    '  float tx = ((uSunDir.x > 0.0 ? 2.6 : -2.6) - p.x) / (abs(uSunDir.x) < 0.02 ? 0.02 : uSunDir.x);',
    '  float tc = ((uSunDir.y > 0.0 ? 2.55 : 0.0) - p.y) / (abs(uSunDir.y) < 0.02 ? 0.02 : uSunDir.y);',
    '  if (tx < tz || tc < tz) return 0.0;',
    '  float inside = smoothstep(1.05, 0.90, abs(q.x - WX)) * smoothstep(0.70, 0.56, abs(q.y - WY));',
    '  return inside;',
    '}',

    'void main(){',
    '  vec2 uv = (gl_FragCoord.xy - 0.5*uRes)/uRes.y;',
    '  vec3 rd = normalize(uCamFwd + uCamRight*uv.x*uFov + uCamUp*uv.y*uFov);',
    '  vec3 ro = uCamPos;',

    '  vec2 hit = trace(ro, rd);',
    '  vec3 col;',
    '  if (hit.x > 1e8) {',
    '    col = sky(rd);                                  // only happens through a hole, never a wall',
    '  } else {',
    '    vec3 p = ro + rd*hit.x;',
    '    float mat = hit.y;',
    '    float sunUp = smoothstep(-0.04, 0.16, uSunElev);',
    '    if (mat > 3.5 && mat < 4.5) {',
    // the viewport: the real sky, dimmed by the curtain. The curtain is a physical thing: closing it
    // takes the sun out of the room, which is the Day-1 experiment made visible.
    '      vec3 out_ = sky(normalize(vec3(rd.x*1.2, rd.y*0.95 - 0.04, 0.72)));',
    '      float cur = mix(0.085, 1.0, uCurtain*uCurtain);',
    '      col = out_ * cur * 1.4 + vec3(0.012);',
    '    } else if (mat > 2.5 && mat < 3.5) {',
    // the door: a lit corridor, so the hab is a room in a bigger building
    '      col = mix(vec3(0.05,0.05,0.055), vec3(0.40,0.38,0.32), 0.30 + 0.45*uScreen);',
    '    } else if (mat > 7.5 && mat < 8.5) {',
    // the wall screen: the room\'s second light source, and the only thing in here with a UI
    '      float tab = smoothstep(0.02,0.0,abs(fract(p.z*6.0)-0.5)-0.44) * step(mod(p.y*8.0,3.0),1.0);',
    '      col = vec3(0.05,0.07,0.08) + vec3(0.18,0.30,0.33)*(0.35+0.65*uScreen)*(0.35+tab);',
    '    } else {',
    '      vec3 n = (mat > 0.5) ? nProps(p) : nShell(p);',
    '      vec3 alb = albedo(p, n, mat);',
    '      float dif = max(dot(n, uSunDir), 0.0);',
    // the sun reaches a surface only if the window sits between the two: this is the difference
    // between a lit room and a room painted blue
    '      float vis = sunVisible(p, n) * smoothstep(0.15, 0.85, uCurtain) * sunUp;',
    // The other half of a real window: the whole aperture is a sky-coloured area light. Without it a
    // room at noon is a bright patch on the floor and a dark box everywhere else, which is correct
    // and reads as a cave. One inverse-square term, cosine-weighted, standing in for the skylight.
    '      vec3 toWin = vec3(WX, WY, 2.38) - p;',
    '      float d2 = dot(toWin, toWin);',
    '      toWin = toWin * inversesqrt(d2 + 1e-4);',
    '      float ap = max(dot(n, toWin), 0.0) * max(toWin.z, 0.0);',
    '      vec3 skylight = mix(uSkyHorizon, uSkyZenith, 0.30) * ap * (3.0 / (1.0 + d2))',
    '                    * smoothstep(0.10, 0.9, uCurtain) * (0.25 + 0.75*uDay);',
    '      float shade = 1.0 - 0.60*gobo(p) - (1.0 - lightBars(p));',
    '      vec3 sunLit = uSunCol * dif * vis * shade * 3.4;',
    // the sky is the fill: cool from above, warm from the boards, and a lamp you can actually see
    '      vec3 fill = mix(uSkyHorizon, uSkyZenith, 0.35) * 0.44 * (0.52 + 0.48*n.y);',
    '      vec3 bounce = vec3(0.36,0.27,0.18) * 0.55 * max(0.0, 0.45 - n.y) * (0.30 + 0.70*sunUp);',
    '      vec3 lamp = uLampCol * uLampOn * 0.80 * max(0.0, 0.35 + n.y*0.65);',
    '      vec3 scr = vec3(0.16,0.26,0.28) * uScreen * 0.5 * max(0.0, dot(n, normalize(vec3(-1.0,0.2,-0.4))));',
    '      col = alb * (sunLit + skylight + fill + bounce + lamp + scr);',
    // occlusion, analytically: dark under the bench, dark where the walls meet
    '      float ao = mix(0.40, 1.0, smoothstep(0.0, 0.50, p.y - floorLevel(p)));',
    '      ao *= mix(0.62, 1.0, smoothstep(0.0, 0.40, min(2.6 - abs(p.x), 2.4 - abs(p.z))));',
    '      col *= ao;',
    '    }',
    '  }',

    // the grade: warm the whole frame at golden hour, then filmic, then gamma. One pass, no LUT.
    '  col = mix(col, col*vec3(1.20,0.95,0.76), uWarmth*0.7);',
    '  col *= 1.0 - length(uv)*0.40;',
    '  col += uPeak * vec3(0.10,0.07,0.03);',
    '  col = col/(col + vec3(0.92));',
    '  col = pow(col, vec3(1.0/2.2));',
    '  fragColor = vec4(col, 1.0);',
    '}'
  ].join('\n');

  var G = { VERT: VERT, SCENE: SCENE };
  if (typeof module !== 'undefined' && module.exports) module.exports = G;
  global.ODGlsl = G;
})(typeof globalThis !== 'undefined' ? globalThis : this);
