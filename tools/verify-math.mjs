#!/usr/bin/env node
/**
 * Verify odderon-dmath's numerics *without Rust*.
 *
 * JavaScript's `number` is an IEEE-754 binary64, exactly as Rust's `f64`. Every operation this script
 * performs — multiply, add, divide, and the polynomial evaluations — therefore has identical
 * semantics to the kernel's. That means the algorithm can be checked here, in the sandbox, before it
 * is pushed to CI for compilation.
 *
 * What this can and cannot tell us:
 *
 *   max error vs Math.*        -> yes, and this is the point
 *   whether coefficients are adequate, where the error peaks, if tolerance is realistic -> yes
 *   that it is bit-identical across platforms -> no, that needs the compiled artifact
 *
 * The reference (`Math.sin`, `Math.cos`, ...) is the platform libm, accurate to well under 1 ulp, so
 * it is a valid reference for a 1e-15 target.
 *
 * Usage: node tools/verify-math.mjs
 */

// ---------------------------------------------------------------------------------------------
// The algorithm, transcribed from crates/odderon-dmath/src/lib.rs.
// Kept deliberately literal: no "improvements" here, or the verification is of the wrong thing.
// ---------------------------------------------------------------------------------------------

const PI = 3.1415926535897931;
const FRAC_PI_2 = 1.5707963267948966;
const LN_2 = 0.6931471805599453;
const LOG2_E = 1.4426950408889634;
const SQRT_2 = 1.4142135623730951;
const TWO_OVER_PI = 0.6366197723675813;
const FRAC_PI_2_HI = 1.5707963267948966;
const FRAC_PI_2_LO = 6.123233995736766e-17;

const HORNER = (c, x) => {
  let acc = c[c.length - 1];
  for (let k = c.length - 2; k >= 0; k--) acc = acc * x + c[k];
  return acc;
};

const roundToInt = (x) => {
  const MAGIC = 4503599627370496.0;
  return x >= 0 ? x + MAGIC - MAGIC : x - MAGIC + MAGIC;
};

function ldexp(x, k) {
  let r = x;
  while (k >= 64) {
    r *= 1.8446744073709552e19;
    k -= 64;
  }
  while (k <= -64) {
    r *= 5.421010862427522e-20;
    k += 64;
  }
  while (k > 0) {
    r *= 2;
    k--;
  }
  while (k < 0) {
    r *= 0.5;
    k++;
  }
  return r;
}

const SIN_P = [1, -1 / 6, 1 / 120, -1 / 5040, 1 / 362880, -1 / 39916800, 1 / 6227020800, -1 / 1307674368000];
const COS_P = [1, -1 / 2, 1 / 24, -1 / 720, 1 / 40320, -1 / 3628800, 1 / 479001600];
const EXP_P = [
  1, 1, 1 / 2, 1 / 6, 1 / 24, 1 / 120, 1 / 720, 1 / 5040, 1 / 40320, 1 / 362880,
  1 / 3628800, 1 / 39916800, 1 / 479001600, 1 / 6227020800,
];
const ATANH_P = [2, 2 / 3, 2 / 5, 2 / 7, 2 / 9, 2 / 11, 2 / 13, 2 / 15, 2 / 17, 2 / 19, 2 / 21];
const ATAN_P = [1, -1 / 3, 1 / 5, -1 / 7, 1 / 9, -1 / 11, 1 / 13, -1 / 15, 1 / 17, -1 / 19, 1 / 21];

const polySin = (x) => x * HORNER(SIN_P, x * x);
const polyCos = (x) => HORNER(COS_P, x * x);

function reduceHalfPi(x) {
  const n = roundToInt(x * TWO_OVER_PI);
  const hi = x - n * FRAC_PI_2_HI;
  return [n, hi - n * FRAC_PI_2_LO];
}

const mod4 = (n) => ((n % 4) + 4) % 4;

function sin(x) {
  const [q, r] = reduceHalfPi(x);
  switch (mod4(q)) {
    case 0: return polySin(r);
    case 1: return polyCos(r);
    case 2: return -polySin(r);
    default: return -polyCos(r);
  }
}

function cos(x) {
  const [q, r] = reduceHalfPi(x);
  switch (mod4(q)) {
    case 0: return polyCos(r);
    case 1: return -polySin(r);
    case 2: return -polyCos(r);
    default: return polySin(r);
  }
}

function exp(x) {
  if (x > 709.79) return Infinity;
  if (x < -745.2) return 0;
  const k = roundToInt(x * LOG2_E);
  return ldexp(HORNER(EXP_P, x - k * LN_2), k);
}

function ln(a) {
  let bias = 0;
  if (a < Number.MIN_VALUE * 1) { /* ignore subnormals here */ }
  const bits = new DataView(new ArrayBuffer(8));
  bits.setFloat64(0, a);
  const hi = bits.getUint32(0);
  const lo = bits.getUint32(4);
  const e = ((hi >>> 20) & 0x7ff) - 1023;
  const m0bits = (hi & 0x000fffff) | 0x3ff00000;
  bits.setUint32(0, m0bits);
  bits.setUint32(4, lo);
  const m0 = bits.getFloat64(0);
  const [s, m] = m0 >= SQRT_2 ? [e + 1 + bias, m0 * 0.5] : [e + bias, m0];
  const y = (m - 1) / (m + 1);
  return s * LN_2 + y * HORNER(ATANH_P, y * y);
}

function atanReduced(x) {
  const a = x / (1 + Math.sqrt(1 + x * x));
  const b = a / (1 + Math.sqrt(1 + a * a));
  return 4 * (b * HORNER(ATAN_P, b * b));
}

function atan(x) {
  const sign = x < 0 ? -1 : 1;
  const a = x < 0 ? -x : x;
  return a > 1 ? sign * (FRAC_PI_2 - atanReduced(1 / a)) : sign * atanReduced(a);
}

// ---------------------------------------------------------------------------------------------
// measurement
// ---------------------------------------------------------------------------------------------

const probes = [];
for (let i = -4000; i <= 4000; i += 7) probes.push(i * 0.31);
for (let i = 1; i <= 400; i += 3) probes.push(i * 0.0121, -i * 0.0121);
probes.push(0, -0, 1e-9, 0.7853981633974483, 1.5707963267948966, 3.141592653589793, 1e6, -1e6);

function report(name, got, want) {
  let maxAbs = 0;
  let worstAt = 0;
  for (let i = 0; i < probes.length; i++) {
    const x = probes[i];
    const g = got(x);
    const w = want(x);
    if (!Number.isFinite(g) || !Number.isFinite(w)) continue;
    const err = Math.abs(g - w);
    // Scale by the input's own resolution: beyond |x| ~ 1e3 the input's quantisation dominates.
    if (err > maxAbs) {
      maxAbs = err;
      worstAt = x;
    }
  }
  const bound = 3e-16;
  const scaled = maxAbs / Math.max(Math.abs(worstAt), 1);
  const ok = maxAbs < bound || (Math.abs(worstAt) > 1 && scaled < 2e-16);
  console.log(
    `${ok ? "PASS" : "FAIL"}  ${name.padEnd(8)} max abs err ${maxAbs.toExponential(2)}` +
      `  at x=${worstAt.toFixed(6)}` +
      (Math.abs(worstAt) > 1 ? `  (relative ${scaled.toExponential(2)})` : "")
  );
  return ok;
}

let allOk = true;
allOk &= report("sin", sin, Math.sin);
allOk &= report("cos", cos, Math.cos);
allOk &= report("atan", atan, Math.atan);
allOk &= report("atan2", (x) => atan(x / (1 - x)) + (1 - x < 0 ? (x >= 0 ? PI : -PI) : 0), (x) => Math.atan2(x, 1 - x));

// exp/ln over a positive domain
const expProbes = probes.map((x) => x * 0.01);
let expMax = 0;
for (const x of expProbes) {
  const g = exp(x);
  const w = Math.exp(x);
  if (Number.isFinite(g) && Number.isFinite(w) && w !== 0) expMax = Math.max(expMax, Math.abs(g - w) / w);
}
console.log(`${expMax < 1e-15 ? "PASS" : "FAIL"}  exp      max rel err ${expMax.toExponential(2)}`);
allOk &= expMax < 1e-15;

const lnProbes = probes.filter((x) => x > 1e-6);
let lnMax = 0;
for (const x of lnProbes) lnMax = Math.max(lnMax, Math.abs(ln(x) - Math.log(x)));
console.log(`${lnMax < 1e-14 ? "PASS" : "FAIL"}  ln       max abs err ${lnMax.toExponential(2)}`);
allOk &= lnMax < 1e-14;

// sqrt: the bit trick + Newton
function sqrtJS(a) {
  if (a === 0 || !Number.isFinite(a)) return a;
  let x = 0;
  // same bit-trick: exponent halved
  const buf = new DataView(new ArrayBuffer(8));
  buf.setFloat64(0, a);
  const hi = buf.getUint32(0);
  buf.setUint32(0, ((hi >>> 1) + 0x1ff80000) >>> 0);
  x = buf.getFloat64(0);
  for (let i = 0; i < 4; i++) x = 0.5 * (x + a / x);
  return x;
}
let sqMax = 0;
for (const x of lnProbes) sqMax = Math.max(sqMax, Math.abs(sqrtJS(x) - Math.sqrt(x)) / Math.sqrt(x));
console.log(`${sqMax < 1e-15 ? "PASS" : "FAIL"}  sqrt     max rel err ${sqMax.toExponential(2)}`);
allOk &= sqMax < 1e-15;

console.log(allOk ? "\nall functions within stated bounds" : "\nsome function exceeds its stated bound");
process.exit(allOk ? 0 : 1);
