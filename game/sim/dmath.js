/* Odderon — deterministic math.
 *
 * Why this file exists (docs/VISION.md §5.2): IEEE-754 guarantees + - * / sqrt, but NOT
 * sin/cos/exp/log/pow — those come from the host libm and differ in the last ulp between browsers
 * and native builds. That breaks replay hashes, which breaks Esse. So every transcendental in the
 * kernel comes from here, with fixed coefficients, and nothing in game/sim/ may call Math.sin & co.
 *
 * Coefficients are stored as ascending-order arrays and evaluated with Horner, which is (a) easy to
 * audit, (b) trivial to port to Rust, and (c) fixed-point reproducible.
 */
(function (global) {
  'use strict';

  var PI = 3.141592653589793;
  var TAU = 6.283185307179586;
  var HALF_PI = 1.5707963267948966;
  var LN2 = 0.6931471805599453;
  var INV_LN2 = 1.4426950408889634;
  var DEG = 0.017453292519943295;

  // Horner over ascending coefficients: c0 + x*(c1 + x*(c2 + ...))
  function poly(c, x) {
    var r = c[c.length - 1];
    for (var i = c.length - 2; i >= 0; i--) r = c[i] + x * r;
    return r;
  }

  var F64 = new Float64Array(1);
  var U32 = new Uint32Array(F64.buffer);

  function pow2i(n) {
    // Exact in every engine: 2^n is representable for -1074 <= n <= 1023, so either write the
    // exponent bits directly (normal range) or multiply down by two (exact in the subnormal range).
    if (n > 1023) return Infinity;
    if (n < -1074) return 0;
    if (n >= -1022) {
      U32[0] = 0;
      U32[1] = ((n + 1023) << 20) >>> 0;
      return F64[0];
    }
    var v = 1;
    for (var i = 0; i < -n; i++) v *= 0.5;
    return v;
  }

  /* ---- log: exponent extraction + atanh series on the mantissa ---- */
  // ln(m) for m in [0.75, 1.5), via the atanh series in t = (m-1)/(m+1), |t| <= 0.2, so t^2 <= 0.04
  // and eleven terms is far below double precision. Step by exact powers of two to get there.
  var LN_C = [1, 1 / 3, 1 / 5, 1 / 7, 1 / 9, 1 / 11, 1 / 13, 1 / 15, 1 / 17];

  function lnMantissa(m) {
    var k = 0;
    while (m > 1.5) { m *= 0.5; k += 1; }
    while (m < 0.75) { m *= 2; k -= 1; }
    var t = (m - 1) / (m + 1);
    return 2 * t * poly(LN_C, t * t) + k * LN2;
  }

  function log(x) {
    if (x <= 0) return x === 0 ? -Infinity : NaN;
    if (x === Infinity) return Infinity;
    F64[0] = x;
    var e = ((U32[1] >>> 20) & 0x7ff) - 1023;
    // Rewrite ONLY the exponent bits in the high word. Zeroing the low word here would throw away
    // 32 mantissa bits and cost ~1e-3 relative accuracy near 1 -- caught by test/dmath.test.mjs.
    U32[1] = ((U32[1] & ~(0x7ff << 20)) | (1023 << 20)) >>> 0;
    return lnMantissa(F64[0]) + e * LN2;
  }
  function log2(x) { return log(x) * INV_LN2; }
  function log10(x) { return log(x) * 0.4342944819032518; }

  /* ---- exp: exact power-of-two scaling + Taylor on [0,1) ---- */
  var EXP_COEFF = [1,
    1, 1 / 2, 1 / 6, 1 / 24, 1 / 120, 1 / 720, 1 / 5040, 1 / 40320,
    1 / 362880, 1 / 3628800, 1 / 39916800, 1 / 479001600, 1 / 6227020800,
    1 / 87178291200, 1 / 1307674368000, 1 / 20922789888000];

  function exp(x) {
    if (x > 709) return Infinity;
    if (x < -745) return 0;
    var n = Math.floor(x * INV_LN2);
    var r = x - n * LN2;
    if (r < 0) { r += LN2; n -= 1; }
    return poly(EXP_COEFF, r) * pow2i(n);
  }

  /* ---- trig: reduce to [-PI/4, PI/4], evaluate both, select by quadrant ---- */
  var SIN_C = [0, 1, 0, -1 / 6, 0, 1 / 120, 0, -1 / 5040, 0, 1 / 362880, 0, -1 / 39916800];
  var COS_C = [1, 0, -1 / 2, 0, 1 / 24, 0, -1 / 720, 0, 1 / 40320, 0, -1 / 3628800, 0, 1 / 479001600];

  function quadrant(x) {
    var k = Math.round(x / HALF_PI);        // nearest multiple of PI/2
    return { k: k, r: x - k * HALF_PI };    // |r| <= PI/4
  }

  function sin(x) {
    if (x !== x || x === Infinity || x === -Infinity) return NaN;
    var q = quadrant(x);
    var qn = ((q.k % 4) + 4) % 4;
    var s = poly(SIN_C, q.r), c = poly(COS_C, q.r);
    switch (qn) {
      case 0: return s;
      case 1: return c;
      case 2: return -s;
      default: return -c;
    }
  }

  function cos(x) {
    if (x !== x || x === Infinity || x === -Infinity) return NaN;
    var q = quadrant(x);
    var qn = ((q.k % 4) + 4) % 4;
    var s = poly(SIN_C, q.r), c = poly(COS_C, q.r);
    switch (qn) {
      case 0: return c;
      case 1: return -s;
      case 2: return -c;
      default: return s;
    }
  }

  function tan(x) { return sin(x) / cos(x); }

  // atan(a) = 2*atan( a / (1 + sqrt(1 + a^2)) )  -- one half-angle step.
  // For a in [0,1] the reduced argument lies in [0, 0.414], so the atanh series converges fast
  // and eleven terms is comfortably below double precision.
  // alternating: atan(t) = t - t^3/3 + t^5/5 - ...
  var ATAN_C = [1, -1 / 3, 1 / 5, -1 / 7, 1 / 9, -1 / 11, 1 / 13, -1 / 15, 1 / 17, -1 / 19, 1 / 21, -1 / 23];

  function atan(x) {
    var neg = x < 0;
    var a = neg ? -x : x;
    if (a === Infinity) return neg ? -HALF_PI : HALF_PI;
    var inv = a > 1;
    if (inv) a = 1 / a;
    // Two half-angle steps. One step leaves t <= 0.414, and the next omitted term (t^25/25) is
    // then ~1e-11 -- which the accuracy test caught. Two steps give t <= 0.199 and the same term
    // falls to ~1e-18, at the cost of one extra sqrt.
    var t = a / (1 + Math.sqrt(1 + a * a));
    t = t / (1 + Math.sqrt(1 + t * t));
    var s = 4 * t * poly(ATAN_C, t * t);
    if (inv) s = HALF_PI - s;
    return neg ? -s : s;
  }

  function atan2(y, x) {
    if (x === 0) {
      if (y > 0) return HALF_PI;
      if (y < 0) return -HALF_PI;
      return 0;
    }
    var a = atan(y / x);
    if (x > 0) return a;
    return y >= 0 ? a + PI : a - PI;
  }

  function asin(x) {
    if (x > 1 || x < -1) return NaN;
    return atan2(x, Math.sqrt(1 - x * x));
  }
  function acos(x) { return HALF_PI - asin(x); }

  function sqrt(x) { return Math.sqrt(x); }     // IEEE-754 correctly rounded everywhere; safe

  function pow(x, y) {
    if (x === 0) return y === 0 ? 1 : 0;
    if (x < 0) return NaN;                      // the kernel never needs a complex result
    if (x === 1) return 1;
    return exp(y * log(x));
  }

  function sinh(x) { var e = exp(x); return (e - 1 / e) * 0.5; }
  function cosh(x) { var e = exp(x); return (e + 1 / e) * 0.5; }

  function clamp(x, a, b) { return x < a ? a : (x > b ? b : x); }
  function lerp(a, b, t) { return a + (b - a) * t; }
  function smoothstep(e0, e1, x) {
    var t = clamp((x - e0) / (e1 - e0), 0, 1);
    return t * t * (3 - 2 * t);
  }

  var D = {
    PI: PI, TAU: TAU, HALF_PI: HALF_PI, DEG: DEG, LN2: LN2,
    sin: sin, cos: cos, tan: tan,
    atan: atan, atan2: atan2, asin: asin, acos: acos,
    exp: exp, log: log, ln: log, log2: log2, log10: log10, pow: pow, sqrt: sqrt,
    sinh: sinh, cosh: cosh,
    clamp: clamp, lerp: lerp, smoothstep: smoothstep,
    toRad: function (d) { return d * DEG; },
    toDeg: function (r) { return r / DEG; }
  };

  if (typeof module !== 'undefined' && module.exports) module.exports = D;
  global.ODMath = D;
})(typeof globalThis !== 'undefined' ? globalThis : this);
