/* Odderon — Block 1: The Room.
 *
 * What you can do here, and why each thing exists:
 *   walk and look            -> the hand and the eye (P3: you are in the room, not on a map)
 *   read the wall thermometer-> the same solver that will run your capsule (§6.7)
 *   close the curtain        -> the Day-1 experiment: 1440 W of solar gain becomes 120 W
 *   flip the switch          -> the AC's compressor, with inrush, duty and its own voice
 *   the grease pencil        -> mark where the needle will settle, then watch it not (§6.1)
 *   the visor                -> provenance of the number, and the ghost of what you assumed (§6.2/6.6)
 *   golden hour              -> the epistemic peak, detected, not scripted (§9.5)
 *
 * There is no game loop. `present()` is called when something changes, and when nothing is pending no
 * frame is requested at all (PERFORMANCE.md I3).
 */
(function (global) {
  'use strict';

  var M = global.ODMath, Wd = global.ODWorld, Th = global.ODThermo, K = global.ODMarks,
      P = global.ODProv, Hud = global.ODHud, Renderer = global.ODRenderer, Glsl = global.ODGlsl,
      Audio = global.ODAudio, Rng = global.ODRng, Sched = global.ODSchedule;

  var App = {
    // ---- state -------------------------------------------------------------------------------
    running: false, t: 0, lastReal: 0, raf: 0, frames: 0, framesRequested: 0,
    curtain: 1.0, acOn: false, visor: false, visorPage: 'provenance',
    pointer: { x: 0, y: 0, down: false, locked: false },
    // spawn on the working floor by the door, looking across the platform to the window: the light
    // is the first thing you see, and the bed is the second
    cam: { pos: [-2.10, 1.62, -1.60], yaw: 0.75, pitch: -0.12 },
    keys: {},
    marks: null, ledger: null, model: null, world: null, audio: null,
    hud: null, renderer: null,
    peakGlow: 0, peakFired: 0, goldenDone: false,
    history: [],           // [{t, T}] for the ghost twin
    drawer: [],
    fps: 0, _fpsN: 0, _fpsT: 0,
    dirty: true,
    // ---- the render schedule (PERFORMANCE.md §3, sim/schedule.js) ----
    lastPresent: 0, intervalMs: 17, frameMs: 0, presented: 0, skipped: 0,
    quality: 1.0,          // adaptive: 1.0 = full DPR, lowered on slow machines
    changeRate: 0,         // how fast the scene is changing (drives the present cadence)
    reticle: 0,            // 0..1 fade of the crosshair, so it does not pop

    // ---- boot ---------------------------------------------------------------------------------
    boot: function () {
      var self = this;
      this.canvas = document.getElementById('gl');
      this.overlay = document.getElementById('hud');
      this.hud = new Hud(this.overlay);
      this.audio = new Audio();

      try {
        this.renderer = new Renderer(this.canvas, Glsl);
      } catch (e) {
        document.getElementById('fatal').textContent =
          'This browser could not start WebGL2, which Odderon needs. (' + e.message + ')';
        document.getElementById('fatal').style.display = 'block';
        return;
      }

      // ---- the world: a real town, a real date, real solar geometry ----
      this.world = new Wd.World({ town: 'tanegashima', day: 226, clock: 16.42, seed: 4471 });
      if (location.hash.indexOf('dusk') >= 0) this.world.clock = 18.35;

      // ---- the apartment: the spacecraft solver, not a special case ----
      this.model = Th.apartment(this.world.seed);
      this.model.hvacs[0].setpoint = 299.15;
      this.model.hvacs[0].hyst = 0.5;

      // ---- the loop's memory ----
      this.ledger = new K.Ledger();
      this.pendingMark = null;

      // ---- the drawer: one open anomaly, written in someone else's handwriting ----
      this.drawer.push('envelope from the pad, 2 days old: "nozzle throat 3% under drawing"');

      // ---- input ----
      this._bindInput();
      this._resize();
      addEventListener('resize', function () { self._resize(); });

      // ---- start ----
      this.running = true;
      this.lastReal = performance.now();
      this._askFrame();

      // the first frame is also the point at which audio may start
      var begin = function () {
        if (self.audio.start()) self.audio.update(self._audioState());
        self._firstGesture = true;
      };
      addEventListener('pointerdown', function once() {
        removeEventListener('pointerdown', once);
        begin();
      }, { once: true });
      addEventListener('keydown', function once2() {
        removeEventListener('keydown', once2);
        begin();
      }, { once: true });
    },

    // ---- render-on-demand: the only frame scheduler ------------------------------------------
    _askFrame: function () {
      if (this.raf) return;
      var self = this;
      this.framesRequested++;
      this.raf = requestAnimationFrame(function (ts) { self.raf = 0; self._frame(ts); });
    },

    _frame: function (ts) {
      var realDt = Math.min(0.1, (ts - this.lastReal) / 1000);

      // Frame *cost*, not frame interval. Comparing rAF timestamps only ever reports the display's
      // refresh rate, and a governor fed that number fights the vsync instead of the shader.
      var t0 = performance.now();

      var moving = this._move(realDt);
      this.world.advanceTo(this.world.t + realDt);
      this.t = this.world.t;
      this.model.advanceTo(this.t);
      this._record();
      this._checkPeak();

      // How fast is the scene changing? The sun, the needle, the compressor.
      this.changeRate = this._changeRate();

      // Ask the schedule, not the vsync. This is the whole performance story in one call.
      var decided = Sched.schedule({
        dirty: this.dirty, moving: moving, changeRate: this.changeRate,
        frameMs: this.frameMs, now: ts, lastPresent: this.lastPresent,
        intervalMs: this.intervalMs
      });
      this.intervalMs = decided.intervalMs;

      if (decided.present) {
        this.dirty = false;
        this.lastPresent = ts - (ts % decided.quantum);
        this._draw(ts);
        this.presented++;
      } else {
        this.skipped++;
      }

      // One correction per frame: the previous frame's measured cost (smoothed).
      this.frameMs = this.frameMs ? this.frameMs * 0.82 + (performance.now() - t0) * 0.18
                                  : (performance.now() - t0);
      this.frames++;
      this._fpsN++;
      if (ts - this._fpsT > 500) {
        this.fps = this._fpsN * 1000 / (ts - this._fpsT);
        this._fpsN = 0; this._fpsT = ts;
        this._governQuality();
      }

      // Keep the loop alive only while the scene is actually changing, the player is moving, or a
      // present was skipped and is now due. Otherwise: nothing is scheduled at all.
      var stillChanging = this.changeRate > 1e-6 || moving || this.dirty || !decided.present
        || Math.abs(this.reticle - (this.pointer.locked ? 1 : 0)) > 0.02;
      if (stillChanging) this._askFrame();
      else this._idle();
    },

    // How fast is the scene changing, per second, in relative units. Used only to pick a cadence.
    _changeRate: function () {
      var h = this.world.hoursLocal();
      var prev = this._prevHour === undefined ? h : this._prevHour;
      this._prevHour = h;
      // 1 hour of sun motion per second of wall clock is "fast"; a slow drift is 1e-5
      var sunRate = Math.abs(h - prev);
      var T = this.model.nodes[0].T;
      var dT = this._prevT === undefined ? 0 : Math.abs(T - this._prevT);
      this._prevT = T;
      var ac = this.model.hvacs[0];
      var acRate = ac.on ? 0.002 : 0.0004;
      return Math.max(sunRate * 0.02, dT * 0.06, acRate);
    },

    // If the machine cannot hold the cadence, drop the resolution before dropping the cadence:
    // a softer image at 30 fps is better than a sharp one at 11.
    _governQuality: function () {
      if (this.fps < 26 && this.quality > 0.55) {
        this.quality = Math.max(0.55, this.quality - 0.08);
        this._resize();
      } else if (this.fps > 55 && this.quality < 1.0) {
        this.quality = Math.min(1.0, this.quality + 0.04);
        this._resize();
      }
    },

    _idle: function () {
      // Nothing is scheduled. The compositor holds the last surface. This is the honest idle:
      // zero rAF, zero timers, zero allocation, no GPU work. A key or pointer wakes it.
      this.idle = true;
    },

    _worldMoving: function () {
      // The room temperature is always drifting toward equilibrium, so the needle is always
      // *slightly* moving. We do not draw for a sub-0.01 K change; that is the difference between a
      // simulation and a renderer that never sleeps.
      var last = this._lastT;
      var T = this.model.nodes[0].T;
      if (last === undefined) { this._lastT = T; return true; }
      if (Math.abs(T - last) > 0.01) { this._lastT = T; return true; }
      // still moving in the world (sun), but sub-perceptual for the room
      return this._sunChanged();
    },

    _sunChanged: function () {
      var h = this.world.hoursLocal();
      if (this._lastH === undefined) { this._lastH = h; return true; }
      if (Math.abs(h - this._lastH) > 0.0008) { this._lastH = h; return true; }  // ~3 s of game time
      return false;
    },

    // ---- input --------------------------------------------------------------------------------
    _bindInput: function () {
      var self = this;
      var c = this.overlay;

      addEventListener('keydown', function (e) {
        var k = e.key.toLowerCase();
        self.keys[k] = true;
        if (k === 'v') { self.visor = !self.visor; self._wake(); }
        if (k === 'p') { self.visorPage = 'provenance'; self.visor = true; self._wake(); }
        if (k === 'g') { self.visorPage = 'ghost'; self.visor = true; self._wake(); }
        if (k === 'm') {
          // you carry a China marker. It marks whatever instrument you are looking at.
          self._placeMark();
        }
        if (k === 'escape') {
          // Esc returns the cursor (the browser does this itself once the pointer is captured; the
          // explicit call covers browsers that do not). It must never be swallowed: if we handled it
          // and the browser also handled it, the second press would appear to do nothing at all.
          if (document.pointerLockElement) {
            document.exitPointerLock();
          } else if (self.visor) {
            self.visor = false;          // no capture to release, so Esc lifts the visor instead
          }
          self._wake();
        }
        if (k === 'e') { self._toggleCurtain(); }
        if (k === 'f') { self._toggleAC(); }
        if (k === 'r') { self._resolveMark(); }
        if (k === 'h') { document.getElementById('help').classList.toggle('open'); }
        self._askFrame();
      });
      addEventListener('keyup', function (e) { self.keys[e.key.toLowerCase()] = false; });

      // Pointer capture. The HUD canvas is pointer-events:none (it must be, or it eats the click),
      // so the request has to live on the document -- listening on the canvas was the bug that made
      // the camera feel fixed.
      var canvasEl = self.canvas;
      function requestLock() {
        if (document.pointerLockElement) return;
        if (canvasEl.requestPointerLock) {
          try { canvasEl.requestPointerLock(); } catch (err) { /* Safari: needs a user gesture */ }
        }
      }
      document.addEventListener('pointerdown', function (e) {
        // Anything that is real UI keeps its click. Without this the document handler captured the
        // pointer first and the frame read-out checkbox could never be ticked.
        var t = e.target;
        if (t && t.closest && t.closest('#bar,#help,[data-ui]')) { self._wake(); return; }
        if (!self.pointer.locked) requestLock();
        else self.pointer.down = true;
        self._wake();
      });
      // double-click also captures; a locked pointer means you look, not click UI
      document.addEventListener('dblclick', function () { requestLock(); });

      addEventListener('pointerup', function () { self.pointer.down = false; });
      document.addEventListener('pointermove', function (e) {
        if (document.pointerLockElement) {
          var s = 0.0020;
          self.cam.yaw += (e.movementX || 0) * s;
          self.cam.pitch = M.clamp(self.cam.pitch - (e.movementY || 0) * s, -1.35, 1.35);
          self._wake();
        } else {
          self.pointer.x = e.clientX; self.pointer.y = e.clientY;
        }
      });
      document.addEventListener('pointerlockchange', function () {
        self.pointer.locked = !!document.pointerLockElement;
        var hint = document.getElementById('lockhint');
        if (hint) hint.style.opacity = self.pointer.locked ? '0' : '1';
        // releasing the pointer must also release the keys, or you keep walking after Esc
        if (!self.pointer.locked) self._clearKeys();
        self._wake();
      });
      // and losing focus must not leave you pressed against a wall forever
      addEventListener('blur', function () { self._clearKeys(); self.pointer.down = false; });
      c.addEventListener('touchstart', function (e) { self._touch(e, true); }, { passive: true });
      c.addEventListener('touchmove', function (e) { self._touch(e, false); }, { passive: true });
    },

    _touch: function (e, down) {
      var t = e.touches[0]; if (!t) return;
      if (down) { this._tx = t.clientX; this._ty = t.clientY; }
      else {
        var dx = t.clientX - this._tx, dy = t.clientY - this._ty;
        this.cam.yaw += dx * 0.005;
        this.cam.pitch = M.clamp(this.cam.pitch - dy * 0.005, -1.2, 1.2);
        this._tx = t.clientX; this._ty = t.clientY;
      }
      this._askFrame();
    },

    _clearKeys: function () {
      for (var k in this.keys) this.keys[k] = false;
    },

    _wake: function () {
      this.idle = false;
      this._askFrame();
    },

    // ---- the hand -----------------------------------------------------------------------------
    _move: function (dt) {
      var speed = 1.55;
      var f = 0, r = 0;
      if (this.keys['w'] || this.keys['arrowup']) f += 1;
      if (this.keys['s'] || this.keys['arrowdown']) f -= 1;
      if (this.keys['a'] || this.keys['arrowleft']) r -= 1;
      if (this.keys['d'] || this.keys['arrowright']) r += 1;
      if (!f && !r) return false;
      var sy = M.sin(this.cam.yaw), cy = M.cos(this.cam.yaw);
      var p = this.cam.pos;
      p[0] += (sy * f + cy * r) * speed * dt;
      p[2] += (cy * f - sy * r) * speed * dt;
      p[0] = M.clamp(p[0], -2.30, 2.30);
      p[2] = M.clamp(p[2], -2.10, 2.10);
      // you step down into the lounge, so your eye steps down with you
      var eyeY = (p[2] > 1.05 ? 0.0 : -0.42) + 1.60;
      p[1] += (eyeY - p[1]) * Math.min(1, dt * 9);
      return true;
    },

    /* ---- the curtain: the Day-1 experiment ---------------------------------------------- */
    _toggleCurtain: function () {
      this.curtain = this.curtain > 0.5 ? 0.05 : 1.0;
      this._wake();
      this._toast(this.curtain > 0.5 ? 'curtain open' : 'curtain closed');
    },

    _toggleAC: function () {
      this.acOn = !this.acOn;
      this.model.hvacs[0].on = this.acOn;
      if (this.acOn) this.audio.clunk(1.0);
      this._wake();
      this._toast(this.acOn ? 'AC on' : 'AC off');
    },

    /* ---- the grease pencil (§6.1) --------------------------------------------------------- */
    _placeMark: function () {
      var T = this.model.nodes[0].T;
      // The pencil marks "where the needle will settle". Zero text: the mark is placed at the
      // current reading, and the player drags it with the wheel or the arrow keys before committing.
      // For a one-keystroke gesture we take the current value +/- the direction the room is heading.
      var drift = this.model.nodes[0].dTdt || 0;
      var guess = T + M.clamp(drift * 900, -3, 3);   // ~15 min of the current trend
      var m = this.ledger.mark({
        t: this.t, what: 'gauge', metric: 'room.T',
        value: guess,
        assumed: {
          ac: this.acOn ? 'full capacity, continuous' : 'off',
          curtain: this.curtain > 0.5 ? 'open' : 'closed',
          solar: 'constant'
        }
      });
      this.pendingMark = m;
      this._wake();
      this._toast('pencil: ' + (guess - 273.15).toFixed(1) + ' C   [' + m.hash + ']');
    },

    _resolveMark: function () {
      var m = this.pendingMark;
      if (!m) { this._toast('nothing marked'); return; }
      var actual = this.model.nodes[0].T;
      this.ledger.resolve(m, this.t, actual, { stakes: 0.4 });
      this.pendingMark = null;
      this._wake();
      this._toast('reality ' + (actual - 273.15).toFixed(1) + ' C   d ' + (m.delta).toFixed(2) + ' K');
    },

    /* ---- the epistemic peak (§9.5) -------------------------------------------------------- */
    _checkPeak: function () {
      var st = this.ledger.stats();
      if (!st) return;
      var pm = this.ledger.peakMark;
      // fire only once per mark, and only after a real amount of play
      if (pm && pm.peak > 0.9 && this.t - this.peakFired > 600 && this.t > 120) {
        this.peakFired = this.t;
        this.peakGlow = 1.0;
        this._toast('— golden hour —');
      }
      if (this.peakGlow > 0) this.peakGlow = Math.max(0, this.peakGlow - 0.004);
    },

    /* ---- history for the ghost twin --------------------------------------------------------- */
    _record: function () {
      var T = this.model.nodes[0].T;
      var h = this.history;
      if (!h.length || this.world.t - h[h.length - 1].t > 5) {
        h.push({ t: this.world.t, T: T, ghi: this.world.light().irr.ghi, ac: this.acOn });
        if (h.length > 720) h.shift();
      }
    },

    /* ---- draw ------------------------------------------------------------------------------ */
    _resize: function () {
      var w = innerWidth, h = innerHeight;
      // A ray-marched pixel is expensive, so the shaded resolution is deliberately below the CSS
      // size: the room is soft, and upscaling is free. This is the single biggest lever we have.
      var base = Math.min(devicePixelRatio || 1, 1.0);
      var dpr = base * this.quality;
      // hard cap the framebuffer: 1.3 megapixels is plenty for a room made of gradients
      var maxPx = 1.35e6;
      var px = w * h * dpr * dpr;
      if (px > maxPx) dpr *= Math.sqrt(maxPx / px);
      this.renderer.resize(w, h, dpr);
      this.hud.resize(w, h, Math.min(devicePixelRatio || 1, 2));
      this.dpr = dpr;
    },

    _camera: function () {
      var cy = M.cos(this.cam.yaw), sy = M.sin(this.cam.yaw);
      var cp = M.cos(this.cam.pitch), sp = M.sin(this.cam.pitch);
      var fwd = [sy * cp, sp, cy * cp];
      var right = [cy, 0, -sy];
      var up = [
        right[1] * fwd[2] - right[2] * fwd[1],
        right[2] * fwd[0] - right[0] * fwd[2],
        right[0] * fwd[1] - right[1] * fwd[0]
      ];
      // up = right x fwd gives -up; flip
      up = [-up[0], -up[1], -up[2]];
      return { pos: this.cam.pos, fwd: fwd, right: right, up: up, fov: 1.05 };
    },

    _audioState: function () {
      var ac = this.model.hvacs[0];
      return {
        acOn: this.acOn, acDuty: ac.duty || 0,
        cicada: this.world.light().sky.day * 0.13 * this.curtain,
        wind: 0.03 + 0.05 * this.world.light().sky.day
      };
    },

    _draw: function (ts) {
      var L = this.world.light();
      var sol = L.sol, sky = L.sky;
      var elevDeg = sol.elev / M.DEG;
      // ---- which way does the flat face? ------------------------------------------------------
      // This is the bug that made the room flat for two rounds. The shader puts the window in the
      // +z wall, and +z is *azimuth 0*, i.e. NORTH: at Tanegashima the sun is never in the north, so
      // sunVisible() was always false and the room was lit by sky fill alone. A real flat on a
      // summer evening has a west window, so the hab is oriented with +z facing 270 deg and the real
      // sun is rotated into room space. 16:26 in August then means a low golden sun almost straight
      // through the glazing, which is the whole reason for the room.
      var roomAz = 270 * M.DEG;
      var localAz = sol.az - roomAz;
      var sunDir = [
        M.sin(localAz) * M.cos(sol.elev),
        M.sin(sol.elev),
        M.cos(localAz) * M.cos(sol.elev)
      ];
      var warm = 1 - sky.gold;
      var sunCol = [
        1.0 * M.lerp(1.0, 1.25, warm),
        0.98 * M.lerp(1.0, 0.80, warm),
        0.92 * M.lerp(1.0, 0.55, warm)
      ];
      var ac = this.model.hvacs[0];
      var st = {
        res: [this.canvas.width, this.canvas.height],
        time: 0,   // the image must not depend on t, or a still room can never settle
        cam: this._camera(),
        sun: { dir: sunDir, col: sunCol, elev: elevDeg },
        sky: { zenith: sky.zenith, horizon: sky.horizon },
        day: sky.day, curtain: this.curtain,
        hour: this.world.hoursLocal(),
        peak: this.peakGlow, warmth: warm,
        lampOn: sky.day < 0.5 ? 1 : 0.25,
        lampCol: [1.0, 0.82, 0.60],
        screen: 0.35 + 0.65 * (1 - sky.day)
      };
      var drew = this.renderer.draw(st);
      // the instruments are redrawn every present, not every frame: they are part of the picture
      this.hud.g.clearRect(0, 0, this.hud.w, this.hud.h);
      this._drawHud(L, ac);
      return drew;
    },

    _drawHud: function (L, ac) {
      var h = this.hud, T = this.model.nodes[0].T - 273.15;
      var mark = this.pendingMark ? this.pendingMark.value - 273.15 : null;
      var cam = this._camera();
      // The two instruments are screwed to the walls, at real positions in the room:
      // the thermometer beside the window, the AC's ammeter on the wall screen.
      h.wallGauge('thermometer', {
        at: [2.33, 1.34, 1.55], cam: cam, value: T, mark: mark, ghost: null, label: 'ROOM'
      });
      h.wallGauge('meter', {
        at: [-2.33, 1.10, -0.55], cam: cam,
        value: ac.p * (ac.on ? 1 : 0.02), duty: ac.duty || 0
      });
      h.drawer({ x: this.hud.w - 300, y: 24, w: 276, text: 'DRAWER  1  ·  ' + (this.drawer[0] || '').slice(0, 34) });
      h.mirror(this.ledger.stats());
      // the crosshair fades in with the capture and out with the release
      var wantR = this.pointer.locked ? 1 : 0;
      this.reticle += (wantR - this.reticle) * 0.25;
      if (this.reticle > 0.01) h.reticle({ alpha: this.reticle });

      if (this.visor) {
        var self = this;
        h.visorPanel({
          title: this.visorPage === 'provenance' ? 'provenance of the room temperature' : 'what you assumed',
          draw: function (g) {
            if (self.visorPage === 'provenance') {
              h.provenance(self._provenanceTree());
            } else {
              h.ghostTwin(self._ghostSeries());
            }
          }
        });
      }
      // a small, honest performance read-out, because the budget is a promise
      if (document.getElementById('perf').checked) {
        var g = h.g;
        g.save();
        g.font = '10px ui-monospace, monospace'; g.textAlign = 'right';
        g.fillStyle = 'rgba(240,232,214,0.5)';
        g.fillText('fps ' + this.fps.toFixed(0) +
          '   present ' + this.presented + ' / skip ' + this.skipped +
          '   ' + this.canvas.width + 'x' + this.canvas.height +
          '   q ' + this.quality.toFixed(2), this.hud.w - 14, this.hud.h - 14);
        g.restore();
      }
    },

    rafRate: function () {
      if (this.idle) return '0 (idle)';
      return 'active';
    },

    /* ---- provenance of the number on the wall ---------------------------------------------- */
    _provenanceTree: function () {
      var self = this;
      var C = this.model.nodes[0].C;
      var UA = this.model.nodes[0].UAout;
      var Tout = P.catalog(this.model.Tout, 1.0, 'JMA Tanegashima AMeDAS 16:25 JST');
      var Qsol = P.derived('Q = G * A * SHGC * tau',
        function (G, A, shgc) { return G * A * shgc; },
        [
          P.catalog(this.world.light().irr.ghi, 25, 'clear-sky model, Kasten-Young airmass'),
          P.assumed(3.0, 'player', 'window area, m2'),
          P.assumed(0.60 * this.curtain, 'player', 'SHGC x curtain transmission')
        ]);
      var Qac = P.derived('Q_ac = HVAC capacity * duty',
        function (q, d) { return q * d; },
        [
          P.modeled(2000, 100, 'vapour-compression capacity'),
          P.assumed(this.acOn ? 1 : 0, 'player', 'switch position')
        ]);
      var Qloss = P.derived('Q_loss = UA (Tout - Tin)',
        function (UA_, dt) { return UA_ * dt; }, [P.modeled(UA, 6, 'room UA, W/K'), Tout]);
      var node = P.derived('dT/dt = (Q_sol + Q_ac + Q_loss) / C',
        function (a, b, c, cap) { return (a + b + c) / cap; },
        [Qsol, Qac, Qloss, P.modeled(C, 0.05 * C, 'masonry+air capacitance')]);
      return P.trace(node);
    },

    /* ---- the ghost twin of the room -------------------------------------------------------- */
    _ghostSeries: function () {
      var h = this.history;
      if (h.length < 3) return null;
      var t0 = h[0].t, t1 = h[h.length - 1].t;
      var vmin = Infinity, vmax = -Infinity;
      for (var i = 0; i < h.length; i++) { vmin = Math.min(vmin, h[i].T); vmax = Math.max(vmax, h[i].T); }
      // The player's assumed model: hold the initial temperature, i.e. "nothing will change".
      // Reality: the solver's answer. The gap between them is delta, drawn as two paths.
      var expected = h.map(function (p) { return { t: p.t, v: h[0].T }; });
      var actual = h.map(function (p) { return { t: p.t, v: p.T }; });
      var m = this.pendingMark || (this.ledger.marks.length ? this.ledger.marks[this.ledger.marks.length - 1] : null);
      var div = null;
      if (m && m.actual !== null) {
        div = {
          t: m.resolvedT || t1,
          cause: 'assumed: ' + Object.keys(m.assumed).map(function (k) { return k + '=' + m.assumed[k]; }).join(' ')
        };
      }
      return { series: { expected: expected, actual: actual, t0: t0, t1: t1 || t0 + 1, vmin: vmin - 0.2, vmax: vmax + 0.2, divergence: div } };
    },

    _toast: function (msg) {
      var el = document.getElementById('toast');
      if (!el) return;
      el.textContent = msg;
      el.classList.add('show');
      clearTimeout(this._toastT);
      var self = this;
      this._toastT = setTimeout(function () { el.classList.remove('show'); }, 2600);
    }
  };

  if (typeof module !== 'undefined' && module.exports) module.exports = App;
  global.ODApp = App;
})(typeof globalThis !== 'undefined' ? globalThis : this);
