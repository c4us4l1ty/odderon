/* Odderon — the grease pencil (docs/GAME-DESIGN-BIBLE.md §6.1).
 *
 * Expectation is a physical gesture, not a form: a yellow tick snapped onto gauge glass, a chalk tick
 * on a scope, a pin on a blueprint. It costs ~0.5 s, so the loop can fire once every two minutes.
 *
 * A mark is also a *model commitment*: it records which simplification the player implied, which is
 * what makes the ghost twin (§6.2) possible with no extra input. If you mark "the needle will settle
 * at 299 K" while the AC is cycling, you have implicitly asserted "the AC removes heat at full
 * capacity, continuously". The kernel holds that assertion and can render it.
 */
(function (global) {
  'use strict';

  function Mark(o) {
    this.id = o.id;
    this.t = o.t;                     // sim time of the mark, s
    this.what = o.what;               // 'gauge' | 'scope' | 'pin' | 'room'
    this.metric = o.metric;           // the observed quantity, e.g. 'room.T'
    this.value = o.value;             // the player's expectation, in SI
    this.band = o.band || 0;          // half-width = honest "I don't know yet"
    this.bandKind = this.band > 0 ? 'band' : 'point';
    // The assumed model implied by the mark. Defaults are the naive ones a beginner would hold.
    this.assumed = o.assumed || {};
    this.wiped = false;
    this.resolvedT = null;
    this.actual = null;
    this.delta = null;
    this.score = null;                // proper score, set on resolve
    this.hash = null;                 // commitment, set on commit
  }

  /* ---- commitment: a stable, order-independent digest of the prediction ----
   * No Math.random, no timestamps from the platform clock: the hash is a function of the prediction
   * and the sim time only, so it is reproducible and comparable across machines. */
  function fnv1a(str) {
    var h = 0x811c9dc5;
    for (var i = 0; i < str.length; i++) {
      h ^= str.charCodeAt(i);
      h = (h + ((h << 1) + (h << 4) + (h << 7) + (h << 8) + (h << 24))) >>> 0;
    }
    return ('00000000' + h.toString(16)).slice(-8);
  }

  Mark.prototype.commit = function () {
    var s = [this.t.toFixed(3), this.what, this.metric,
      this.value.toPrecision(12), this.band.toPrecision(6),
      JSON.stringify(this.assumed)].join('|');
    this.hash = fnv1a(s);
    this.committedAt = this.t;
    return this.hash;
  };

  /* ---- the scale of a prediction -----------------------------------------------------
   * A proper score cannot be dimensionless-by-magic: it needs to know what "being wrong" costs.
   * Each metric declares its own tolerance, in SI, because that is the only honest way:
   *   0.5 K on a room air temperature  is a confident engineer
   *   0.5 K on a coolant inlet temp    is a guess
   */
  var SCALES = {
    'room.T': 0.5,        // K
    'room.busW': 25,      // W
    'glass.T': 1.0,       // K
    'wall.T': 0.5         // K
  };
  function scaleOf(metric, value) {
    if (SCALES[metric]) return SCALES[metric];
    return Math.max(1e-9, Math.abs(value) * 0.02);
  }

  /* ---- resolution: the needle settles, or it does not ---- */
  Mark.prototype.resolve = function (t, actual) {
    this.resolvedT = t;
    this.actual = actual;
    this.delta = actual - this.value;                 // this IS delta: reality - expectation
    this.score = score(this.value, this.band, actual, scaleOf(this.metric, this.value));
    return this;
  };

  /* ---- proper scoring -------------------------------------------------------------
   * Two regimes, because a calibrated engineer and a guessing one must not be scored the same:
   *
   *   point mark (band = 0): the reward is *surprise*, |delta|, normalised by the gauge's own scale.
   *     - if you were right, delta ~ 0 and there is nothing to learn: score -> 0
   *     - if you were wrong, the score rises and the loop fires
   *   band mark (band > 0):  the reward is being right *within a stated honest uncertainty*.
   *     - inside the band: positive (you knew what you did not know)
   *     - outside: decays with distance, and collapses if the band was dishonestly narrow
   *
   * Both are bounded [0,1] so they can be summed into a rate the game can display.
   */
  function score(predicted, band, actual, scale) {
    scale = scale || scaleOf('', predicted);
    var err = actual - predicted;
    if (band <= 0) {
      return { kind: 'surprise', value: 1 - Math.exp(-Math.abs(err) / (3 * scale)), err: err };
    }
    var z = Math.abs(err) / band;
    return { kind: 'calibration', value: Math.exp(-0.5 * z * z), err: err, z: z };
  }

  /* ---- the epistemic peak (bible §9.5) ---------------------------------------------
   * peak = (delta/sigma) x convergence x stakes x novelty
   *   |delta|/sigma  how surprising relative to what the player claimed to know
   *   convergence    1 right-or-wrong-both-informative: a resolve, not a guess
   *   stakes         mass x energy x irreversibility (0..1, supplied by the game)
   *   novelty        decays with repeated exposure to the same class of divergence
   */
  function peakScore(mark, opts) {
    opts = opts || {};
    // A point mark has no stated uncertainty, so "how surprising was this?" is measured against
    // the metric's own tolerance -- the same number that defines a good prediction here.
    var sigma = mark.band > 0 ? mark.band : scaleOf(mark.metric, mark.value);
    var surprise = Math.abs(mark.delta) / sigma;
    var convergence = mark.score ? (mark.score.kind === 'calibration' ? 1 : 0.85) : 0;
    var stakes = opts.stakes === undefined ? 0.5 : opts.stakes;
    var novelty = opts.novelty === undefined ? 1 : opts.novelty;
    return surprise * convergence * stakes * novelty;
  }

  function Ledger() {
    this.marks = [];
    this.n = 0;
    this.peak = 0;
    this.peakMark = null;
    this.lastPeakAt = -1e9;
    this._classes = {};                // novelty bookkeeping
  }
  Ledger.prototype.mark = function (o) {
    o.id = this.n++;
    var m = new Mark(o);
    m.commit();
    this.marks.push(m);
    return m;
  };
  Ledger.prototype.wipe = function (m) { m.wiped = true; return m; };

  Ledger.prototype.resolve = function (m, t, actual, opts) {
    m.resolve(t, actual);
    var cls = m.what + ':' + m.metric;
    var seen = this._classes[cls] || 0;
    var novelty = 1 / (1 + 0.6 * seen);
    this._classes[cls] = seen + 1;
    var ps = peakScore(m, { stakes: opts && opts.stakes, novelty: novelty });
    m.peak = ps;
    if (ps > this.peak) { this.peak = ps; this.peakMark = m; this.lastPeakAt = t; }
    return m;
  };

  // session statistics the game turns into plain language ("your mean error is 3.1%, your bias +0.8%")
  Ledger.prototype.stats = function () {
    var done = this.marks.filter(function (m) { return m.actual !== null && !m.wiped; });
    if (!done.length) return null;
    var absErr = 0, bias = 0, inside = 0;
    for (var i = 0; i < done.length; i++) {
      var m = done[i];
      absErr += Math.abs(m.delta);
      bias += m.delta;
      if (m.band > 0 && Math.abs(m.delta) <= m.band) inside++;
    }
    return {
      n: done.length,
      meanErr: absErr / done.length,
      bias: bias / done.length,
      calibration: inside / done.length,
      marks: done
    };
  };

  var K = { Mark: Mark, Ledger: Ledger, score: score, peakScore: peakScore, fnv1a: fnv1a, scaleOf: scaleOf, SCALES: SCALES };
  if (typeof module !== 'undefined' && module.exports) module.exports = K;
  global.ODMarks = K;
})(typeof globalThis !== 'undefined' ? globalThis : this);
