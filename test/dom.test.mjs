/* Headless smoke test of the actual page.
 *
 * There is no browser in this environment, so this stubs just enough DOM + WebGL2 to execute every
 * script in index.html in order and then run ~600 frames of the real app. It catches: syntax errors,
 * missing globals, shader-source assembly mistakes, and any exception on the hot path.
 * It does NOT validate rendering (that needs a GPU) -- the shaders are compiled in the browser, and
 * the frame budget is measured there.
 */
import { suite, ok } from './harness.mjs';
import { readFileSync } from 'fs';
import vm from 'vm';

function makeGL() {
  const calls = { draws: 0, programs: 0, uniformSets: 0 };
  const gl = {
    TRIANGLES: 4, FLOAT: 5126, ARRAY_BUFFER: 34962, STATIC_DRAW: 35044,
    VERTEX_SHADER: 35633, FRAGMENT_SHADER: 35632,
    COMPILE_STATUS: 35713, LINK_STATUS: 35714, ACTIVE_UNIFORMS: 35718,
    drawingBufferWidth: 1280, drawingBufferHeight: 720,
    createShader: () => ({}), shaderSource: () => {}, compileShader: () => {},
    getShaderParameter: () => true, getShaderInfoLog: () => '',
    createProgram: () => { calls.programs++; return {}; },
    attachShader: () => {}, linkProgram: () => {},
    getProgramParameter: (p, k) => (k === 35718 ? 12 : true),
    getProgramInfoLog: () => '',
    getActiveUniform: (p, i) => ({ name: ['uRes', 'uTime', 'uCamPos', 'uCamFwd', 'uCamRight', 'uCamUp',
      'uFov', 'uSunDir', 'uSunCol', 'uSkyZenith', 'uSkyHorizon', 'uDay'][i] || 'uX' }),
    getUniformLocation: (p, n) => ({ name: n }),
    useProgram: () => {}, createVertexArray: () => ({}), bindVertexArray: () => {},
    createBuffer: () => ({}), bindBuffer: () => {}, bufferData: () => {},
    getAttribLocation: () => 0, enableVertexAttribArray: () => {}, vertexAttribPointer: () => {},
    viewport: () => {}, uniform1f: () => { calls.uniformSets++; },
    uniform2f: () => { calls.uniformSets++; }, uniform3f: () => { calls.uniformSets++; },
    drawArrays: () => { calls.draws++; }
  };
  return { gl, calls };
}

function makeCtx2d() {
  const noop = () => {};
  return new Proxy({
    setTransform: noop, clearRect: noop, save: noop, restore: noop, translate: noop, rotate: noop,
    beginPath: noop, arc: noop, fill: noop, stroke: noop, moveTo: noop, lineTo: noop,
    fillRect: noop, strokeRect: noop, fillText: noop, setLineDash: noop,
    createRadialGradient: () => ({ addColorStop: noop }),
    measureText: () => ({ width: 10 })
  }, { get: (t, k) => (k in t ? t[k] : noop), set: () => true });
}

function makeEnv() {
  const { gl, calls } = makeGL();
  const listeners = {};
  const rafQ = [];
  let now = 0;
  const el = (tag) => ({
    tagName: tag, style: {}, width: 1280, height: 720, textContent: '', className: '',
    checked: false, classList: { add: () => {}, remove: () => {}, toggle: () => {} },
    addEventListener: () => {}, requestPointerLock: () => {},
    getContext: (kind) => (kind === 'webgl2' ? gl : makeCtx2d())
  });
  const doc = {
    pointerLockElement: null,
    getElementById: (id) => {
      if (!doc._els) doc._els = {};
      if (!doc._els[id]) doc._els[id] = el(id);
      return doc._els[id];
    },
    addEventListener: (t, f) => { (listeners['d:' + t] = listeners['d:' + t] || []).push(f); }
  };
  const win = {
    innerWidth: 1280, innerHeight: 720, devicePixelRatio: 1,
    addEventListener: (t, f) => { (listeners[t] = listeners[t] || []).push(f); },
    removeEventListener: () => {},
    requestAnimationFrame: (f) => { rafQ.push(f); return rafQ.length; },
    cancelAnimationFrame: () => {},
    setTimeout: (f, ms) => setTimeout(f, 0),
    clearTimeout: (id) => clearTimeout(id),
    performance: { now: () => now },
    location: { hash: '', href: 'http://localhost/' },
    document: doc,
    console: console,
    AudioContext: undefined,      // audio must degrade gracefully when absent
    Float64Array, Uint32Array, Int32Array, Float32Array, Math, JSON, Date, Proxy, Error, Object, Array,
    isFinite, isNaN, parseInt, parseFloat
  };
  win.window = win;
  win.globalThis = win;
  win.self = win;
  return {
    win, listeners, rafQ, calls,
    step: (frames) => {
      for (let i = 0; i < frames; i++) {
        const f = rafQ.shift();
        if (!f) break;
        now += 16.7;
        f(now);
      }
    },
    advance: (ms) => { now += ms; }
  };
}

suite('the page boots and survives 600 real frames (headless)');
{
  const html = readFileSync(new URL('../index.html', import.meta.url), 'utf8');
  const scripts = [...html.matchAll(/<script src="([^"]+)"><\/script>/g)].map(m => m[1]);
  ok(scripts.length >= 10, 'index.html loads ' + scripts.length + ' scripts in order');

  const env = makeEnv();
  const ctx = vm.createContext(env.win);
  let bootError = null;
  for (const s of scripts) {
    const code = readFileSync(new URL('../' + s, import.meta.url), 'utf8');
    try { vm.runInContext(code, ctx, { filename: s }); }
    catch (e) { bootError = s + ': ' + e.message; break; }
  }
  ok(!bootError, 'every script parses and evaluates', bootError);
  ok(env.win.ODApp && typeof env.win.ODApp.boot === 'function', 'the app object is present');

  let runError = null;
  try {
    env.win.ODApp.boot();
    env.step(600);          // ~10 s of game time, so the ghost twin has a history
    // simulate input: close the curtain, toggle the AC, place and resolve a mark
    const kd = env.listeners.keydown || [];
    const fire = (key) => kd.forEach(f => f({ key, preventDefault() {} }));
    fire('e'); env.step(5);
    fire('f'); env.step(60);
    fire('m'); env.step(30);
    fire('r'); env.step(30);
    fire('v'); env.step(10);
    fire('p'); env.step(10);
    fire('g'); env.step(10);
    const kd2 = env.listeners.keydown || [];
    kd2.forEach(f => f({ key: 'w', preventDefault() {} }));
    env.step(300);
  } catch (e) {
    runError = e && (e.stack || e.message);
  }
  ok(!runError, 'the app runs 600 frames with input without throwing', runError);

  const app = env.win.ODApp;
  ok(env.calls.programs >= 1, 'a WebGL2 program was created (both shaders linked)');
  ok(env.calls.draws > 0, 'frames were drawn (' + env.calls.draws + ' draws)');
  ok(env.calls.uniformSets > 0, 'uniforms were uploaded from the kernel to the shader');
  ok(app.ledger.marks.length >= 1, 'the grease pencil created a mark');
  ok(app.ledger.marks[0].actual !== null, 'and reality resolved it (delta was measured)');
  ok(typeof app.ledger.marks[0].delta === 'number', 'delta is a number: ' + app.ledger.marks[0].delta);
  ok(app.curtain < 0.5, 'the curtain was closed by the E key');
  ok(app.model.nodes[0].T > 250, 'the solver stayed numerically sane over 600 frames');
  ok(isFinite(app.model.nodes[0].T), 'no NaN leaked into the room temperature');

  // the shader source must contain no unresolved template artefacts
  const src = env.win.ODGlsl.SCENE;
  ok(src.indexOf('${') < 0, 'the shader source has no unsubstituted template literals');
  ok(src.indexOf('#version 300 es') === 0, 'the shader declares GLSL ES 3.00 first, as WebGL2 requires');
  const declares = (src.match(/\buniform\b/g) || []).length;
  ok(declares >= 15, 'the scene shader declares ' + declares + ' uniforms');
}
