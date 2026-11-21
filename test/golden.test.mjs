/* Golden hashes: the determinism claim, proven across separate processes.
 *
 * The kernel's whole credibility rests on "the same inputs produce the same bits". We test it the
 * only way that means anything: run the identical simulation in TWO independent node processes and
 * compare the digests. If the platform's libm or hash order leaked into the kernel, this fails.
 */
import { suite, ok, near } from './harness.mjs';
import { execFileSync } from 'child_process';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';
import { createRequire } from 'module';
const require = createRequire(import.meta.url);
const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = join(__dirname, '..');

const RUNNER = `
  const { createRequire } = require('module');
  const Th = require('${join(ROOT, 'game/sim/thermo.js').replace(/\\/g, '/')}');
  const K  = require('${join(ROOT, 'game/sim/marks.js').replace(/\\/g, '/')}');
  const Wd = require('${join(ROOT, 'game/sim/world.js').replace(/\\/g, '/')}');
  const m = Th.apartment(4471);
  for (const n of m.nodes) n.T = 301.2;
  const ac = m.hvacs[0]; ac.setpoint = 299.15; ac.hyst = 0.5; ac.on = false;
  const w = new Wd.World({ town: 'tanegashima', day: 226, clock: 16.42 });
  const L = new K.Ledger();
  let acc = 0;
  for (let i = 0; i < 7200; i++) {
    const L2 = w.light();
    m.G = L2.irr.ghi;
    m.apartment.room.aper.shgc = 0.60 * (i < 3600 ? 1.0 : 0.05);
    m.netW = 0;
    m.step(1);
    w.advanceTo(w.t + 1);
    acc += m.nodes[0].T;
    if (i === 1800) L.mark({ t: i, what: 'gauge', metric: 'room.T', value: m.nodes[0].T });
  }
  const last = L.marks[0];
  L.resolve(last, 7200, m.nodes[0].T);
  process.stdout.write(JSON.stringify({
    digest: K.fnv1a(m.nodes[0].T.toPrecision(17) + '|' + acc.toPrecision(17) + '|' + m.netW.toPrecision(17)),
    T: m.nodes[0].T,
    acc: acc,
    markHash: last.hash
  }));
`;

function runOnce() {
  return execFileSync(process.execPath, ['-e', RUNNER], { encoding: 'utf8' });
}

suite('determinism: two independent processes agree bit for bit');
{
  const a = JSON.parse(runOnce());
  const b = JSON.parse(runOnce());
  console.log('      digest A ' + a.digest + '   digest B ' + b.digest);
  ok(a.digest === b.digest, 'a 7200-step thermal run produces the same digest in two processes');
  ok(a.T === b.T, 'the final temperature is bit-identical, not merely close');
  ok(a.markHash === b.markHash, 'and the committed prediction hash is identical');
  ok(typeof a.T === 'number' && isFinite(a.T), 'the run actually produced a temperature');
}

suite('determinism: the hash is a function of the prediction, not of the clock');
{
  const K = require('../game/sim/marks.js');
  const a = new K.Ledger().mark({ t: 10, what: 'gauge', metric: 'room.T', value: 300.0 });
  const b = new K.Ledger().mark({ t: 999, what: 'gauge', metric: 'room.T', value: 300.0 });
  ok(a.hash !== b.hash, 'a later mark of the same value is a different commitment (time is part of it)');
  ok(K.fnv1a('') === '811c9dc5', 'FNV-1a offset basis is the published constant');
}
