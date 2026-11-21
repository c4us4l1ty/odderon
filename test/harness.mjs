/* A deliberately tiny assertion harness: no dependencies, runs with `node test/run.mjs`. */
let pass = 0, fail = 0, current = '';
const failures = [];

export function suite(name) { current = name; console.log('\n  ' + name); }

export function ok(cond, msg, detail) {
  if (cond) { pass++; console.log('    \x1b[32mPASS\x1b[0m ' + msg); }
  else {
    fail++; failures.push(current + ' :: ' + msg + (detail ? '  (' + detail + ')' : ''));
    console.log('    \x1b[31mFAIL\x1b[0m ' + msg + (detail ? '\n         ' + detail : ''));
  }
}

export function near(actual, expected, tol, msg) {
  const d = Math.abs(actual - expected);
  ok(d <= tol, msg, 'got ' + actual + ', expected ' + expected + ' +/-' + tol);
}

export function lt(actual, limit, msg) {
  ok(actual < limit, msg, 'got ' + actual + ', limit ' + limit);
}

export function report() {
  console.log('\n  ' + (fail === 0 ? '\x1b[32m' : '\x1b[31m') + pass + ' passed, ' + fail + ' failed\x1b[0m');
  if (fail) { console.log('\n  failures:'); failures.forEach(f => console.log('   - ' + f)); }
  return fail === 0 ? 0 : 1;
}
