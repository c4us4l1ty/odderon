/* Odderon — the instruments.
 *
 * There is almost no HUD. What exists is drawn as *objects on glass and on paper*:
 *   - a thermometer on the wall you can read (and mark)
 *   - a wall meter with a needle for the AC's draw (and mark)
 *   - the grease pencil itself, and the yellow marks accumulating on both
 *   - the drawer: the anomaly you will think about tomorrow
 *   - the visor: the only "UI" surface in the game, and it is a thing on your face
 *
 * The visor is where provenance and the ghost twin live. Nothing else is a screen.
 */
(function (global) {
  'use strict';

  var INK = '#c8452b',   // gauge red
      PAPER = '#efe7d8',
      GRAPH = '#6d6a63',
      MARKER = '#e8b923',
      AMBER = '#f0c46a',
      COOL = '#8fb8d6';

  function Hud(canvas) {
    this.c = canvas;
    this.g = canvas.getContext('2d');
    this.dpr = 1;
    this.visor = false;
    this.visorPage = 'provenance';
  }

  Hud.prototype.resize = function (w, h, dpr) {
    this.dpr = dpr;
    this.c.width = Math.round(w * dpr);
    this.c.height = Math.round(h * dpr);
    this.c.style.width = w + 'px';
    this.c.style.height = h + 'px';
    this.g.setTransform(dpr, 0, 0, dpr, 0, 0);
    this.w = w; this.h = h;
  };

  /* ---------- the crosshair: a painted ring on the glass, only while the mouse is captured ------
   * Pointer lock hides the OS cursor, so the game owes the player a mark. Without it, looking around
   * feels like the camera is drifting on its own.
   */
  Hud.prototype.reticle = function (o) {
    var g = this.g;
    var x = this.w * 0.5, y = this.h * 0.5;
    g.save();
    g.globalAlpha = o.alpha;
    g.strokeStyle = 'rgba(248,240,226,0.75)'; g.lineWidth = 1.2;
    g.beginPath(); g.arc(x, y, 5.5, 0, 6.283); g.stroke();
    g.fillStyle = 'rgba(248,240,226,0.85)';
    g.beginPath(); g.arc(x, y, 1.1, 0, 6.283); g.fill();
    g.strokeStyle = 'rgba(20,18,15,0.35)'; g.lineWidth = 2.6;
    g.beginPath(); g.arc(x, y, 5.5, 0, 6.283); g.stroke();
    g.restore();
  };

  /* ---------- projection: an instrument is an object on a wall, not an overlay -------------------
   * The shader projects with rd = fwd + right*(uv.x*fov) + up*(uv.y*fov); this is the same camera,
   * inverted, so the gauge lands exactly on the plaster it is screwed to. Look away and it is gone,
   * which is the point: you turn your head to read it.
   */
  Hud.prototype.project = function (p, cam, w, h) {
    var v = [p[0] - cam.pos[0], p[1] - cam.pos[1], p[2] - cam.pos[2]];
    var dot = function (a, b) { return a[0] * b[0] + a[1] * b[1] + a[2] * b[2]; };
    var z = dot(v, cam.fwd);
    if (z <= 0.12) return null;
    var ux = dot(v, cam.right) / (z * cam.fov);
    var uy = dot(v, cam.up) / (z * cam.fov);
    return {
      x: w * 0.5 + ux * h,
      y: h * 0.5 - uy * h,
      // pixels per metre at this depth: a gauge of radius r metres draws r*perMetre px
      perMetre: (h * 0.5) / (z * cam.fov),
      z: z,
      facing: -z / Math.sqrt(dot(v, v))
    };
  };

  /* ---------- a gauge mounted on a wall ---------- */
  Hud.prototype.wallGauge = function (kind, o) {
    var pr = this.project(o.at, o.cam, this.w, this.h);
    if (!pr || pr.facing < 0.10) return;
    // fade at the edge of vision and when nearly edge-on, so it never looks pasted on
    var a = Math.min(1, (pr.facing - 0.10) / 0.35);
    if (a <= 0.02) return;
    var k = Math.max(0.35, Math.min(2.4, pr.perMetre * 0.16));
    var g = this.g;
    g.save();
    g.globalAlpha = a;
    g.translate(pr.x, pr.y);
    g.scale(k, k);
    if (kind === 'thermometer') this.thermometer({ x: 0, y: 0, value: o.value, mark: o.mark, ghost: o.ghost, label: o.label });
    else this.meter({ x: 0, y: 0, value: o.value, duty: o.duty });
    g.restore();
  };

  /* ---------- the wall thermometer, as an object ---------- */
  Hud.prototype.thermometer = function (o) {
    var g = this.g;
    g.save();
    g.translate(o.x, o.y);
    // brass case
    g.fillStyle = 'rgba(38,34,28,0.82)';
    g.beginPath(); g.arc(0, 0, 46, 0, 6.283); g.fill();
    g.strokeStyle = 'rgba(180,160,120,0.5)'; g.lineWidth = 2; g.stroke();
    // face
    g.fillStyle = PAPER;
    g.beginPath(); g.arc(0, 0, 40, 0, 6.283); g.fill();
    // scale arc: 18..32 C, 200 degrees
    var a0 = Math.PI * 0.78, a1 = Math.PI * 2.22;
    g.strokeStyle = GRAPH; g.lineWidth = 1.2;
    g.beginPath(); g.arc(0, 0, 33, a0, a1); g.stroke();
    for (var i = 0; i <= 14; i++) {
      var a = a0 + (a1 - a0) * (i / 14);
      var len = (i % 2 === 0) ? 7 : 4;
      g.beginPath();
      g.moveTo(Math.cos(a) * 33, Math.sin(a) * 33);
      g.lineTo(Math.cos(a) * (33 - len), Math.sin(a) * (33 - len));
      g.stroke();
    }
    g.fillStyle = INK; g.font = '8px ui-monospace, monospace'; g.textAlign = 'center';
    g.fillText('18', Math.cos(a0) * 26, Math.sin(a0) * 26 + 3);
    g.fillText('32', Math.cos(a1) * 26, Math.sin(a1) * 26 + 3);

    // grease-pencil mark: a yellow tick on the glass at the player's expectation
    if (o.mark !== null && o.mark !== undefined) {
      var am = a0 + (a1 - a0) * ((o.mark - 18) / 14);
      g.strokeStyle = 'rgba(232,185,35,0.95)'; g.lineWidth = 3.5; g.lineCap = 'round';
      g.beginPath();
      g.moveTo(Math.cos(am) * 30, Math.sin(am) * 30);
      g.lineTo(Math.cos(am) * 40, Math.sin(am) * 40);
      g.stroke();
      // and the ghost of what the model *assumed*, before reality answered
      if (o.ghost !== null && o.ghost !== undefined) {
        var ag = a0 + (a1 - a0) * ((o.ghost - 18) / 14);
        g.strokeStyle = 'rgba(232,185,35,0.35)'; g.lineWidth = 6;
        g.beginPath();
        g.moveTo(Math.cos(ag) * 31, Math.sin(ag) * 31);
        g.lineTo(Math.cos(ag) * 39, Math.sin(ag) * 39);
        g.stroke();
      }
    }

    // the needle: reality
    var an = a0 + (a1 - a0) * ((o.value - 18) / 14);
    g.strokeStyle = 'rgba(200,60,40,0.95)'; g.lineWidth = 2;
    g.beginPath(); g.moveTo(0, 0); g.lineTo(Math.cos(an) * 34, Math.sin(an) * 34); g.stroke();
    g.fillStyle = '#2b2620'; g.beginPath(); g.arc(0, 0, 3.4, 0, 6.283); g.fill();
    // the label is handwriting on a strip of tape, not a tooltip
    g.save(); g.rotate(-0.04);
    g.fillStyle = 'rgba(240,232,214,0.85)';
    g.fillRect(-26, 44, 52, 14);
    g.fillStyle = '#3a342b'; g.font = '9px ui-monospace, monospace'; g.textAlign = 'center';
    g.fillText(o.label || 'ROOM', 0, 54);
    g.restore();
    g.restore();
  };

  /* ---------- the wall meter: AC electrical draw ---------- */
  Hud.prototype.meter = function (o) {
    var g = this.g;
    g.save(); g.translate(o.x, o.y);
    g.fillStyle = 'rgba(38,34,28,0.82)';
    g.fillRect(-62, -34, 124, 72);
    g.strokeStyle = 'rgba(180,160,120,0.35)'; g.strokeRect(-62, -34, 124, 72);
    g.fillStyle = 'rgba(240,236,222,0.9)';
    g.fillRect(-55, -27, 110, 30);
    g.fillStyle = '#3a342b'; g.font = '8px ui-monospace, monospace'; g.textAlign = 'left';
    g.fillText('AC  W', -50, -17);
    g.textAlign = 'right';
    g.font = 'bold 13px ui-monospace, monospace';
    g.fillText(Math.round(o.value).toString(), 48, -7);
    // duty bar
    g.fillStyle = 'rgba(240,236,222,0.15)'; g.fillRect(-55, 8, 110, 6);
    g.fillStyle = o.value > 1 ? AMBER : '#4f4a42';
    g.fillRect(-55, 8, 110 * Math.max(0, Math.min(1, o.duty || 0)), 6);
    g.fillStyle = 'rgba(240,236,222,0.55)'; g.font = '7px ui-monospace, monospace'; g.textAlign = 'left';
    g.fillText('DUTY ' + Math.round((o.duty || 0) * 100) + '%', -55, 26);
    g.restore();
  };

  /* ---------- the drawer ---------- */
  Hud.prototype.drawer = function (o) {
    var g = this.g;
    g.save(); g.translate(o.x, o.y);
    g.fillStyle = 'rgba(30,26,21,0.55)';
    g.fillRect(0, 0, o.w, 22);
    g.fillStyle = 'rgba(232,185,35,0.9)';
    g.fillRect(0, 0, 3, 22);
    g.fillStyle = 'rgba(240,232,214,0.72)'; g.font = '10px ui-monospace, monospace'; g.textAlign = 'left';
    g.fillText(o.text || 'DRAWER  0', 10, 14);
    g.restore();
  };

  /* ---------- the visor ---------- */
  Hud.prototype.visorPanel = function (o) {
    var g = this.g;
    g.save();
    // the glass of the visor, tinted, with a vignette: it is an object, not an overlay
    var grd = g.createRadialGradient(this.w / 2, this.h / 2, this.h * 0.2, this.w / 2, this.h / 2, this.h * 0.95);
    grd.addColorStop(0, 'rgba(150,200,190,0.05)');
    grd.addColorStop(1, 'rgba(20,40,45,0.55)');
    g.fillStyle = grd; g.fillRect(0, 0, this.w, this.h);
    g.strokeStyle = 'rgba(160,220,210,0.22)'; g.lineWidth = 1;
    for (var i = 0; i < 4; i++) {
      g.beginPath();
      g.moveTo(0, this.h * (0.15 + i * 0.2));
      g.lineTo(this.w, this.h * (0.15 + i * 0.2));
      g.stroke();
    }
    g.fillStyle = 'rgba(190,240,230,0.85)';
    g.font = '11px ui-monospace, monospace'; g.textAlign = 'left';
    g.fillText('EIDOS  ·  ' + (o.title || ''), 24, 28);
    g.fillStyle = 'rgba(190,240,230,0.5)';
    g.fillText('[V] lift visor    [P] provenance    [G] ghost twin', 24, 44);
    g.restore();
    if (o.draw) o.draw(g, this);
  };

  /* ---------- provenance: the lineage, drawn as a tree ---------- */
  Hud.prototype.provenance = function (tree) {
    var g = this.g;
    var x0 = this.w * 0.12, y0 = this.h * 0.18, lh = 20, ind = 22;
    g.save();
    g.fillStyle = 'rgba(8,18,20,0.55)';
    g.fillRect(x0 - 18, y0 - 26, Math.min(this.w * 0.78, 620), Math.min(this.h * 0.7, 42 + tree.rows.length * lh));
    g.font = '11px ui-monospace, monospace'; g.textAlign = 'left';
    g.fillStyle = 'rgba(190,240,230,0.9)';
    g.fillText('PROVENANCE  ·  ' + tree.rows.length + ' nodes  ·  '
      + tree.assumed + ' assumed', x0, y0 - 8);
    for (var i = 0; i < tree.rows.length; i++) {
      var r = tree.rows[i];
      var y = y0 + 12 + i * lh;
      if (y > this.h - 20) break;
      g.fillStyle = r.assumed ? MARKER : 'rgba(160,200,215,0.75)';
      g.globalAlpha = r.assumed ? 1 : 0.85;
      g.fillText((r.assumed ? '* ' : '  ') + r.kind.toUpperCase().padEnd(9, ' ')
        + (typeof r.value === 'number' ? r.value.toPrecision(6) : r.value)
        + (r.sigma ? '  ±' + r.sigma.toPrecision(2) : '')
        + '  ' + (r.note || r.label || ''), x0 + r.depth * ind, y);
      g.globalAlpha = 1;
    }
    g.fillStyle = MARKER; g.font = '10px ui-monospace, monospace';
    g.fillText('yellow = you guessed it. white = the universe told us.', x0, y0 + 12 + Math.min(tree.rows.length, 18) * lh + 12);
    g.restore();
  };

  /* ---------- ghost twin: the divergence, as two divergent paths ---------- */
  Hud.prototype.ghostTwin = function (o) {
    var g = this.g;
    var w = Math.min(this.w * 0.62, 560), h = 150;
    var x = (this.w - w) / 2, y = this.h - h - 56;
    g.save();
    g.fillStyle = 'rgba(8,18,20,0.62)'; g.fillRect(x, y, w, h);
    g.strokeStyle = 'rgba(160,220,210,0.25)'; g.strokeRect(x, y, w, h);
    g.font = '10px ui-monospace, monospace'; g.textAlign = 'left';
    g.fillStyle = 'rgba(190,240,230,0.8)';
    g.fillText('WHAT YOU ASSUMED  vs  WHAT HAPPENED', x + 12, y + 18);
    if (!series) {
      g.fillStyle = 'rgba(190,240,230,0.45)';
      g.fillText('no history yet — let the room run for a minute.', x + 12, y + 44);
      g.restore();
      return;
    }

    var series = o && o.series;   // { expected: [{t,v}], actual: [{t,v}], t0, t1, vmin, vmax }
    if (series) {
      var px = function (t) { return x + 24 + (w - 48) * ((t - series.t0) / (series.t1 - series.t0 || 1)); };
      var py = function (v) { return y + h - 22 - (h - 52) * ((v - series.vmin) / (series.vmax - series.vmin || 1)); };
      // the ghost: dashed amber, the player's model
      g.setLineDash([5, 4]); g.strokeStyle = 'rgba(232,185,35,0.9)'; g.lineWidth = 1.6;
      g.beginPath();
      for (var i = 0; i < series.expected.length; i++) {
        var p = series.expected[i];
        i ? g.lineTo(px(p.t), py(p.v)) : g.moveTo(px(p.t), py(p.v));
      }
      g.stroke(); g.setLineDash([]);
      // reality: solid
      g.strokeStyle = COOL; g.lineWidth = 1.8;
      g.beginPath();
      for (var j = 0; j < series.actual.length; j++) {
        var q = series.actual[j];
        j ? g.lineTo(px(q.t), py(q.v)) : g.moveTo(px(q.t), py(q.v));
      }
      g.stroke();
      // the divergence point, and ONE arrow: the largest contributor, on the hardware
      if (series.divergence !== null && series.divergence !== undefined) {
        var dx = px(series.divergence.t);
        g.strokeStyle = 'rgba(232,185,35,0.55)'; g.lineWidth = 1;
        g.beginPath(); g.moveTo(dx, y + 26); g.lineTo(dx, y + h - 22); g.stroke();
        g.fillStyle = 'rgba(240,232,214,0.92)'; g.font = '10px ui-monospace, monospace';
        g.fillText(series.divergence.cause || '', x + 14, y + h - 6);
      }
    }
    g.restore();
  };

  // the mirror: one honest sentence about the player, computed from their own marks
  Hud.prototype.mirror = function (stats) {
    if (!stats) return;
    var g = this.g;
    g.save();
    g.font = '10px ui-monospace, monospace'; g.textAlign = 'left';
    g.fillStyle = 'rgba(240,232,214,0.55)';
    g.fillText('n=' + stats.n + '   mean err ' + stats.meanErr.toFixed(2)
      + '   bias ' + (stats.bias >= 0 ? '+' : '') + stats.bias.toFixed(2), 16, this.h - 14);
    g.restore();
  };

  global.ODHud = Hud;
})(typeof globalThis !== 'undefined' ? globalThis : this);
