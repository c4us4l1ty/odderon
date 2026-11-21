/* Deterministic math conformance.
 *
 * These are accuracy tests, not determinism tests: determinism is proven across *processes* by
 * test/golden.test.mjs, which compares hashes of long trajectories run in separate node processes.
 */
import { suite, ok, near, lt } from './harness.mjs';
import { createRequire } from 'module';
const require = createRequire(import.meta.url);
const M = require('../game/sim/dmath.js');

function maxAbsErr(f, g, xs) {
  let e = 0, at = 0;
  for (const x of xs) { const d = Math.abs(f(x) - g(x)); if (d > e) { e = d; at = x; } }
  return { e, at };
}
function grid(n, lo, hi) { const a = []; for (let i = 0; i < n; i++) a.push(lo + (hi - lo) * i / (n - 1)); return a; }

suite('dmath: transcendental accuracy');
{
  const s = maxAbsErr(M.sin, Math.sin, grid(20000, -60, 60));
  lt(s.e, 1e-11, 'sin / 2e4 points over [-60,60]: max abs err < 1e-11');

  const c = maxAbsErr(M.cos, Math.cos, grid(20000, -60, 60));
  lt(c.e, 1e-11, 'cos: max abs err < 1e-11');

  const t = maxAbsErr(M.tan, Math.tan, grid(5000, -1.4, 1.4));
  lt(t.e, 1e-9, 'tan over (-pi/2, pi/2): max abs err < 1e-9');

  let ex = 0;
  for (const x of grid(20000, -700, 700)) {
    const g = Math.exp(x);
    if (g > 0 && isFinite(g)) ex = Math.max(ex, Math.abs(M.exp(x) - g) / g);
  }
  lt(ex, 1e-13, 'exp: max rel err < 1e-13 over [-700,700]');

  let lg = 0;
  for (const x of grid(20000, 1e-8, 1e12)) {
    lg = Math.max(lg, Math.abs(M.log(x) - Math.log(x)) / Math.max(1e-300, Math.abs(Math.log(x))));
  }
  lt(lg, 1e-12, 'log: max rel err < 1e-12 over [1e-8,1e12]');

  const a = maxAbsErr(M.atan, Math.atan, grid(20000, -1000, 1000));
  lt(a.e, 1e-11, 'atan: max abs err < 1e-11');

  let a2 = 0;
  for (const y of grid(60, -3, 3)) for (const x of grid(60, -3, 3)) {
    a2 = Math.max(a2, Math.abs(M.atan2(y, x) - Math.atan2(y, x)));
  }
  lt(a2, 1e-11, 'atan2: max abs err < 1e-11 over a 60x60 grid');

  const as = maxAbsErr(M.asin, Math.asin, grid(2000, -0.999, 0.999));
  lt(as.e, 1e-11, 'asin: max abs err < 1e-11');
  const ac = maxAbsErr(M.acos, Math.acos, grid(2000, -0.999, 0.999));
  lt(ac.e, 1e-11, 'acos: max abs err < 1e-11');

  let pw = 0;
  for (let i = 0; i < 3000; i++) {
    const x = 0.01 + i * 0.7, y = -6 + (i % 40) * 0.35;
    pw = Math.max(pw, Math.abs(M.pow(x, y) - Math.pow(x, y)) / Math.pow(x, y));
  }
  lt(pw, 1e-12, 'pow: max rel err < 1e-12');
}

suite('dmath: edge cases that break naive implementations');
{
  ok(M.sin(0) === 0, 'sin(0) is exactly 0');
  ok(M.cos(0) === 1, 'cos(0) is exactly 1');
  ok(M.exp(0) === 1, 'exp(0) is exactly 1');
  ok(M.log(1) === 0, 'log(1) is exactly 0');
  ok(M.log(0) === -Infinity, 'log(0) is exactly -Infinity');
  ok(M.log(-1) !== M.log(-1), 'log(negative) is NaN');
  ok(M.exp(-745) === 0, 'exp(-745) underflows to 0, not to a denormal');
  ok(M.exp(710) === Infinity, 'exp(710) overflows to Infinity');
  ok(M.sqrt(2) === Math.sqrt(2), 'sqrt is correctly rounded (delegated to the platform)');
  // exactness at the integers a fluid network actually uses
  near(M.log(2), 0.6931471805599453, 1e-15, 'log(2) to 1e-15');
  near(M.exp(1), 2.718281828459045, 1e-15, 'e to 1e-15');
}
