/* Odderon — the world: time, the sun, the event queue.
 *
 * There is no game loop (docs/PERFORMANCE.md I3). Time advances because something asked what the
 * state is at a later time. The only things that "happen" are events, and they are queued.
 *
 * Clock: local mean solar time at the chosen town, from real solar geometry (NOAA low-precision
 * algorithm, valid ~1900-2100, +-0.01 deg). The town's latitude/longitude are catalog data, not
 * invented, so the sun is where the sun actually is.
 */
(function (global) {
  'use strict';
  var M = global.ODMath || (typeof require !== 'undefined' ? require('./dmath.js') : null);

  // Tanegashima, Japan — a real launch town on a real coast. Catalog provenance.
  var TOWNS = {
    tanegashima: { name: 'Tanegashima', lat: 30.4019, lon: 130.9681, tz: 9, utcOffsetMin: 540 },
    boca_chica: { name: 'Boca Chica', lat: 25.9971, lon: -97.1539, tz: -6, utcOffsetMin: -360 },
    mojave: { name: 'Mojave', lat: 35.0594, lon: -118.1515, tz: -8, utcOffsetMin: -480 },
    kourou: { name: 'Kourou', lat: 5.1595, lon: -52.6469, tz: -3, utcOffsetMin: -180 }
  };

  /* ---- solar position (NOAA low-precision) ---- */
  function solar(dayOfYear, hoursLocal, latDeg, lonDeg, utcOffsetMin) {
    var g = 2 * M.PI / 365 * (dayOfYear - 1 + (hoursLocal - 12) / 24);
    var eqtime = 229.18 * (0.000075 + 0.001868 * M.cos(g) - 0.032077 * M.sin(g)
      - 0.014615 * M.cos(2 * g) - 0.040849 * M.sin(2 * g));            // minutes
    var decl = 0.006918 - 0.399912 * M.cos(g) + 0.070257 * M.sin(g)
      - 0.006758 * M.cos(2 * g) + 0.000907 * M.sin(2 * g)
      - 0.002697 * M.cos(3 * g) + 0.00148 * M.sin(3 * g);              // radians
    var tst = hoursLocal * 60 + eqtime + 4 * lonDeg - utcOffsetMin;    // true solar time, min
    var ha = (tst / 4 - 180) * M.DEG;                                  // hour angle
    var lat = latDeg * M.DEG;
    var cosZen = M.sin(lat) * M.sin(decl) + M.cos(lat) * M.cos(decl) * M.cos(ha);
    var zen = M.acos(M.clamp(cosZen, -1, 1));
    var elev = M.HALF_PI - zen;
    // azimuth from north, clockwise
    var az = M.atan2(M.sin(ha), M.cos(ha) * M.sin(lat) - M.tan(decl) * M.cos(lat)) + M.PI;
    return { elev: elev, zen: zen, az: az, decl: decl, eqtime: eqtime };
  }

  // Clear-sky direct-normal + global horizontal irradiance, by air mass. Kasten-Young airmass.
  function irradiance(elevRad, opts) {
    opts = opts || {};
    var turb = opts.turbidity || 2.4;             // summer, slightly hazy
    var elevDeg = elevRad / M.DEG;
    if (elevDeg <= 0) return { dni: 0, ghi: 0, dhi: 0, elevDeg: elevDeg };
    var airmass = 1 / (M.sin(elevRad) + 0.50572 * M.pow(elevDeg + 6.07995, -1.6364));
    // Beer-Lambert with a mean extinction. Calibrated so clear-sky DNI at the zenith is ~950 W/m2,
    // which is what a sea-level site actually sees; anything above ~1000 is a model error.
    var trans = M.exp(-turb * 0.152 * airmass);
    var dni = 1361 * trans;                       // solar constant 1361 W/m2 (real catalog value)
    var sinE = M.sin(elevRad);
    var dhi = 0.10 * dni * sinE + 25 * M.smoothstep(0, 8, elevDeg);   // sky brightness floor
    var ghi = dni * sinE + dhi;
    return { dni: dni, ghi: ghi, dhi: dhi, elevDeg: elevDeg, airmass: airmass };
  }

  // Sky colour from the sun. Warm sun vs cool sky-shadow is the whole art direction, so it is a
  // function here rather than a constant in the shader: the shader asks the kernel.
  function skyColors(sol, elevDegIn) {
    var e = elevDegIn !== undefined ? elevDegIn : (sol.elev !== undefined ? sol.elev / M.DEG : 0);
    var day = M.smoothstep(-6, 12, e);            // 0 night .. 1 full day
    var gold = M.smoothstep(3, 30, e);            // 1 = high sun/white; 0 = golden hour
    // zenith: deep blue by day, near-black by night
    var z = [M.lerp(0.02, 0.10, day), M.lerp(0.03, 0.30, day), M.lerp(0.06, 0.62, day)];
    // horizon: warm at low sun
    var h = [M.lerp(0.10, 0.62, day), M.lerp(0.11, 0.72, day), M.lerp(0.16, 0.82, day)];
    var warmth = (1 - gold);
    h[0] = M.lerp(h[0], 0.95, warmth * day);
    h[1] = M.lerp(h[1], 0.62, warmth * day);
    h[2] = M.lerp(h[2], 0.42, warmth * day);
    return { zenith: z, horizon: h, day: day, gold: gold };
  }

  /* ---- event queue: the only source of time advancement ---- */
  function Queue() { this.items = []; this.seq = 0; }
  Queue.prototype.at = function (t, fn, label) {
    this.items.push({ t: t, fn: fn, label: label || '', s: this.seq++ });
    this.items.sort(function (a, b) { return a.t - b.t || a.s - b.s; });
    return this;
  };
  Queue.prototype.before = function (t) {
    var out = [];
    while (this.items.length && this.items[0].t <= t) out.push(this.items.shift());
    return out;
  };
  Queue.prototype.pending = function () { return this.items.length ? this.items[0].t : Infinity; };

  /* ---- the world ---- */
  function World(o) {
    o = o || {};
    this.town = TOWNS[o.town || 'tanegashima'];
    this.day = o.day || 226;                        // 14 August
    this.clock = o.clock || (16.0);                 // hours, local mean time
    this.rate = 1;                                  // sim seconds per real second (1 = real time)
    this.queue = new Queue();
    this.t = 0;                                     // sim seconds since boot
    this.epoch = o.epoch || 'gaia-dr3+horizons-2026.44';
    this.seed = o.seed || 4471;
    this.listeners = [];
  }
  World.prototype.on = function (fn) { this.listeners.push(fn); };
  World.prototype.emit = function (ev) { for (var i = 0; i < this.listeners.length; i++) this.listeners[i](ev); };

  World.prototype.hoursLocal = function () {
    return (this.clock + this.t / 3600) % 24;
  };
  World.prototype.sun = function () {
    return solar(this.day, this.hoursLocal(), this.town.lat, this.town.lon, this.town.utcOffsetMin);
  };
  World.prototype.light = function () {
    var s = this.sun();
    return { sol: s, irr: irradiance(s.elev), sky: skyColors(s) };
  };

  // Advance to sim time t, firing every queued event on the way. NOTHING else advances.
  World.prototype.advanceTo = function (t) {
    var due = this.queue.before(t);
    for (var i = 0; i < due.length; i++) due[i].fn(due[i]);
    this.t = Math.max(this.t, t);
    return due.length;
  };

  var W = { World: World, Queue: Queue, TOWNS: TOWNS, solar: solar, irradiance: irradiance, skyColors: skyColors };
  if (typeof module !== 'undefined' && module.exports) module.exports = W;
  global.ODWorld = W;
})(typeof globalThis !== 'undefined' ? globalThis : this);
