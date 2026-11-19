/* Two-body orbital mechanics.
 *
 * All vectors are inertial (Earth-centred), in metres and metres/second. The
 * simulation is a patched two-body model: while the engine is off time advances
 * with an exact universal-variable Kepler solution, so time warp never winds up
 * numerical error. Under thrust a fixed-step RK4 integrates the force model.
 *
 * The model is deliberately honest: no ephemerides, no Moon, no gravity field
 * beyond an optional J2. The HUD says so.
 */

import { Vector3, Quaternion, Matrix4 } from "three";
import { EARTH_MU, EARTH_RADIUS, EARTH_J2, clamp } from "./math";
import { coreElements, coreKepler, coreReady, coreRk4, coreTimeToApsis } from "./core";

export interface Elements {
  /** Semi-major axis, metres. Negative for hyperbolic. */
  a: number;
  /** Eccentricity, dimensionless. */
  e: number;
  /** Inclination, radians. */
  i: number;
  /** Right ascension of the ascending node, radians. */
  raan: number;
  /** Argument of periapsis, radians. */
  argp: number;
  /** True anomaly, radians. */
  nu: number;
  /** Semi-latus rectum, metres. */
  p: number;
  /** Periapsis radius, metres. */
  pe: number;
  /** Apoapsis radius, metres (Infinity for open orbits). */
  ap: number;
  /** Orbital period, seconds (Infinity for open orbits). */
  period: number;
  /** Specific orbital energy, J/kg. */
  energy: number;
  /** Angular momentum vector. */
  h: Vector3;
  /** Eccentricity vector, points at periapsis. */
  eVec: Vector3;
  /** Perifocal unit vectors, inertial. */
  pHat: Vector3;
  qHat: Vector3;
  /** Closing (1) or open (0) orbit. */
  closed: boolean;
}

export interface State {
  r: Vector3;
  v: Vector3;
}

const EPS = 1e-9;

/** Classical elements from an inertial state vector. */
export function elementsFromState(r: Vector3, v: Vector3, mu = EARTH_MU): Elements {
  if (coreReady()) {
    const w = coreElements(r.x, r.y, r.z, v.x, v.y, v.z, mu);
    if (w) return elementsFromPacked(w);
  }
  const rm = r.length();
  const vm = v.length();
  const h = new Vector3().crossVectors(r, v);
  const hm = h.length();
  const energy = (vm * vm) / 2 - mu / rm;
  const a = Math.abs(energy) < 1e-12 ? Infinity : -mu / (2 * energy);

  const eVec = new Vector3()
    .copy(r)
    .multiplyScalar(vm * vm - mu / rm)
    .addScaledVector(v, -r.dot(v))
    .divideScalar(mu);
  const e = clamp(eVec.length(), 0, 1e6);

  let i = 0;
  if (hm > EPS) i = Math.acos(clamp(h.z / hm, -1, 1));

  // Node vector n = z_hat x h = (-h.y, h.x, 0)
  const n = new Vector3(-h.y, h.x, 0);
  const nm = n.length();

  let raan = 0;
  if (nm > EPS) raan = Math.atan2(n.y, n.x);
  // atan2(n.y,n.x) with n.x=-h.y, n.y=h.x gives the standard RAAN.

  let argp = 0;
  if (nm > EPS && e > 1e-8) {
    argp = Math.atan2(
      eVec.dot(new Vector3().crossVectors(n, h)) / h.length(),
      eVec.dot(n),
    );
  } else if (e > 1e-8) {
    // Equatorial: argument of periapsis degrades to longitude of periapsis.
    argp = Math.atan2(eVec.y, eVec.x);
  }
  argp = ((argp % (Math.PI * 2)) + Math.PI * 2) % (Math.PI * 2);

  let nu = 0;
  if (e > 1e-8) {
    const q = new Vector3().crossVectors(h, eVec).divideScalar(hm);
    nu = Math.atan2(r.dot(q) / Math.max(e * rm, EPS), r.dot(eVec) / Math.max(e * rm, EPS));
  } else if (nm > EPS) {
    // Circular inclined: measure from the node.
    const q = new Vector3().crossVectors(h, n).divideScalar(hm * nm);
    nu = Math.atan2(r.dot(q) / Math.max(rm, EPS), r.dot(n) / Math.max(nm, EPS));
  } else {
    nu = Math.atan2(r.y, r.x);
  }

  // Perifocal basis.
  const p = (hm * hm) / mu;
  const pHat = new Vector3();
  if (e > 1e-8) pHat.copy(eVec).divideScalar(e);
  else if (nm > EPS) pHat.copy(n).divideScalar(nm);
  else pHat.set(1, 0, 0);
  const qHat = new Vector3().crossVectors(h, pHat).normalize();

  const pe = p / (1 + e);
  const ap = e < 1 ? p / (1 - e) : Infinity;
  const period = e < 1 && Number.isFinite(a) ? 2 * Math.PI * Math.sqrt(a ** 3 / mu) : Infinity;

  return { a, e, i, raan, argp, nu, p, pe, ap, period, energy, h, eVec, pHat, qHat, closed: e < 1 };
}

/** Unpack the Rust core's 23-double elements record. */
function elementsFromPacked(w: Float64Array): Elements {
  return {
    a: w[0]!, e: w[1]!, i: w[2]!, raan: w[3]!, argp: w[4]!, nu: w[5]!,
    p: w[6]!, pe: w[7]!, ap: w[8]!, period: w[9]!, energy: w[10]!,
    h: new Vector3(w[11]!, w[12]!, w[13]!),
    eVec: new Vector3(w[14]!, w[15]!, w[16]!),
    pHat: new Vector3(w[17]!, w[18]!, w[19]!),
    qHat: new Vector3(w[20]!, w[21]!, w[22]!),
    closed: w[1]! < 1,
  };
}

/** Inertial state from classical elements. */
export function stateFromElements(el: Elements, mu = EARTH_MU): State {
  const p = el.p;
  const r = p / (1 + el.e * Math.cos(el.nu));
  const cnu = Math.cos(el.nu);
  const snu = Math.sin(el.nu);
  const pos = new Vector3().copy(el.pHat).multiplyScalar(r * cnu).addScaledVector(el.qHat, r * snu);
  const k = Math.sqrt(mu / p);
  const vel = new Vector3()
    .copy(el.pHat)
    .multiplyScalar(-k * snu)
    .addScaledVector(el.qHat, k * (el.e + cnu));
  return { r: pos, v: vel };
}

export function perifocalBasis(raan: number, i: number, argp: number): { p: Vector3; q: Vector3 } {
  const P1 = new Vector3(Math.cos(raan), Math.sin(raan), 0);
  const Q1 = new Vector3(-Math.sin(raan), Math.cos(raan), 0);
  const W = new Vector3(0, 0, 1);
  const P = P1.clone().multiplyScalar(Math.cos(argp)).addScaledVector(Q1, Math.sin(argp));
  // Q2 = W x P1
  const Q2 = new Vector3().crossVectors(W, P1);
  const Q = Q2.clone().multiplyScalar(Math.cos(argp)).addScaledVector(P1, -Math.sin(argp));
  // Rotate both by inclination about the ascending-node axis P1.
  const rot = new Matrix4().makeRotationAxis(P1, i);
  P.applyMatrix4(rot);
  Q.applyMatrix4(rot);
  return { p: P.normalize(), q: Q.normalize() };
}

/** Sample the conic for drawing, starting at periapsis. */
export function sampleConic(el: Elements, segments = 256): Vector3[] {
  const pts: Vector3[] = [];
  const e = el.e;
  if (!Number.isFinite(e) || el.p <= 0) return pts;
  const nuMax = e < 1 ? Math.PI * 2 : Math.acos(-1 / e) * 0.985;
  const steps = e < 1 ? segments : Math.max(24, Math.floor(segments * 0.5));
  for (let k = 0; k < steps; k++) {
    const nu = (nuMax * k) / steps;
    const r = el.p / (1 + e * Math.cos(nu));
    if (!(r > 0)) continue;
    const pos = new Vector3()
      .copy(el.pHat)
      .multiplyScalar(r * Math.cos(nu))
      .addScaledVector(el.qHat, r * Math.sin(nu));
    pts.push(pos);
  }
  if (e < 1) {
    const r = el.p / (1 + e);
    pts.push(new Vector3().copy(el.pHat).multiplyScalar(r));
  }
  return pts;
}

/** True anomaly at periapsis/apoapsis of the *current* conic. */
export function apsisStates(el: Elements, mu = EARTH_MU): { ap: Vector3 | null; pe: Vector3 } {
  const peR = el.p / (1 + el.e);
  const pe = el.pHat.clone().multiplyScalar(peR);
  let ap: Vector3 | null = null;
  if (el.e < 1) {
    const apR = el.p / (1 - el.e);
    ap = el.pHat.clone().multiplyScalar(-apR);
  }
  void mu;
  return { ap, pe };
}

/* ------------------------------------------------------------------ Kepler */
/* Universal-variable propagation (Vallado, Algorithm 8). Returns the state
 * after `dt` seconds on the two-body conic. No perturbations. */

export function propagateKepler(r0: Vector3, v0: Vector3, dt: number, mu = EARTH_MU): State {
  if (!Number.isFinite(dt) || dt === 0) {
    return { r: r0.clone(), v: v0.clone() };
  }
  if (coreReady()) {
    const w = coreKepler(r0.x, r0.y, r0.z, v0.x, v0.y, v0.z, dt, mu);
    if (w) return { r: new Vector3(w[0]!, w[1]!, w[2]!), v: new Vector3(w[3]!, w[4]!, w[5]!) };
  }
  const rm = r0.length();
  const vm = v0.length();
  const sqrtMu = Math.sqrt(mu);
  const alpha = 2 / rm - (vm * vm) / mu;
  const sigma0 = r0.dot(v0) / sqrtMu;

  // Initial guess.
  let chi: number;
  if (alpha > 1e-10) {
    chi = sqrtMu * dt * alpha;
  } else if (alpha < -1e-10) {
    const a = 1 / alpha;
    const num = -2 * mu * alpha * dt;
    const den = sigma0 + Math.sign(dt) * Math.sqrt(-mu * a) * (1 - rm * alpha);
    chi = Math.sign(dt) * Math.sqrt(-a) * Math.log(Math.max(num / den, 1e-12));
    if (!Number.isFinite(chi)) chi = (sqrtMu * dt) / rm;
  } else {
    chi = (sqrtMu * dt) / rm;
  }

  const c2 = (z: number): number => {
    if (z > 1e-6) return (1 - Math.cos(Math.sqrt(z))) / z;
    if (z < -1e-6) return (Math.cosh(Math.sqrt(-z)) - 1) / -z;
    return 0.5;
  };
  const c3 = (z: number): number => {
    if (z > 1e-6) {
      const s = Math.sqrt(z);
      return (s - Math.sin(s)) / (s * s * s);
    }
    if (z < -1e-6) {
      const s = Math.sqrt(-z);
      return (Math.sinh(s) - s) / (s * s * s);
    }
    return 1 / 6;
  };

  const solve = (x: number): { f: number; fp: number } => {
    const z = alpha * x * x;
    const zc2 = c2(z);
    const zc3 = c3(z);
    const r = x * x * zc2 + sigma0 * x * (1 - z * zc3) + rm * (1 - z * zc2);
    const f = sigma0 * x * x * zc2 + (1 - alpha * rm) * x * x * x * zc3 + rm * x - sqrtMu * dt;
    return { f, fp: r };
  };

  let solved = false;
  for (let k = 0; k < 80; k++) {
    const { f, fp } = solve(chi);
    if (Math.abs(f) < 1e-9) {
      solved = true;
      break;
    }
    if (!Number.isFinite(f) || !Number.isFinite(fp) || Math.abs(fp) < 1e-14) break;
    const next = chi - f / fp;
    if (!Number.isFinite(next)) break;
    if (Math.abs(next - chi) < 1e-12) {
      chi = next;
      solved = true;
      break;
    }
    chi = next;
  }
  if (!solved) {
    // Bisection fallback keeps a frame from poisoning the state.
    let lo = Math.min(chi, 0) - sqrtMu * Math.abs(dt) / rm - 1;
    let hi = Math.max(chi, 0) + sqrtMu * Math.abs(dt) / rm + 1;
    for (let k = 0; k < 100; k++) {
      const mid = (lo + hi) / 2;
      const { f } = solve(mid);
      if (Math.abs(f) < 1e-6) break;
      if ((solve(lo).f < 0) === (f < 0)) lo = mid;
      else hi = mid;
    }
    chi = (lo + hi) / 2;
  }

  const z = alpha * chi * chi;
  const zc2 = c2(z);
  const zc3 = c3(z);
  const f = 1 - (chi * chi) / rm * zc2;
  const g = dt - (chi * chi * chi) / sqrtMu * zc3;
  const rNew = new Vector3().copy(r0).multiplyScalar(f).addScaledVector(v0, g);
  const rmNew = rNew.length();
  const fdot = (sqrtMu / (rm * rmNew)) * (alpha * chi * chi * chi * zc3 - chi);
  const gdot = 1 - (chi * chi) / rmNew * zc2;
  const vNew = new Vector3().copy(r0).multiplyScalar(fdot).addScaledVector(v0, gdot);
  if (!Number.isFinite(rNew.x) || rmNew < 1) return { r: r0.clone(), v: v0.clone() };
  return { r: rNew, v: vNew };
}

/* ---------------------------------------------------------------- dynamics */

export interface ForceContext {
  /** Thrust acceleration in inertial m/s². */
  accel: Vector3;
  j2: boolean;
}

function accel(r: Vector3, v: Vector3, ctx: ForceContext, mu: number): Vector3 {
  const rm = r.length();
  const out = r.clone().multiplyScalar(-mu / (rm * rm * rm));
  if (ctx.j2) {
    const x = r.x, y = r.y, z = r.z;
    const r2 = rm * rm;
    const k = (1.5 * EARTH_J2 * mu * EARTH_RADIUS * EARTH_RADIUS) / (r2 * r2 * rm);
    const z2r2 = (5 * z * z) / r2;
    out.x += k * x * (z2r2 - 1);
    out.y += k * y * (z2r2 - 1);
    out.z += k * z * (z2r2 - 3);
  }
  // Atmospheric drag: exponential model, only meaningful below 200 km.
  const alt = rm - EARTH_RADIUS;
  if (alt < 200_000 && alt > 0) {
    const rho = 1.225 * Math.exp(-alt / 8500);
    const speed = v.length();
    const cdA = 0.6 * 8.0; // Cd * A (m²) for the stack
    const m = 4200;
    const drag = (-0.5 * rho * speed * speed * cdA) / m;
    out.addScaledVector(v, drag / Math.max(speed, 1e-6));
  }
  return out.add(ctx.accel);
}

/** One fixed RK4 step of the force model. */
export function stepRK4(
  r: Vector3,
  v: Vector3,
  dt: number,
  ctx: ForceContext,
  mu = EARTH_MU,
): State {
  if (coreReady()) {
    const w = coreRk4(
      r.x, r.y, r.z, v.x, v.y, v.z, dt,
      ctx.accel.x, ctx.accel.y, ctx.accel.z, ctx.j2, mu,
    );
    if (w) return { r: new Vector3(w[0]!, w[1]!, w[2]!), v: new Vector3(w[3]!, w[4]!, w[5]!) };
  }
  const a1 = accel(r, v, ctx, mu);
  const r2 = r.clone().addScaledVector(v, dt / 2);
  const v2 = v.clone().addScaledVector(a1, dt / 2);
  const a2 = accel(r2, v2, ctx, mu);
  const r3 = r.clone().addScaledVector(v2, dt / 2);
  const v3 = v.clone().addScaledVector(a2, dt / 2);
  const a3 = accel(r3, v3, ctx, mu);
  const r4 = r.clone().addScaledVector(v3, dt);
  const v4 = v.clone().addScaledVector(a3, dt);
  const a4 = accel(r4, v4, ctx, mu);
  const dr = new Vector3()
    .copy(v)
    .addScaledVector(v2, 2)
    .addScaledVector(v3, 2)
    .add(v4)
    .multiplyScalar(dt / 6);
  const dv = new Vector3()
    .copy(a1)
    .addScaledVector(a2, 2)
    .addScaledVector(a3, 2)
    .add(a4)
    .multiplyScalar(dt / 6);
  return { r: r.clone().add(dr), v: v.clone().add(dv) };
}

/** Time to next periapsis/apoapsis, positive seconds. */
export function timeToApsis(el: Elements, targetApoapsis: boolean, mu = EARTH_MU): number {
  if (!el.closed) return Infinity;
  if (coreReady()) {
    const t = coreTimeToApsis(el.a, el.e, el.nu, targetApoapsis, mu);
    if (t !== null) return t;
  }
  const nu = el.nu;
  const n = Math.sqrt(mu / el.a ** 3);
  let target = targetApoapsis ? Math.PI : 0;
  let dnu = target - nu;
  if (dnu <= 0) dnu += Math.PI * 2;
  // Convert true anomaly to mean anomaly difference.
  const E0 = 2 * Math.atan2(Math.sqrt(1 - el.e) * Math.sin(nu / 2), Math.sqrt(1 + el.e) * Math.cos(nu / 2));
  const E1 = 2 * Math.atan2(Math.sqrt(1 - el.e) * Math.sin((nu + dnu) / 2), Math.sqrt(1 + el.e) * Math.cos((nu + dnu) / 2));
  const M0 = E0 - el.e * Math.sin(E0);
  const M1 = E1 - el.e * Math.sin(E1);
  let dM = M1 - M0;
  if (dM < 0) dM += Math.PI * 2;
  return dM / n;
}

/** Apply a Δv impulse in the local TNW frame (along-track, radial, normal). */
export function applyImpulse(v: Vector3, r: Vector3, dvT: number, dvN: number, dvW: number): Vector3 {
  const t = v.clone().normalize();
  const h = new Vector3().crossVectors(r, v).normalize();
  const n = new Vector3().crossVectors(h, t).normalize(); // radial-ish, in plane
  return v
    .clone()
    .addScaledVector(t, dvT)
    .addScaledVector(n, dvN)
    .addScaledVector(h, dvW);
}

export function altitude(r: Vector3): number {
  return r.length() - EARTH_RADIUS;
}

export function periodOf(a: number, mu = EARTH_MU): number {
  return 2 * Math.PI * Math.sqrt(a ** 3 / mu);
}

/** Basis quaternion for a vessel: +Y nose, +Z dorsal. */
export function basisQuaternion(forward: Vector3, dorsal: Vector3): Quaternion {
  const y = forward.clone().normalize();
  let z = dorsal.clone();
  z.addScaledVector(y, -z.dot(y));
  if (z.lengthSq() < 1e-10) {
    z = Math.abs(y.z) > 0.9 ? new Vector3(1, 0, 0) : new Vector3(0, 0, 1);
    z.addScaledVector(y, -z.dot(y));
  }
  z.normalize();
  const x = new Vector3().crossVectors(y, z).normalize();
  const m = new Matrix4().makeBasis(x, y, z);
  return new Quaternion().setFromRotationMatrix(m);
}

export function localFrameQuaternion(r: Vector3, v: Vector3): Quaternion {
  // Nose along the velocity, dorsal toward open space: an upright, readable
  // attitude that puts the local horizon across the navball.
  return basisQuaternion(v, r);
}
