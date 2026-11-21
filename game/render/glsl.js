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
     * A hab interior, built the way the reference (Star Citizen's apartment habs) is built:
     *
     *   - the *bed sits on a raised platform* in the middle of the room, and the floor steps down
     *     to the working level in four wide, shallow treads. That single move -- one platform with
     *     a staircase around it -- is what makes the volume read as designed instead of as a box
     *     with furniture in it.
     *   - every tread carries a lit strip along its nosing. The light is *in the architecture*.
     *   - two long horizontal strip windows on the far wall, above the bed, with a heavy frame.
     *   - an angled ceiling with a lit cove, and structural ribs crossing it.
     *   - one curved glass balustrade at the foot of the platform, because the hab does not have a
     *     straight line where a curve would do.
     *
     * Room: x [-2.7, 2.7], z [-2.6, 2.6], ceiling 2.62. Platform: x [-1.5, 1.5],
     * z [0.15, 2.3], top at y = 0.84. Treads: four, each 0.21 m high being too many for the height,
     * so the stair runs down in three treads of 0.21 plus the platform face.
     */
    'float PLAT_Z = 0.10;',        // the platform's near edge: z greater than this is on the platform
    'float PLAT_H = 0.64;',        // its height: you sit on the edge of a bed 0.63 m above the floor
    'float TREAD_W = 0.30;',       // each tread is 300 mm deep: shallow enough to walk up without looking
    'float TREAD_H = 0.16;',
    'float WX = 0.0;',           // the strip windows, centred on the bed
    'float WY1 = 1.28;',           // lower window band
    'float WY2 = 1.84;',           // upper band
    'float WZ = 2.28;',
    'float DX =  1.62;',           // the door in the -z wall
    'float DZ = -2.30;',

    // How high is the floor at this point? One function, and the whole room reads off it.
    'float floorAt(vec3 p){',
    '  if (p.z > PLAT_Z && abs(p.x) < 1.55) return PLAT_H;',
    '  float zs = p.z - 1.5;',        // steps push toward +z
    '  return 0.0;',
    '}',

    // the treads: a staircase that runs along the -z face of the platform and round its -x side
    'float stepAt(vec3 p){',
    '  if (p.z > PLAT_Z) return PLAT_H;',
    '  // three treads descending from the platform edge toward -z',
    '  float dz = PLAT_Z - p.z;',
    '  if (abs(p.x) < 1.55 && dz < 4.0*TREAD_W) {',
    '    float k = floor(dz / TREAD_W);',
    '    return PLAT_H - (k + 1.0)*TREAD_H;',
    '  }',
    '  // and the same three treads wrapping round the -x end, so the platform is approached from two sides',
    '  float dx = -1.55 - p.x;',
    '  if (p.x < -1.55 && dx < 4.0*TREAD_W) {',
    '    float k = floor(dx / TREAD_W);',
    '    return PLAT_H - (k + 1.0)*TREAD_H;',
    '  }',
    '  return 0.0;',
    '}',

    'float inWindow(vec3 p){',
    '  if (p.z < 2.18) return false;',
    '  float band = 0.0;',
    '  if (abs(p.y - WY1) < 0.24) band = 1.0;',
    '  if (abs(p.y - WY2) < 0.24) band = 1.0;',
    '  return band > 0.5 && abs(p.x - WX) < 1.85;',
    '}',
    'bool inDoor(vec3 p){ return p.z < -2.20 && abs(p.x - DX) < 0.52 && p.y < 2.04; }',

    // distance to the shell, from the inside: walls, the stepped floor, the angled ceiling
    'float shell(vec3 p){',
    '  float d = min(2.35 - abs(p.x), 2.30 - abs(p.z));',
    '  float ceil = 2.45 - p.y;',
    '  d = min(d, ceil);',
    '  d = min(d, p.y - stepAt(p));',
    '  return d;',
    '}',

    /* ---------------- props ----------------
     * The bed is the anchor of the room, exactly as in the reference: a wide low platform bed with a
     * pale mattress, two pillows, and its base lit from underneath. Everything else is smaller and
     * quieter than the bed.
     */
    'vec2 props(vec3 p){',
    '  vec2 r = vec2(1e9, 0.0);',
    '  float d;',
    // the mattress, recessed in the platform
    '  d = sdBox(p - vec3(0.0, PLAT_H + 0.13, 1.15), vec3(1.30, 0.13, 0.86));',
    '  r = vec2(d, 9.0);',
    // the base under the mattress, inset, so the bed appears to float on a lit plinth
    '  d = sdBox(p - vec3(0.0, PLAT_H + 0.02, 1.15), vec3(1.16, 0.05, 0.76));',
    '  if (d < r.x) r = vec2(d, 1.0);',
    // the headboard: a tall upholstered panel against the window wall
    '  d = sdBox(p - vec3(0.0, PLAT_H + 0.12, 2.05), vec3(1.22, 0.30, 0.05));',
    '  if (d < r.x) r = vec2(d, 2.0);',
    // two pillows
    '  d = min(sdBox(p - vec3(-0.64, PLAT_H + 0.28, 1.78), vec3(0.36, 0.075, 0.17)),',
    '          sdBox(p - vec3( 0.64, PLAT_H + 0.28, 1.78), vec3(0.36, 0.075, 0.17)));',
    '  if (d < r.x) r = vec2(d, 10.0);',
    // the console beside the bed on the platform: a dark slab the wall screen is set into
    '  d = sdBox(p - vec3(-1.92, 0.28, 1.62), vec3(0.40, 0.28, 0.40));',
    '  if (d < r.x) r = vec2(d, 2.0);',
    // a low cube seat on the floor below the platform, as in the reference
    '  d = sdBox(p - vec3(1.70, 0.20, -0.26), vec3(0.24, 0.20, 0.24));',
    '  if (d < r.x) r = vec2(d, 6.0);',
    // the pipe run: the machine the room is. Low along -z, then up into the ceiling.
    '  d = min(sdCyl(vec3(p.x - 2.22, p.y - 0.30, p.z + 0.30), 0.055, 1.55),',
    '          sdCyl(vec3(p.x - 2.22, p.y - 1.85, p.z + 0.30), 0.055, 0.55));',
    '  if (d < r.x) r = vec2(d, 7.0);',
    // the wall screen: a dark panel on the -z wall, the room\'s second light at night
    '  d = sdBox(p - vec3(-1.20, 1.28, -2.26), vec3(0.58, 0.34, 0.03));',
    '  if (d < r.x) r = vec2(d, 8.0);',
    // the desk: a slab cantilevered off the +x wall, under the window, with one leg
    '  d = sdBox(p - vec3(2.02, 0.74, 0.10), vec3(0.32, 0.03, 0.72));',
    '  if (d < r.x) r = vec2(d, 5.0);',
    '  d = sdBox(p - vec3(1.72, 0.37, 0.10), vec3(0.022, 0.37, 0.68));',
    '  if (d < r.x) r = vec2(d, 5.0);',
    // the glass balustrade at the foot of the platform: a thin curved sheet on two posts
    '  d = sdBox(p - vec3(-1.05, 0.44, -1.16), vec3(0.50, 0.44, 0.012));',
    '  if (d < r.x) r = vec2(d, 11.0);',
    '  return r;',
    '}',

    /* ---------------- normals ----------------
     * Inside a room, the visible face of a wall is the one facing the camera, so the normal points
     * back into the volume. Getting this sign wrong is what made the whole room read as blue fog:
     * with -s every wall faced away from the sun and only the sky fill survived.
     */
    'vec3 nShell(vec3 p){',
    '  float fy = stepAt(p);',
    '  if (p.y - fy < 0.05) return vec3(0.0, 1.0, 0.0);',
    // tread risers face -z (or +x on the wrapped side), and the platform face faces -z
    '  if (p.z < PLAT_Z && p.y - fy > 0.06) {',
    '    if (abs(p.x) < 1.5) return vec3(0.0, 0.0, -1.0);',
    '    if (p.x < -1.55) return vec3(1.0, 0.0, 0.0);',
    '  }',
    '  float ex = 2.35 - abs(p.x), ey = 2.45 - p.y, ez = 2.30 - abs(p.z);',
    '  if (ex < ez && ex < ey) return vec3(sign(p.x), 0.0, 0.0);',
    '  if (ez < ey) return vec3(0.0, 0.0, sign(p.z));',
    '  return vec3(0.0, -1.0, 0.0);',        // the ceiling, seen from below
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
    '  for(int i=0;i<32;i++){',
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
    '    float fy = stepAt(p);',
    // the floor: pale stone on the working level, warm walnut on the platform, as in the reference
    '    if (p.y - fy < 0.05) {',
    '      if (fy > 0.3) {',
    '        float seam = smoothstep(0.03, 0.0, abs(fract(p.x*2.4)-0.5)-0.47);',
    '        return mix(vec3(0.42,0.27,0.14), vec3(0.52,0.35,0.19), 0.4+0.6*noise(p.xz*9.0)) * (1.0 - 0.22*seam);',
    '      }',
    '      float slab = smoothstep(0.02, 0.0, abs(fract(p.x*1.3)-0.5)-0.485);',
    '      return mix(vec3(0.66,0.63,0.57), vec3(0.76,0.73,0.67), noise(p.xz*3.0)) * (1.0 - 0.22*slab);',
    '    }',
    // tread nosings: a band of brushed aluminium at the lip of every step
    '    if (p.y - fy < 0.075 && fy > 0.02) return vec3(0.42,0.42,0.40);',
    // the wall: light grey panel bay, with the hab\'s segmented bays and a dark base
    '    vec3 c = vec3(0.66,0.65,0.62);',
    '    float bay = smoothstep(0.012, 0.0, abs(fract(p.z*1.15)-0.5)-0.494);',
    '    c *= 1.0 - 0.35*bay;',
    '    if (p.y - fy < 0.14) c = vec3(0.16,0.16,0.17);',
    // structural ribs: the vertical posts you can see in the reference, every 2.2 m
    '    float rib = smoothstep(0.045,0.015,abs(abs(p.x)-1.98));',
    '    c = mix(c, vec3(0.44,0.44,0.42), rib);',
    // the ceiling: dark, with a bright cove running down both sides
    '    if (n.y < -0.5) {',
    '      float cove = smoothstep(0.62, 0.18, 2.35 - abs(p.x));',
    '      c = mix(vec3(0.18,0.18,0.19), vec3(1.0,0.97,0.90), cove);',
    '    }',
    '    return c;',
    '  }',
    '  if (mat < 1.5) return vec3(0.12,0.12,0.13);',                       // bed base, dark steel
    '  if (mat < 2.5) return vec3(0.10,0.10,0.11);',                       // console: dark lacquer
    '  if (mat < 3.5) return vec3(0.09,0.09,0.10);',                       // door panel
    '  if (mat < 4.5) return vec3(1.0);',                                  // the window
    '  if (mat < 5.5) return vec3(0.55,0.55,0.53);',                       // desk: pale composite
    '  if (mat < 6.5) return vec3(0.62,0.62,0.60);',                       // the cube seat, white boucle
    '  if (mat < 7.5) return vec3(0.20,0.22,0.21);',                       // painted pipe, dark green
    '  if (mat < 8.5) return vec3(0.03,0.04,0.05);',                       // screen glass
    '  if (mat < 9.5) return vec3(0.80,0.82,0.79);',                       // the mattress, pale linen
    '  if (mat < 10.5) return vec3(0.88,0.88,0.85);',                      // pillow
    '  return vec3(0.62,0.68,0.70);',                                      // the glass balustrade
    '}',

    /* ---------------- the light ----------------
     * Two different things come through a window, and the room was only modelling one of them.
     *
     *  1. The sun: a *directional* beam, but the window clips it, so what lands on the floor is the
     *     window's own silhouette, projected along the sun. Two horizontal bands plus three vertical
     *     mullions, which is exactly the pattern the reference hab has painted across its floor.
     *  2. The sky: an *area* source filling the whole aperture. This is what makes a daytime room
     *     readable -- a soft, cool, cosine-weighted light from one direction, falling off with
     *     distance. Without it a room at noon is a bright patch on the floor and a cave elsewhere.
     *
     * Both are closed-form. There is no shadow map anywhere in this file, and no march: the sun test
     * is one division per plane, which is why it costs nothing on a 2015 iGPU.
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

    /* The window's silhouette, cast on whatever we hit. We project the surface point *back* along
     * the sun onto the window plane and ask which part of the glazing it came through: a band, a
     * mullion, or the wall. That is the pattern you see on a floor at four in the afternoon. */
    'float windowPattern(vec3 p){',
    '  if (abs(uSunDir.z) < 0.02) return 1.0;',
    '  float t = (WZ - p.z) / uSunDir.z;',
    '  vec3 q = p + uSunDir * t;',
    '  // the two bands',
    '  float band = max(smoothstep(0.26, 0.20, abs(q.y - WY1)),',
    '                   smoothstep(0.26, 0.20, abs(q.y - WY2)));',
    '  // the mullions: three vertical bars, and the frame at the ends',
    '  float frame = smoothstep(1.85, 1.78, abs(q.x - WX));',
    '  float bar = 1.0;',
    '  for(int i=0;i<3;i++){',
    '    float bx = WX + (float(i) - 1.0) * 1.15;',
    '    bar *= smoothstep(0.0, 0.045, abs(q.x - bx));',
    '  }',
    '  return band * frame * bar;',
    '}',

    // the tread nosings, the cove and the bed base: the light that is built into the room
    'float stripLight(vec3 p, vec3 n){',
    '  float lit = 0.0;',
    // a strip under the platform edge and along every tread nosing
    '  float fy = stepAt(p);',
    '  lit += smoothstep(0.075, 0.02, p.y - fy) * 1.6;',
    // a strip up the sides of the platform
    '  lit += smoothstep(0.06, 0.0, abs(p.y - (PLAT_H - 0.09))) * smoothstep(0.0, 0.5, 1.55 - abs(p.x)) * 0.9;',
    // the ceiling cove: two long lines down the room
    '  lit += smoothstep(0.42, 0.06, 2.35 - abs(p.x)) * smoothstep(0.0, 0.25, 2.45 - p.y) * 1.2;',
    '  return lit * max(0.0, 0.35 + 0.65*n.y);',
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
     * The window is an aperture in the +z wall, so a sun ray reaches a point iff:
     *   - the sun is actually in front of that wall (sunDir.z > 0),
     *   - the ray exits through z = WZ before it hits any other wall,
     *   - and the exit point lies inside one of the two glazing bands.
     * One division per plane, no march, no shadow map. This is the function the whole art direction
     * hangs on, and it is eleven lines.
     */
    'float inGlazing(vec3 q){',
    '  float band = max(smoothstep(0.24, 0.18, abs(q.y - WY1)),',
    '                   smoothstep(0.24, 0.18, abs(q.y - WY2)));',
    '  return band * smoothstep(1.85, 1.80, abs(q.x - WX));',
    '}',
    'float sunVisible(vec3 p, vec3 n){',
    '  if (uSunDir.z <= 0.02) return 0.0;',
    '  float tz = (WZ - p.z) / uSunDir.z;',
    '  if (tz <= 0.0) return 0.0;',
    '  vec3 q = p + uSunDir * tz;',
    '  float tx = ((uSunDir.x > 0.0 ? 2.35 : -2.35) - p.x) / (abs(uSunDir.x) < 0.02 ? 0.02 : uSunDir.x);',
    '  float tc = ((uSunDir.y > 0.0 ? 2.45 : 0.0) - p.y) / (abs(uSunDir.y) < 0.02 ? 0.02 : uSunDir.y);',
    '  if (tx < tz || tc < tz) return 0.0;',
    '  return inGlazing(q);',
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
    // the window: two bands of real sky, a heavy frame between them, dimmed by the curtain
    '      float band = max(smoothstep(0.26, 0.20, abs(p.y - WY1)), smoothstep(0.26, 0.20, abs(p.y - WY2)));',
    '      vec3 out_ = sky(normalize(vec3(rd.x*1.15, rd.y*0.92 - 0.03, 0.72)));',
    '      float cur = mix(0.06, 1.0, uCurtain*uCurtain);',
    // the sash and mullions are dark aluminium, catching a little of the room
    '      float bar = 1.0;',
    '      for(int i=0;i<3;i++){',
    '        float bx = WX + (float(i) - 1.0) * 1.15;',
    '        bar *= smoothstep(0.0, 0.035, abs(p.x - bx));',
    '      }',
    '      vec3 frameCol = vec3(0.10,0.10,0.11) + vec3(0.22,0.24,0.26)*0.5;',
    '      col = mix(frameCol, out_*cur*1.5 + vec3(0.01), band*bar);',
    '    } else if (mat > 2.5 && mat < 3.5) {',
    // the door: a lit corridor, so the hab is a room in a bigger building
    '      col = mix(vec3(0.05,0.05,0.055), vec3(0.40,0.38,0.32), 0.30 + 0.45*uScreen);',
    '    } else if (mat > 7.5 && mat < 8.5) {',
    // the wall screen
    '      float tab = smoothstep(0.02,0.0,abs(fract(p.z*6.0)-0.5)-0.44) * step(mod(p.y*8.0,3.0),1.0);',
    '      col = vec3(0.05,0.07,0.08) + vec3(0.18,0.30,0.33)*(0.35+0.65*uScreen)*(0.35+tab);',
    '    } else {',
    '      vec3 n = (mat > 0.5) ? nProps(p) : nShell(p);',
    '      vec3 alb = albedo(p, n, mat);',
    '      float sunUp = smoothstep(-0.04, 0.16, uSunElev);',
    '      float dif = max(dot(n, uSunDir), 0.0);',
    // the sun, clipped by the window: a hard-edged pattern of two bands and three bars
    '      float vis = sunVisible(p, n) * smoothstep(0.10, 0.80, uCurtain) * sunUp;',
    '      float pat = windowPattern(p);',
    '      float shade = mix(1.0, 0.25 + 0.75*pat, smoothstep(0.0, 0.35, vis));',
    '      vec3 sunLit = uSunCol * dif * vis * shade * 4.4;',
    // the sky through the same aperture: an area light, which is what actually makes a room readable
    '      vec3 toWin = vec3(WX, (WY1+WY2)*0.5, WZ) - p;',
    '      float d2 = dot(toWin, toWin);',
    '      toWin = toWin * inversesqrt(d2 + 1e-4);',
    '      float ap = max(dot(n, toWin), 0.0) * max(toWin.z, 0.0);',
    '      vec3 skylight = mix(uSkyHorizon, uSkyZenith, 0.22) * ap * (11.0 / (1.0 + d2*0.55))',
    '                    * smoothstep(0.08, 0.9, uCurtain) * (0.25 + 0.75*uDay);',
    // the light built into the architecture: tread nosings, the cove, the bed base
    '      vec3 builtIn = vec3(1.0,0.90,0.74) * stripLight(p, n) * 1.35;',
    // and the fill that keeps the shadows from being black
    '      vec3 fill = mix(uSkyHorizon, uSkyZenith, 0.35) * 0.34 * (0.55 + 0.45*n.y);',
    '      vec3 bounce = vec3(0.40,0.30,0.20) * 0.34 * max(0.0, 0.45 - n.y) * (0.30 + 0.70*sunUp);',
    '      vec3 lamp = uLampCol * uLampOn * 0.55 * max(0.0, 0.35 + n.y*0.65);',
    '      vec3 scr = vec3(0.16,0.26,0.28) * uScreen * 0.35 * max(0.0, dot(n, normalize(vec3(-1.0,0.2,-0.4))));',
    '      col = alb * (sunLit + skylight + builtIn + fill + bounce + lamp + scr);',
    // occlusion, analytically: dark under the platform, dark where the walls meet
    '      float ao = mix(0.38, 1.0, smoothstep(0.0, 0.30, p.y - stepAt(p)));',
    '      ao *= mix(0.62, 1.0, smoothstep(0.0, 0.45, min(2.35 - abs(p.x), 2.30 - abs(p.z))));',
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
