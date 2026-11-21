/* Physics conformance: the claims in the docs, turned into assertions.
 * These are the numbers the design bible asserts, so they must hold or the doc is wrong.
 */
import { suite, ok, near, lt } from './harness.mjs';
import { createRequire } from 'module';
const require = createRequire(import.meta.url);
require('../game/sim/rng.js');
const M = require('../game/sim/dmath.js');
const Th = require('../game/sim/thermo.js');
const Wd = require('../game/sim/world.js');
const K = require('../game/sim/marks.js');

function settle(curtain, hours) {
  const m = Th.apartment(1);
  m.apartment.curtain = curtain;
  for (const n of m.nodes) n.T = 295;
  m.hvacs[0].on = false;
  for (let i = 0; i < hours * 3600; i++) {
    m.G = 800 * curtain;
    m.apartment.room.aper.shgc = 0.60 * curtain;
    m.step(1);
  }
  return m.nodes[0].T;
}

suite('apartment: the Day-1 experiment (bible 6.7)');
{
  const open = settle(1.0, 6) - 273.15;
  const closed = settle(0.05, 6) - 273.15;
  const swing = open - closed;
  console.log('      AC off, steady state: open ' + open.toFixed(2) + ' C, closed ' +
    closed.toFixed(2) + ' C, swing ' + swing.toFixed(2) + ' K');
  ok(swing > 1.0, 'closing the curtain cools the room by more than 1 K');
  lt(swing, 12.0, 'and by less than 12 K (no runaway model)');
}

suite('apartment: the AC is a real cycling machine, and it can lose');
{
  function run(G, curtain, hours) {
    const m = Th.apartment(1);
    for (const n of m.nodes) n.T = 299;
    const ac = m.hvacs[0];
    ac.setpoint = 299.15; ac.hyst = 0.5; ac.on = true;
    for (let i = 0; i < 4 * 3600; i++) {           // settle
      m.G = G * curtain; m.apartment.room.aper.shgc = 0.60 * curtain;
      m.netW = 0; m.step(1);
    }
    let onCount = 0, samples = 0, transitions = 0, prev = ac.on, W = 0;
    for (let i = 0; i < hours * 3600; i++) {
      m.G = G * curtain; m.apartment.room.aper.shgc = 0.60 * curtain;
      m.netW = 0; m.step(1);
      if (ac.on !== prev) transitions++;
      prev = ac.on;
      if (ac.on) onCount++;
      W += m.netW; samples++;
    }
    return { duty: onCount / samples, transitions, W: W / samples, T: m.nodes[0].T, sat: m.saturated(0) };
  }

  const afternoon = run(300, 1.0, 2);          // late afternoon sun, curtain open
  console.log('      late afternoon: duty ' + (afternoon.duty * 100).toFixed(0) + '%, ' +
    afternoon.transitions + ' transitions, ' + afternoon.W.toFixed(0) + ' W');
  ok(afternoon.transitions > 2, 'the thermostat cycles (bang-bang with hysteresis), it does not sit still');
  ok(afternoon.duty > 0.3 && afternoon.duty < 1.0, 'and holds a duty cycle strictly between 0 and 100%');
  ok(Math.abs(afternoon.T - 299.15) < 0.6, 'holding the setpoint within the hysteresis band');

  // a saturated unit is a physical failure, and the draw tells the story
  const noonOpen = run(1006, 1.0, 2);
  const noonClosed = run(1006, 0.05, 2);
  console.log('      midday open  : ' + noonOpen.W.toFixed(0) + ' W, ' + (noonOpen.T - 273.15).toFixed(1) +
    ' C, saturated=' + noonOpen.sat);
  console.log('      midday closed: ' + noonClosed.W.toFixed(0) + ' W, ' + (noonClosed.T - 273.15).toFixed(1) +
    ' C, saturated=' + noonClosed.sat);
  ok(noonOpen.sat, 'at midday a 2 kW unit is oversized for the loss and saturates');
  ok(noonOpen.W > noonClosed.W + 100, 'the wall meter sees the curtain: ' +
    Math.round(noonOpen.W - noonClosed.W) + ' W more with it open');
  ok(noonOpen.T > noonClosed.T, 'and the room is measurably warmer, because the unit lost the fight');
}

suite('world: solar geometry against published values');
{
  const t = Wd.TOWNS.tanegashima;
  // Aug 14, latitude 30.4 N, declination ~ +14.2 deg  =>  noon elevation ~ 73.8 deg
  const noon = Wd.solar(226, 12.4, t.lat, t.lon, t.utcOffsetMin);
  near(noon.elev / M.DEG, 73.8, 1.0, 'solar noon elevation at Tanegashima on 14 Aug is ~73.8 deg');

  // equinox at the equator: noon should be ~90 deg
  const eq = Wd.solar(80, 12.0, 0.0, 0.0, 0);
  near(eq.elev / M.DEG, 89.5, 2.0, 'equinox at the equator: noon elevation ~90 deg');

  // the sun must not rise in the west
  const am = Wd.solar(226, 8, t.lat, t.lon, t.utcOffsetMin);
  const pm = Wd.solar(226, 16, t.lat, t.lon, t.utcOffsetMin);
  ok(am.az < Math.PI && pm.az > Math.PI, 'morning azimuth is east (az<180), afternoon west (az>180)');

  const n = Wd.solar(226, 0, t.lat, t.lon, t.utcOffsetMin);
  ok(n.elev < 0, 'midnight: the sun is below the horizon');

  const irrNoon = Wd.irradiance(noon.elev);
  console.log('      clear-sky DNI at noon: ' + irrNoon.dni.toFixed(0) + ' W/m2, GHI ' + irrNoon.ghi.toFixed(0));
  ok(irrNoon.dni > 700 && irrNoon.dni < 1000, 'clear-sky DNI at sea level is 700-1000 W/m2 (not unphysical)');
  ok(irrNoon.ghi > irrNoon.dni * Math.sin(noon.elev) * 0.9, 'GHI includes the diffuse component');
  ok(Wd.irradiance(-0.2).dni === 0, 'below the horizon there is no direct beam, not a negative one');
}

suite('world: time advances only through the queue');
{
  const w = new Wd.World({ town: 'tanegashima', day: 226, clock: 16.0 });
  let fired = 0;
  w.queue.at(10, () => fired++);
  w.queue.at(5, () => fired += 10);
  w.advanceTo(7);
  ok(fired === 10, 'advanceTo(7) fires only the event at t=5');
  w.advanceTo(20);
  ok(fired === 11, 'advanceTo(20) fires the event at t=10, in order');
  ok(w.queue.pending() === Infinity, 'queue is empty afterwards');
}

suite('grease pencil: the loop (bible 6.1)');
{
  const L = new K.Ledger();
  const m = L.mark({ t: 0, what: 'gauge', metric: 'room.T', value: 299.0 });
  ok(/^[0-9a-f]{8}$/.test(m.hash), 'the mark commits to an 8-hex digest');
  const m2 = L.mark({ t: 0, what: 'gauge', metric: 'room.T', value: 299.0 });
  ok(m.hash === m2.hash, 'the same prediction hashes identically (reproducible commitment)');
  const m3 = L.mark({ t: 0, what: 'gauge', metric: 'room.T', value: 299.1 });
  ok(m.hash !== m3.hash, 'a different prediction hashes differently');

  // a point mark is rewarded for being *surprised*: delta is the teaching signal
  L.resolve(m, 100, 301.0);
  near(m.delta, 2.0, 1e-12, 'delta is reality minus expectation');
  const s0 = K.score(299.0, 0, 299.0, 0.5);
  const s2 = K.score(299.0, 0, 301.0, 0.5);
  lt(s0.value, 0.05, 'a perfectly predicted point mark earns ~nothing: nothing was learned');
  ok(s2.value > 0.5, 'a wrong point mark earns the reward: that is the loop firing');

  // a band mark is rewarded for being right within a stated honest uncertainty
  const inBand = K.score(299.0, 1.5, 300.0, 0.5);
  const outBand = K.score(299.0, 0.2, 301.0, 0.5);
  ok(inBand.value > 0.7, 'inside a stated band scores well (you knew what you did not know)');
  lt(outBand.value, 0.05, 'a dishonestly narrow band collapses when the world disagrees');

  // the epistemic peak: surprise x convergence x stakes x novelty
  const L2 = new K.Ledger();
  const a = L2.mark({ t: 0, what: 'room', metric: 'room.T', value: 297.5 });
  L2.resolve(a, 60, 299.3, { stakes: 1.0 });
  const b = L2.mark({ t: 60, what: 'room', metric: 'room.T', value: 297.5 });
  L2.resolve(b, 120, 299.3, { stakes: 1.0 });
  lt(b.peak, a.peak, 'the same divergence scores lower the second time: novelty decays');
  ok(a.peak > 0.5, 'the first surprise of a class is a real peak');

  const st = L2.stats();
  ok(st.n === 2, 'stats count resolved, unwiped marks');
  near(st.bias, 1.8, 1e-9, 'bias is the signed mean error, in the metric units');
}
