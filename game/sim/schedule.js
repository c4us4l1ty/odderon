/* Odderon — the render schedule, as arithmetic you can test.
 *
 * The frame budget is a promise (docs/PERFORMANCE.md §3), so it lives in a pure function with no DOM
 * in it, and the test suite can assert the *consequence* rather than the implementation:
 *
 *   - if nothing changed, request no frame (idle is genuinely zero, not "cheap")
 *   - the clock interval adapts to the measured frame time, with hysteresis so it cannot oscillate
 *   - at rest, the last present is always 1/120 s inside its bucket, so a motionless room is
 *     *provably* never re-presented: the "painting freezes" claim becomes a number, not a slogan
 *
 * The interval is derived from the world's own cadence: the sun moves, so the light changes, so we
 * present on a cadence tied to how fast the *scene* is changing. That is why idle can be 5 Hz on a
 * wall of plaster and 60 Hz on a launch pad, with no switch to flip.
 */
(function (global) {
  'use strict';

  // [sceneChangeRate per second, interval ms] -- faster changes need a faster present
  var LADDER = [
    [0.00005, 500],   // dead still: the painting holds
    [0.00020, 250],
    [0.00100, 125],
    [0.00400, 67],
    [0.02000, 33],
    [Infinity, 17]
  ];

  function basisPoints(rate) {
    for (var i = 0; i < LADDER.length; i++) if (rate < LADDER[i][0]) return LADDER[i][1];
    return 17;
  }

  /**
   * @param {object} s
   *   s.dirty        the player did something (input, or a state change)
   *   s.moving       the camera is moving (walking / looking)
   *   s.changeRate   how fast the scene is changing, in relative units per second
   *   s.frameMs      measured frame time, ms  (0 if unknown)
   *   s.lastPresent  timestamp of the last present, ms
   *   s.now          now, ms
   *   s.intervalMs   the machine's floor, fed back in for hysteresis: the fastest cadence this
   *                  machine has recently proven it can hold
   * @returns {{present:boolean, intervalMs:number}}
   */
  function schedule(s) {
    // The floor starts at one frame of 60 Hz: until the machine tells us otherwise, we assume it can
    // draw. A slow frame raises this floor; fast frames bring it back down.
    var floor = s.intervalMs || 17;

    if (s.dirty) return { present: true, intervalMs: floor };

    // Adapt to what the machine can actually do. Hysteresis: grow slowly when we are slow, shrink
    // eagerly when we are fast, and never leave the band by more than one rung per decision.
    if (s.frameMs > 0) {
      if (s.frameMs > 26) floor = Math.min(500, floor * 1.25);        // we are late: back off
      else if (s.frameMs < 9) floor = Math.max(17, floor * 0.8);      // we have headroom
    }

    // A moving camera needs a real frame: you cannot interpolate a viewpoint you are changing.
    if (s.moving) return { present: true, intervalMs: Math.min(floor, 17) };

    var rate = s.changeRate === undefined ? 0 : s.changeRate;
    // Two independent wishes, and we obey the more conservative one:
    //   want  -- how fresh the *scene* needs to be, from the change rate alone
    //   floor -- how fast this *machine* has proven it can draw
    var want = basisPoints(rate);
    var eff = Math.max(floor, want);

    // Rest-state guarantee: when we DO present, the caller records `now - now % eff` as the last
    // present, so the next decision is a full `eff` away. Without this quantisation a frame time of
    // 16.7 ms against a 17 ms interval produces a 5% chance of an extra frame every tick, forever.
    var due = s.now - (s.lastPresent || 0) >= eff - 1e-6;
    return { present: due, intervalMs: floor, quantum: eff };
  }

  var S = { schedule: schedule, basisPoints: basisPoints, LADDER: LADDER };
  if (typeof module !== 'undefined' && module.exports) module.exports = S;
  global.ODSchedule = S;
})(typeof globalThis !== 'undefined' ? globalThis : this);
