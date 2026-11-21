/* The frame budget as arithmetic (docs/PERFORMANCE.md §3).
 *
 * The claim under test is the one the docs make and the vision depends on: when nothing is changing,
 * no frame is requested -- and the room is therefore *provably* never re-presented while it is still.
 */
import { suite, ok, lt } from './harness.mjs';
import { createRequire } from 'module';
const require = createRequire(import.meta.url);
const S = require('../game/sim/schedule.js');

suite('schedule: idle is genuinely zero');
{
  const now = 1000;
  const d = S.schedule({ now: now, lastPresent: now, changeRate: 0.000001, intervalMs: 500 });
  ok(d.present === false, 'a dead-still room presents nothing, even though a frame is available');
}

suite('schedule: the cadence follows how fast the scene is changing');
{
  const now = 100000;
  const still = S.schedule({ now: now, lastPresent: now - 40, changeRate: 0.000001 });
  const slow = S.schedule({ now: now, lastPresent: now - 40, changeRate: 0.0005 });
  const busy = S.schedule({ now: now, lastPresent: now - 40, changeRate: 0.02 });
  ok(still.present === false, 'at 40 ms since the last present, a still room is not due');
  ok(slow.present === false, 'nor a slowly drifting one');
  ok(busy.present === true, 'but a fast-changing one is due immediately');

  // and given enough time, even a still room eventually presents
  const later = S.schedule({ now: now + 600, lastPresent: now, changeRate: 0.000001 });
  ok(later.present === true, 'a still room presents again after its interval, because the sun moves');
}

suite('schedule: a moving camera always gets a frame');
{
  const now = 5000;
  const d = S.schedule({ now: now, lastPresent: now, moving: true, intervalMs: 500 });
  ok(d.present === true, 'walking or looking is never interpolated away');
  ok(d.intervalMs <= 17, 'and it forces the fastest cadence while the camera moves');
}

suite('schedule: it backs off on a slow machine and recovers on a fast one');
{
  let interval = 125;
  for (let i = 0; i < 8; i++) {
    interval = S.schedule({ now: i * 100, lastPresent: 0, intervalMs: interval, frameMs: 40 }).intervalMs;
  }
  ok(interval > 125, 'a 40 ms frame time backs the cadence off (' + interval.toFixed(0) + ' ms interval)');
  for (let i = 0; i < 60; i++) {
    interval = S.schedule({ now: i * 100, lastPresent: 0, intervalMs: interval, frameMs: 5 }).intervalMs;
  }
  lt(interval, 30, 'a 5 ms frame time brings it back down to the fast ladder (' + interval.toFixed(1) + ' ms)');
  ok(interval >= 17, 'and never past a 60 Hz present');
}

suite('schedule: the rest state cannot chatter');
{
  // 60 fps ticks against a 17 ms quantum: without quantisation the phase walks and one frame slips
  // every ~20 ticks, forever. With quantisation the room settles.
  let lastPresent = 0, presents = 0;
  for (let i = 0; i < 600; i++) {
    const now = i * 16.667;
    const d = S.schedule({ now: now, lastPresent: lastPresent, changeRate: 0.000001 });
    if (d.present) { presents++; lastPresent = now - (now % d.quantum); }
  }
  ok(presents < 30, 'over 10 s of a still room the schedule presents ' + presents + ' times, not 600');
}
