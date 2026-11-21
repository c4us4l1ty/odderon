import { suite, ok, near, lt } from './harness.mjs';
import { createRequire } from 'module';
const require = createRequire(import.meta.url);
const P = require('../game/sim/provenance.js');

suite('provenance: the lineage of a number (bible 6.6)');
{
  const area = P.assumed(3.0, 'player', 'window area m2');
  const shgc = P.assumed(0.60, 'player', 'SHGC x curtain');
  const ghi = P.catalog(800, 25, 'clear-sky model');
  const q = P.derived('Q = G A shgc', (g, a, s) => g * a * s, [ghi, area, shgc]);

  near(q.value, 1440, 1e-9, 'solar gain = 800 W/m2 * 3 m2 * 0.6 = 1440 W');
  ok(q.sigma > 0, 'uncertainty propagated automatically from the inputs');

  const tr = P.trace(q);
  ok(tr.rows.length === 4, 'the trace has all four nodes');
  ok(tr.assumed === 2, 'exactly two of them are flagged as player assumptions');
  ok(tr.rows[0].id === q.id, 'the trace starts at the number on the gauge');

  // sensitivity: d(Q)/d(shgc) = G*A = 2400
  const s = P.sensitivity(q, shgc);
  near(s.value, 2400, 1, 'd(solar gain)/d(SHGC) = G*A = 2400 W per unit SHGC');

  // a measured input must never be reported as an assumption
  const meas = P.measured(301.2, 0.15, 'AM2302');
  const t2 = P.trace(meas);
  ok(t2.rows[0].kind === 'measured', 'a sensor reading is tagged measured, with its noise');
  ok(t2.assumed === 0, 'and contributes zero assumptions');

  // the budget tells the player where the uncertainty actually comes from
  const budget = P.uncertaintyBudget(q);
  ok(Object.keys(budget).some(k => k.indexOf('catalog') === 0), 'uncertainty budget separates catalog error');
}
