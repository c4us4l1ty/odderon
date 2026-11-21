/* Odderon — deterministic PRNG (xoshiro128** over integer state).
 *
 * Separate from dmath so the "no float reaches the RNG" rule is visible in the file list.
 */
(function (global) {
  'use strict';

  function Rng(seed) {
    this.s = new Uint32Array(4);
    var x = (seed | 0) || 1;
    for (var i = 0; i < 4; i++) {
      x ^= x << 13; x ^= x >>> 17; x ^= x << 5;   // xorshift to spread the seed, once
      this.s[i] = x >>> 0;
    }
    if (!(this.s[0] | this.s[1] | this.s[2] | this.s[3])) this.s[0] = 1;
  }

  Rng.prototype.next = function () {
    var s = this.s;
    var a = s[0], b = s[1], c = s[2], d = s[3];
    var t = (b << 9) >>> 0;
    c = (c ^ a) >>> 0; d = (d ^ b) >>> 0; b = (b ^ c) >>> 0; a = (a ^ d) >>> 0;
    c = (c ^ t) >>> 0;
    d = ((d << 11) | (d >>> 21)) >>> 0;
    s[0] = a; s[1] = b; s[2] = c; s[3] = d;
    return ((b * 5) >>> 7) / 4294967296;          // [0,1)
  };

  Rng.prototype.range = function (lo, hi) { return lo + (hi - lo) * this.next(); };
  Rng.prototype.int = function (n) { return (this.next() * n) | 0; };

  if (typeof module !== 'undefined' && module.exports) module.exports = Rng;
  global.ODRng = Rng;
})(typeof globalThis !== 'undefined' ? globalThis : this);
