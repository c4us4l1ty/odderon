/* A CPU renderer for the room, used to *look* at the shader before it reaches a GPU.
 *
 *   node test/scene-probe.mjs out.png
 *
 * This is a hand port of the geometry functions in game/render/glsl.js (shell, stepAt, props, trace,
 * nrm) plus the lighting terms in main(). It exists because there is no GPU in the environment where
 * this is written, and "it compiles" is not the same as "the bed is in the frame".
 *
 * It is deliberately not a general GLSL interpreter: the vector helpers below are the whole language.
 * When the shader's geometry changes, this port changes with it -- and the unit test that checks
 * composition invariants (`test/scene.test.mjs`) is what notices if they drift apart.
 */
import { writeFileSync } from 'fs';
import { deflateSync } from 'zlib';

/* ---------------------------------- vector helpers ------------------------------------------ */

export const v3_ = (x, y, z) => [x, y, z];
const v3 = v3_;
export const add = (a, b) => [a[0] + b[0], a[1] + b[1], a[2] + b[2]];
const sub = (a, b) => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
export const mul = (a, s) => [a[0] * s, a[1] * s, a[2] * s];
const mulv = (a, b) => [a[0] * b[0], a[1] * b[1], a[2] * b[2]];
const len = (a) => Math.hypot(a[0], a[1], a[2]);
const dot = (a, b) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
export const norm = (a) => { const l = len(a) || 1; return [a[0] / l, a[1] / l, a[2] / l]; };
const clamp = (x, a, b) => (x < a ? a : x > b ? b : x);
const mix = (a, b, t) => (Array.isArray(a) ? a.map((v, i) => v + (b[i] - v) * t) : a + (b - a) * t);
const smoothstep = (e0, e1, x) => { const t = clamp((x - e0) / (e1 - e0), 0, 1); return t * t * (3 - 2 * t); };
const length2 = (x, y) => Math.hypot(x, y);

const sdBox = (p, b) => {
  const q = [Math.abs(p[0]) - b[0], Math.abs(p[1]) - b[1], Math.abs(p[2]) - b[2]];
  return Math.hypot(Math.max(q[0], 0), Math.max(q[1], 0), Math.max(q[2], 0)) +
    Math.min(Math.max(q[0], Math.max(q[1], q[2])), 0);
};
const sdCyl = (p, r, h) => {
  const dxz = length2(p[0], p[2]) - r, dy = Math.abs(p[1]) - h;
  return Math.min(Math.max(dxz, dy), 0) + Math.hypot(Math.max(dxz, 0), Math.max(dy, 0));
};

/* ---------------------------------- the room, as shipped ----------------------------------- */
export const ROOM = {
  PLAT_Z: 0.10, PLAT_H: 0.64, TREAD_W: 0.30, TREAD_H: 0.16,
  WX: 0.0, WY1: 1.28, WY2: 1.84, WZ: 2.28, DX: 1.62, DZ: -2.30,
  HX: 2.35, HZ: 2.30, CEIL: 2.45
};

export function stepAt(p) {
  const R = ROOM;
  if (p[2] > R.PLAT_Z && Math.abs(p[0]) < 1.55) return R.PLAT_H;
  const dz = R.PLAT_Z - p[2];
  if (Math.abs(p[0]) < 1.55 && dz < 4 * R.TREAD_W) return R.PLAT_H - (Math.floor(dz / R.TREAD_W) + 1) * R.TREAD_H;
  const dx = -1.55 - p[0];
  if (p[0] < -1.55 && dx < 4 * R.TREAD_W) return R.PLAT_H - (Math.floor(dx / R.TREAD_W) + 1) * R.TREAD_H;
  return 0;
}

export function shell(p) {
  const R = ROOM;
  return Math.min(R.HX - Math.abs(p[0]), R.HZ - Math.abs(p[2]), R.CEIL - p[1], p[1] - stepAt(p));
}

export function props(p) {
  const R = ROOM, H = R.PLAT_H;
  let best = [1e9, 0];
  let d;
  d = sdBox(sub(p, [0, H + 0.13, 1.15]), [1.30, 0.13, 0.86]); best = [d, 9];
  d = sdBox(sub(p, [0, H + 0.02, 1.15]), [1.16, 0.05, 0.76]); if (d < best[0]) best = [d, 1];
  d = sdBox(sub(p, [0, H + 0.12, 2.05]), [1.22, 0.30, 0.05]); if (d < best[0]) best = [d, 2];
  d = Math.min(sdBox(sub(p, [-0.64, H + 0.28, 1.78]), [0.36, 0.075, 0.17]),
               sdBox(sub(p, [0.64, H + 0.28, 1.78]), [0.36, 0.075, 0.17]));
  if (d < best[0]) best = [d, 10];
  d = sdBox(sub(p, [-1.92, 0.28, 1.62]), [0.40, 0.28, 0.40]); if (d < best[0]) best = [d, 2];
  d = sdBox(sub(p, [1.70, 0.20, -0.26]), [0.24, 0.20, 0.24]); if (d < best[0]) best = [d, 6];
  d = Math.min(sdCyl([p[0] - 2.22, p[1] - 0.30, p[2] + 0.30], 0.055, 1.55),
               sdCyl([p[0] - 2.22, p[1] - 1.85, p[2] + 0.30], 0.055, 0.55));
  if (d < best[0]) best = [d, 7];
  d = sdBox(sub(p, [-1.20, 1.28, -2.26]), [0.58, 0.34, 0.03]); if (d < best[0]) best = [d, 8];
  d = sdBox(sub(p, [2.02, 0.74, 0.10]), [0.32, 0.03, 0.72]); if (d < best[0]) best = [d, 5];
  d = sdBox(sub(p, [1.72, 0.37, 0.10]), [0.022, 0.37, 0.68]); if (d < best[0]) best = [d, 5];
  d = sdBox(sub(p, [-1.05, 0.44, -1.16]), [0.50, 0.44, 0.012]); if (d < best[0]) best = [d, 11];
  return best;
}

export const inWindow = (p) => p[2] > 2.18 &&
  (Math.abs(p[1] - ROOM.WY1) < 0.24 || Math.abs(p[1] - ROOM.WY2) < 0.24) && Math.abs(p[0] - ROOM.WX) < 1.85;
export const inDoor = (p) => p[2] < -2.20 && Math.abs(p[0] - ROOM.DX) < 0.52 && p[1] < 2.04;

export function trace(ro, rd) {
  let t = 0.02;
  for (let i = 0; i < 64; i++) {
    const h = props(add(ro, mul(rd, t)));
    if (h[0] < 0.0016) return [t, h[1]];
    t += Math.max(h[0] * 0.9, 0.008);
    if (t > 8) break;
  }
  t = 0.02;
  for (let i = 0; i < 48; i++) {
    const p = add(ro, mul(rd, t));
    const d = shell(p);
    if (d < 0.004) {
      if (inWindow(p)) return [t, 4];
      if (inDoor(p)) return [t, 3];
      return [t, 0];
    }
    t += Math.max(d * 0.95, 0.02);
    if (t > 8) break;
  }
  return [1e9, 0];
}

export function nShell(p) {
  const R = ROOM, fy = stepAt(p);
  if (p[1] - fy < 0.05) return [0, 1, 0];
  if (p[2] < R.PLAT_Z && p[1] - fy > 0.06) {
    if (Math.abs(p[0]) < 1.55) return [0, 0, -1];
    if (p[0] < -1.55) return [1, 0, 0];
  }
  const ex = R.HX - Math.abs(p[0]), ey = R.CEIL - p[1], ez = R.HZ - Math.abs(p[2]);
  if (ex < ez && ex < ey) return [Math.sign(p[0]), 0, 0];
  if (ez < ey) return [0, 0, Math.sign(p[2])];
  return [0, -1, 0];
}

export function nProps(p) {
  const h = 0.0022;
  return norm([
    props(add(p, [h, 0, 0]))[0] - props(sub(p, [h, 0, 0]))[0],
    props(add(p, [0, h, 0]))[0] - props(sub(p, [0, h, 0]))[0],
    props(add(p, [0, 0, h]))[0] - props(sub(p, [0, 0, h]))[0] + 1e-6
  ]);
}

const ns = (a, b, x) => smoothstep(a, b, x);
export function inGlazing(q) {
  const R = ROOM;
  const band = Math.max(ns(0.24, 0.18, Math.abs(q[1] - R.WY1)), ns(0.24, 0.18, Math.abs(q[1] - R.WY2)));
  return band * ns(1.85, 1.80, Math.abs(q[0] - R.WX));
}
export function sunVisible(p, sunDir) {
  const R = ROOM;
  if (sunDir[2] <= 0.02) return 0;
  const tz = (R.WZ - p[2]) / sunDir[2];
  if (tz <= 0) return 0;
  const q = add(p, mul(sunDir, tz));
  const tx = ((sunDir[0] > 0 ? R.HX : -R.HX) - p[0]) / (Math.abs(sunDir[0]) < 0.02 ? 0.02 : sunDir[0]);
  const tc = ((sunDir[1] > 0 ? R.CEIL : 0) - p[1]) / (Math.abs(sunDir[1]) < 0.02 ? 0.02 : sunDir[1]);
  if (tx < tz || tc < tz) return 0;
  return inGlazing(q);
}
export function windowPattern(p, sunDir) {
  const R = ROOM;
  if (Math.abs(sunDir[2]) < 0.02) return 1;
  const t = (R.WZ - p[2]) / sunDir[2];
  const q = add(p, mul(sunDir, t));
  const band = Math.max(ns(0.26, 0.20, Math.abs(q[1] - R.WY1)), ns(0.26, 0.20, Math.abs(q[1] - R.WY2)));
  let bar = 1;
  for (let i = 0; i < 3; i++) bar *= ns(0, 0.045, Math.abs(q[0] - (R.WX + (i - 1) * 1.15)));
  return band * ns(1.85, 1.78, Math.abs(q[0] - R.WX)) * bar;
}
export function stripLight(p, n) {
  const R = ROOM, fy = stepAt(p);
  let lit = 0;
  lit += ns(0.075, 0.02, p[1] - fy) * 1.6;
  lit += ns(0.06, 0.0, Math.abs(p[1] - (R.PLAT_H - 0.09))) * ns(0.0, 0.5, 1.55 - Math.abs(p[0])) * 0.9;
  lit += ns(0.42, 0.06, R.HX - Math.abs(p[0])) * ns(0.0, 0.25, R.CEIL - p[1]) * 1.2;
  return lit * Math.max(0, 0.35 + 0.65 * n[1]);
}

function albedo(p, n, mat) {
  const R = ROOM, fy = stepAt(p);
  if (mat < 0.5) {
    if (p[1] - fy < 0.05) {
      if (fy > 0.3) {
        const seam = smoothstep(0.03, 0, Math.abs((p[0] * 2.4 % 1) - 0.5) - 0.47);
        return mulv(mix([0.30, 0.20, 0.12], [0.38, 0.26, 0.16], 0.4 + 0.6 * 0.5), [1 - 0.25 * seam, 1 - 0.25 * seam, 1 - 0.25 * seam]);
      }
      return mix([0.66, 0.63, 0.57], [0.76, 0.73, 0.67], 0.5);
    }
    if (p[1] - fy < 0.075 && fy > 0.02) return [0.42, 0.42, 0.40];
    let c = [0.66, 0.65, 0.62];
    if (p[1] - fy < 0.14) c = [0.16, 0.16, 0.17];
    if (n[1] > 0.5) {
      const cove = smoothstep(0.55, 0.20, R.HX - Math.abs(p[0]));
      c = mix([0.18, 0.18, 0.19], [1.0, 0.97, 0.90], cove);
    }
    return c;
  }
  if (mat < 1.5) return [0.12, 0.12, 0.13];
  if (mat < 2.5) return [0.05, 0.05, 0.06];
  if (mat < 3.5) return [0.09, 0.09, 0.10];
  if (mat < 4.5) return [1, 1, 1];
  if (mat < 5.5) return [0.55, 0.55, 0.53];
  if (mat < 6.5) return [0.62, 0.62, 0.60];
  if (mat < 7.5) return [0.30, 0.24, 0.14];
  if (mat < 8.5) return [0.03, 0.04, 0.05];
  if (mat < 9.5) return [0.80, 0.82, 0.79];
  if (mat < 10.5) return [0.88, 0.88, 0.85];
  return [0.55, 0.60, 0.62];
}

/* ---------------------------------- the frame ---------------------------------------------- */
export const ENV = {
  sunDir: [-0.020, 0.530, 0.848],
  sunCol: [1.0, 1.05, 1.05],
  horizon: [0.83, 0.72, 0.67],
  zenith: [0.14, 0.38, 0.74],
  day: 1, curtain: 1, warmth: 0.48, sunElev: 16.9, lampOn: 0.25, screen: 0.5
};

export function shade(ro, rd, env) {
  const [t, mat] = trace(ro, rd);
  if (t > 1e8) {
    // the sky outside
    const h = clamp(rd[1] * 0.5 + 0.5, 0, 1);
    let c = mix(env.horizon, env.zenith, Math.pow(h, 0.62));
    const sun = Math.max(dot(rd, env.sunDir), 0);
    c = add(c, mul(env.sunCol, Math.pow(sun, 300) * 5));
    c = add(c, mul(env.sunCol, Math.pow(sun, 8) * 0.18));
    const hz = smoothstep(0.07, -0.03, rd[1]);
    c = mix(c, mul([0.28, 0.32, 0.27], 1.15), hz * 0.88);
    return c;
  }
  const p = add(ro, mul(rd, t));
  if (mat === 4) {
    // the window: two bands of sky with a dark frame
    const h = clamp(rd[1] * 0.5 + 0.5, 0, 1);
    let out_ = mix(env.horizon, env.zenith, Math.pow(h, 0.62));
    out_ = add(out_, mul(env.sunCol, Math.pow(Math.max(dot(norm([rd[0] * 1.15, rd[1] * 0.92 - 0.03, 0.72]), env.sunDir), 0), 300) * 5));
    const band = Math.max(ns(0.26, 0.20, Math.abs(p[1] - ROOM.WY1)), ns(0.26, 0.20, Math.abs(p[1] - ROOM.WY2)));
    let bar = 1;
    for (let i = 0; i < 3; i++) bar *= ns(0, 0.035, Math.abs(p[0] - (ROOM.WX + (i - 1) * 1.15)));
    const frameCol = [0.21, 0.22, 0.24];
    return mix(frameCol, add(mul(out_, 1.5), [0.01, 0.01, 0.01]), band * bar);
  }
  if (mat === 3) return [0.22, 0.21, 0.18];
  if (mat === 8) return [0.12, 0.19, 0.21];
  const n = mat > 0.5 ? nProps(p) : nShell(p);
  const alb = albedo(p, n, mat);
  const sunUp = smoothstep(-0.04, 0.16, env.sunElev);
  const dif = Math.max(dot(n, env.sunDir), 0);
  const vis = sunVisible(p, env.sunDir) * smoothstep(0.10, 0.80, env.curtain) * sunUp;
  const pat = windowPattern(p, env.sunDir);
  const shade_ = mix(1, 0.25 + 0.75 * pat, smoothstep(0, 0.35, vis));
  let col = mul(env.sunCol, dif * vis * shade_ * 4.4);
  const toWin = sub([ROOM.WX, (ROOM.WY1 + ROOM.WY2) * 0.5, ROOM.WZ], p);
  const d2 = dot(toWin, toWin);
  const tw = mul(toWin, 1 / Math.sqrt(d2 + 1e-4));
  const ap = Math.max(dot(n, tw), 0) * Math.max(tw[2], 0);
  const sky = mul(mix(env.horizon, env.zenith, 0.22), ap * (11 / (1 + d2 * 0.55)) * smoothstep(0.08, 0.9, env.curtain) * (0.25 + 0.75 * env.day));
  const builtIn = mul([1.0, 0.90, 0.74], stripLight(p, n) * 1.35);
  const fill = mul(mix(env.horizon, env.zenith, 0.35), 0.34 * (0.55 + 0.45 * n[1]));
  const bounce = mul([0.40, 0.30, 0.20], 0.34 * Math.max(0, 0.45 - n[1]) * (0.30 + 0.70 * sunUp));
  const lamp = mul([1.0, 0.82, 0.60], env.lampOn * 0.45 * Math.max(0, 0.35 + n[1] * 0.65));
  const scr = mul([0.16, 0.26, 0.28], env.screen * 0.35 * Math.max(0, dot(n, norm([-1, 0.2, -0.4]))));
  col = add(add(add(add(add(col, sky), builtIn), fill), bounce), add(lamp, scr));
  col = mulv(alb, col);
  let ao = mix(0.38, 1, smoothstep(0, 0.30, p[1] - stepAt(p)));
  ao *= mix(0.62, 1, smoothstep(0, 0.45, Math.min(ROOM.HX - Math.abs(p[0]), ROOM.HZ - Math.abs(p[2]))));
  return mul(col, ao);
}

export function camera(pos, yaw, pitch, fov = 1.05) {
  const cp = Math.cos(pitch), sp = Math.sin(pitch);
  const fwd = [Math.sin(yaw) * cp, sp, Math.cos(yaw) * cp];
  const right = [Math.cos(yaw), 0, -Math.sin(yaw)];
  const up = norm([-(right[1] * fwd[2] - right[2] * fwd[1]),
                   -(right[2] * fwd[0] - right[0] * fwd[2]),
                   -(right[0] * fwd[1] - right[1] * fwd[0])]);
  return { pos, fwd, right, up, fov };
}

export function render(cam, W, H, env = ENV) {
  const img = new Uint8Array(W * H * 3);
  for (let y = 0; y < H; y++) {
    for (let x = 0; x < W; x++) {
      const ux = (x + 0.5 - W / 2) / H;
      const uy = (H / 2 - (y + 0.5)) / H;
      const rd = norm(add(add(cam.fwd, mul(cam.right, ux * cam.fov)), mul(cam.up, uy * cam.fov)));
      let c = shade(cam.pos, rd, env);
      // the same grade as the shader: warm, vignette, filmic, gamma
      c = mix(c, mulv(c, [1.20, 0.95, 0.76]), env.warmth * 0.7);
      const vig = 1 - Math.hypot(ux * 1.2, uy * 1.2) * 0.40;
      c = mul(c, vig);
      c = [c[0] / (c[0] + 0.92), c[1] / (c[1] + 0.92), c[2] / (c[2] + 0.92)];
      const i = (y * W + x) * 3;
      img[i] = Math.round(Math.pow(clamp(c[0], 0, 1), 1 / 2.2) * 255);
      img[i + 1] = Math.round(Math.pow(clamp(c[1], 0, 1), 1 / 2.2) * 255);
      img[i + 2] = Math.round(Math.pow(clamp(c[2], 0, 1), 1 / 2.2) * 255);
    }
  }
  return img;
}

/* ---------------------------------- a PNG, by hand ----------------------------------------- */
function crc32(buf) {
  let c, crc = 0xffffffff;
  for (let n = 0; n < buf.length; n++) {
    c = (crc ^ buf[n]) & 0xff;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    crc = (crc >>> 8) ^ c;
  }
  return (crc ^ 0xffffffff) >>> 0;
}
function chunk(type, data) {
  const len = Buffer.alloc(4); len.writeUInt32BE(data.length);
  const td = Buffer.concat([Buffer.from(type, 'ascii'), data]);
  const crc = Buffer.alloc(4); crc.writeUInt32BE(crc32(td));
  return Buffer.concat([len, td, crc]);
}
export function png(img, W, H) {
  const raw = Buffer.alloc((W * 3 + 1) * H);
  for (let y = 0; y < H; y++) {
    raw[y * (W * 3 + 1)] = 0;
    Buffer.from(img.buffer, y * W * 3, W * 3).copy(raw, y * (W * 3 + 1) + 1);
  }
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(W, 0); ihdr.writeUInt32BE(H, 4);
  ihdr[8] = 8; ihdr[9] = 2; ihdr[10] = 0; ihdr[11] = 0; ihdr[12] = 0;
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk('IHDR', ihdr),
    chunk('IDAT', deflateSync(raw, { level: 9 })),
    chunk('IEND', Buffer.alloc(0))
  ]);
}

if (process.argv[1] && process.argv[1].endsWith('scene-probe.mjs')) {
  const out = process.argv[2] || '/tmp/room.png';
  const yaw = process.argv[3] ? +process.argv[3] : 1.30;
  const pitch = process.argv[4] ? +process.argv[4] : -0.06;
  const px = process.argv[5] ? +process.argv[5] : -1.95;
  const pz = process.argv[6] ? +process.argv[6] : -1.35;
  const W = 480, H = 270;
  const cam = camera([px, 1.62, pz], yaw, pitch);
  const t0 = Date.now();
  const img = render(cam, W, H);
  writeFileSync(out, png(img, W, H));
  console.log('wrote ' + out + ' in ' + (Date.now() - t0) + ' ms');
}
