/* Odderon — the one thermal/fluid solver (docs/GAME-DESIGN-BIBLE.md §6.7).
 *
 * The apartment, the capsule, the lunar base and the O'Neill cylinder all run on this file. There is
 * no "apartment physics" and no "spaceship physics"; there is a resistive network with capacitance,
 * solved semi-implicitly, and everything in the game is a use of it.
 *
 * Units, always SI and always named in the field:
 *   T [K]  Q [W]  C [J/K]  UA [W/K]  m [kg]  q [W/m2]  A [m2]
 *
 * Time is seconds. There is no per-frame step: solve(t) is called only when t advances past the next
 * solver tick, and that cadence is set by the physics (thermal constants of ~minutes), not by vsync.
 */
(function (global) {
  'use strict';

  // ---- node kinds ----------------------------------------------------------------
  // A node is a lumped thermal mass. `T` is state. `shC` is the short-wave (solar) aperture.
  function node(o) {
    return {
      id: o.id,
      name: o.name,
      T: o.T,                       // K
      C: o.C,                       // J/K  (thermal capacitance)
      UAout: o.UAout,               // W/K  loss to the reference outdoor air temperature
      qOut: o.qOut || 0,            // W    fixed internal gain (fridge duty, standby loads)
      aper: o.aper || null,         // {A, shgc, tau} short-wave aperture
      window: !!o.window,
      mins: []
    };
  }

  // An HVAC unit: clamped heat pump with start-up inrush current.
  function hvac(o) {
    return {
      id: o.id, name: o.name,
      node: o.node,                 // index of the node it conditions
      cool: !!o.cool,               // true = removes heat
      qMax: o.qMax,                 // W of heat moved (at the coil)
      cop: o.cop || 3.0,            // coefficient of performance
      pInrush: o.pInrush || 0,      // W, for ~1 s at start
      pStandby: o.pStandby || 0,    // W
      setpoint: o.setpoint,         // K  -- absent means "no thermostat, runs when switched on"
      hyst: o.hyst || 0.4,          // K  -- real units cycle; they do not modulate perfectly
      on: false, tOn: -1e9, q: 0, p: 0, duty: 0
    };
  }

  function Model(o) {
    this.nodes = o.nodes;
    this.hvacs = o.hvacs || [];
    this.Tout = o.Tout || 300;      // reference outdoor air temperature, K
    this.G = o.G || 0;              // solar global horizontal irradiance, W/m2
    this.extra = [];                // callbacks(need: {Q: Float64Array}, dt) for custom loads
    this._nextTick = 0;
    this.tickHz = o.tickHz || 1;    // solver cadence, Hz  (physics, not frame rate)
    this.bus = o.bus || null;       // optional electrical bus to record power draws
    this.netW = 0;
  }

  // Does the climate control currently hold the node at its setpoint? Oversizing a unit makes it
  // saturate; an undersized unit loses the setpoint and that failure must be visible in the draw.
  Model.prototype.saturated = function (i) {
    var h = this.hvacs[i];
    if (!h || h.setpoint === undefined) return false;
    var T = this.nodes[h.node].T;
    return h.on && ((h.cool && T > h.setpoint + h.hyst * 0.5) || (!h.cool && T < h.setpoint - h.hyst * 0.5));
  };

  Model.prototype.link = function (from, to, UA, name) {
    // Conductance between two nodes, symmetric; stored once per direction for cheap summation.
    this.nodes[from].mins.push({ to: to, UA: UA, name: name || (this.nodes[from].name + '->' + this.nodes[to].name) });
    this.nodes[to].mins.push({ to: from, UA: UA, name: name || (this.nodes[to].name + '->' + this.nodes[from].name) });
  };

  // Short-wave gain into a node, W.  q*(A*shgc)*(1 - angle factor) -- angle handled by `incidence`.
  Model.prototype.solarW = function (n, inc) {
    if (!n.aper) return 0;
    return this.G * n.aper.A * n.aper.shgc * (inc === undefined ? 1 : inc);
  };

  // One solver step. SECONDS, and it is called with however much time passed -- never per frame.
  Model.prototype.step = function (dt) {
    var i, n, Q = new Float64Array(this.nodes.length);

    // 1. HVAC command: a first-order lag on the conditioned node, plus inrush bookkeeping.
    for (i = 0; i < this.hvacs.length; i++) {
      var h = this.hvacs[i];
      var Tnode = this.nodes[h.node].T;
      // Thermostat: bang-bang with hysteresis, exactly like a window unit. This is what makes the
      // electrical draw tell the story -- a real unit cycles, it does not sit at constant power.
      if (h.setpoint !== undefined) {
        var hi = h.setpoint + h.hyst * 0.5, lo = h.setpoint - h.hyst * 0.5;
        if (h.on && Tnode <= lo) { h.on = false; }
        else if (!h.on && Tnode >= hi) { h.on = true; h.tOn = 0; }
      }
      h.tOn += dt;
      if (h.on) {
        // moving heat at full capacity; sign by mode
        h.q = (h.cool ? -1 : 1) * h.qMax;
        var running = (dt + (h.tOn > 0 ? 1 : 0));
        var inrush = (running > 1.2) ? 0 : h.pInrush;        // ~1 s start-up draw
        h.p = h.qMax / h.cop + h.pStandby + (h.tOn > 0 ? inrush : 0);
      } else {
        h.q = 0;
        h.p = h.pStandby;
      }
      Q[h.node] += h.q;
      this.netW += h.p;
    }

    // 2. Conduction
    for (i = 0; i < this.nodes.length; i++) {
      n = this.nodes[i];
      for (var j = 0; j < n.mins.length; j++) {
        var m = n.mins[j];
        Q[i] += m.UA * (this.nodes[m.to].T - n.T);
      }
      Q[i] += n.UAout * (this.Tout - n.T);                  // loss to outdoor reference
      Q[i] += n.qOut;                                       // fixed internal gain
    }

    // 3. Short-wave (solar) -- applied as a separate named contribution so the debrief can show it
    for (i = 0; i < this.nodes.length; i++) {
      n = this.nodes[i];
      if (n.aper) n.qSolar = this.solarW(n, n.incidence);
      Q[i] += n.qSolar || 0;
    }

    // 4. External loads (a kettle, an oven, a life-support rack)
    var need = { Q: Q, dt: dt, model: this };
    for (i = 0; i < this.extra.length; i++) this.extra[i](need);

    // 5. Semi-implicit Euler. Stable for any dt up to C/UA, which for a room is minutes.
    for (i = 0; i < this.nodes.length; i++) {
      n = this.nodes[i];
      var dT = Q[i] / n.C;
      n.T += dT * dt;
      n.qNet = Q[i];
      n.dTdt = dT;
    }
    this._lastQ = Q;
    return Q;
  };

  // Advance to a target time by sub-stepping at the model's own cadence. This is the only entry
  // point the game uses: sim.advanceTo(t) -> model.advanceTo(t).
  Model.prototype.advanceTo = function (t) {
    var dtMax = 1 / this.tickHz;
    var n = Math.max(0, Math.ceil((t - this._nextTick) / dtMax));
    if (n > 100000) n = 100000;
    for (var i = 0; i < n; i++) this.step(dtMax);
    this._nextTick = Math.max(this._nextTick, t);
    return n;
  };

  /* ---- the apartment, defined entirely with the spacecraft vocabulary ---- */
  function apartment(seed) {
    var Rng = global.ODRng;
    // Room 25 m2, 2.5 m ceiling. Concrete+plaster walls, single-glazed 3 m2 south window.
    var room = node({ id: 0, name: 'room', T: 302.6, C: 2.42e6, UAout: 160, qOut: 120,
      aper: { A: 3.0, shgc: 0.60 }, window: true });
    // Thermal mass of the masonry, coupled to the air. Gives the room its multi-hour memory.
    var wall = node({ id: 1, name: 'wall mass', T: 301.8, C: 1.1e7, UAout: 0 });
    // Inside surface of the glazing, so the window can be seen to be hotter than the wall.
    var glass = node({ id: 2, name: 'glass', T: 306.0, C: 1.2e4, UAout: 0, window: true });

    var m = new Model({ nodes: [room, wall, glass], Tout: 305.15, G: 800, tickHz: 1 });
    m.link(0, 1, 400, 'air->masonry');
    m.link(0, 2, 40, 'air->glass');
    m.link(2, 1, 9, 'glass->frame');

    // The window AC: a real vapour-compression unit, 2.0 kW of cooling at COP 3, 700 W inrush.
    var ac = hvac({ id: 0, name: 'window AC', node: 0, cool: true, qMax: 2000, cop: 3.0,
      pInrush: 700, pStandby: 2.0 });
    m.hvacs.push(ac);

    m.apartment = { room: room, wall: wall, glass: glass, ac: ac, curtain: 1.0 };
    return m;
  }

  var T = { Model: Model, node: node, hvac: hvac, apartment: apartment };

  if (typeof module !== 'undefined' && module.exports) module.exports = T;
  global.ODThermo = T;
})(typeof globalThis !== 'undefined' ? globalThis : this);
