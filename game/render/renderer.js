/* Odderon — renderer.
 *
 * Two draws for the whole room. No meshes, no textures, no shadow maps, no MSAA.
 * The camera's uniforms come from the kernel; when they have not changed and no event is pending,
 * NOTHING is scheduled (PERFORMANCE.md I3, I4). The painting freezes.
 */
(function (global) {
  'use strict';

  function Renderer(canvas, glsl) {
    this.canvas = canvas;
    this.glsl = glsl;
    var gl = canvas.getContext('webgl2', {
      antialias: false, alpha: false, depth: false, stencil: false,
      powerPreference: 'low-power', desynchronized: true, preserveDrawingBuffer: false
    });
    if (!gl) throw new Error('WebGL2 unavailable');
    this.gl = gl;
    this.prog = this._program(glsl.VERT, glsl.SCENE);
    this._initQuad();
    this.attrs = {};
    this.frames = 0;
    this.last = null;
    this._state = null;
  }

  Renderer.prototype._shader = function (type, src) {
    var gl = this.gl, s = gl.createShader(type);
    gl.shaderSource(s, src); gl.compileShader(s);
    if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) {
      var log = gl.getShaderInfoLog(s);
      throw new Error('shader compile: ' + log);
    }
    return s;
  };

  Renderer.prototype._program = function (vs, fs) {
    var gl = this.gl;
    var p = gl.createProgram();
    gl.attachShader(p, this._shader(gl.VERTEX_SHADER, vs));
    gl.attachShader(p, this._shader(gl.FRAGMENT_SHADER, fs));
    gl.linkProgram(p);
    if (!gl.getProgramParameter(p, gl.LINK_STATUS)) throw new Error('link: ' + gl.getProgramInfoLog(p));
    var n = gl.getProgramParameter(p, gl.ACTIVE_UNIFORMS);
    var map = {};
    for (var i = 0; i < n; i++) {
      var info = gl.getActiveUniform(p, i);
      map[info.name] = gl.getUniformLocation(p, info.name);
    }
    p._u = map;
    return p;
  };

  Renderer.prototype._initQuad = function () {
    var gl = this.gl;
    this.vao = gl.createVertexArray();
    gl.bindVertexArray(this.vao);
    var buf = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, buf);
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 3, -1, -1, 3]), gl.STATIC_DRAW);
    var loc = gl.getAttribLocation(this.prog, 'aPos');
    gl.enableVertexAttribArray(loc);
    gl.vertexAttribPointer(loc, 2, gl.FLOAT, false, 0, 0);
  };

  // resize only when the devicePixelRatio/size actually changed -- render-on-demand applies here too
  Renderer.prototype.resize = function (w, h, dpr) {
    var gl = this.gl;
    var cw = Math.max(1, Math.round(w * dpr)), ch = Math.max(1, Math.round(h * dpr));
    if (this.canvas.width === cw && this.canvas.height === ch) return false;
    this.canvas.width = cw; this.canvas.height = ch;
    gl.viewport(0, 0, cw, ch);
    return true;
  };

  Renderer.prototype._set = function (name, kind, v) {
    var loc = this.prog._u[name];
    if (!loc) return;
    var gl = this.gl;
    if (kind === 'f') gl.uniform1f(loc, v);
    else if (kind === 'v2') gl.uniform2f(loc, v[0], v[1]);
    else if (kind === 'v3') gl.uniform3f(loc, v[0], v[1], v[2]);
  };

  // s = { res, time, cam:{pos,fwd,right,up,fov}, sun:{dir,col,elev}, sky:{zenith,horizon},
  //       day, curtain, acOn, rate, hour, peak, warmth }
  Renderer.prototype.draw = function (s) {
    if (this._state && Renderer.same(this._state, s)) return false;   // nothing changed: no frame
    this._state = Renderer.clone(s);
    var gl = this.gl;
    gl.useProgram(this.prog);
    gl.bindVertexArray(this.vao);
    this._set('uRes', 'v2', [gl.drawingBufferWidth, gl.drawingBufferHeight]);
    this._set('uTime', 'f', s.time);
    this._set('uCamPos', 'v3', s.cam.pos);
    this._set('uCamFwd', 'v3', s.cam.fwd);
    this._set('uCamRight', 'v3', s.cam.right);
    this._set('uCamUp', 'v3', s.cam.up);
    this._set('uFov', 'f', s.cam.fov);
    this._set('uSunDir', 'v3', s.sun.dir);
    this._set('uSunCol', 'v3', s.sun.col);
    this._set('uSunElev', 'f', s.sun.elev);
    this._set('uSkyZenith', 'v3', s.sky.zenith);
    this._set('uSkyHorizon', 'v3', s.sky.horizon);
    this._set('uDay', 'f', s.day);
    this._set('uCurtain', 'f', s.curtain);
    this._set('uACOn', 'f', s.acOn);
    this._set('uRate', 'f', s.rate);
    this._set('uHour', 'f', s.hour);
    this._set('uPeak', 'f', s.peak);
    this._set('uWarmth', 'f', s.warmth);
    gl.drawArrays(gl.TRIANGLES, 0, 3);
    this.frames++;
    return true;
  };

  Renderer.clone = function (s) {
    return JSON.stringify(s, function (k, v) {
      return typeof v === 'number' ? Math.round(v * 1e4) / 1e4 : v;   // sub-0.1mm camera moves: skip
    });
  };
  Renderer.same = function (a, b) { return a === Renderer.clone(b); };

  global.ODRenderer = Renderer;
})(typeof globalThis !== 'undefined' ? globalThis : this);
