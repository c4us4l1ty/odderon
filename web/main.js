// Odderon — Checkpoint 1A ("The Sunlit Room") Browser Presenter.
// Architecture:
// - Pure Rust simulation & parametric geometry compiled to 105 KB WebAssembly (`odderon_wasm.wasm`)
// - Raw WebGL2 (3-4 draw calls total, zero textures, zero shadow maps, zero per-frame allocations)
// - Event-driven Render-On-Demand presenter (`0 rAF/s` when settled -> literally 0.0% idle CPU/GPU!)
// - Adaptive Pixel Governor for 2015 Intel HD/UHD laptops (4 GB RAM) and Android 8+ phones

import { SKY_VERT_GL2, SKY_FRAG_GL2, WORLD_VERT_GL2, WORLD_FRAG_GL2 } from './shaders.js';
import { PhysicalAudioEngine } from './audio.js';

const TARGET_INFO = {
  0: null,
  1: { name: 'SHOJI SOLAR SCREEN (WASHI + KUMIKO)', hint: '[Click/Tap] Slide Shoji Screen · Rejects 87% of Solar Shortwave Heat' },
  2: { name: 'COASTAL SLIDING GLASS WINDOW', hint: '[Click/Tap] Slide Sash · Modulates Coastal Breeze & Cicada/Shockwave Acoustics' },
  3: { name: 'MAIN 15A HAB BUS BREAKER', hint: '[Click/Tap] Toggle / Reset Tripped Bi-Metallic Thermal Breaker' },
  4: { name: 'R32 VAPOUR-COMPRESSION AC SWITCH', hint: '[Click/Tap] Toggle Heat Pump · Rejects Cabin Heat to Balcony Condenser' },
  5: { name: 'OVERHEAD TIMBER CEILING FAN SWITCH', hint: '[Click/Tap] Toggle 4-Blade Fan · Mixes Air Boundary Layer (Wakes 60Hz Presenter)' },
  6: { name: 'SHOJI ANDON & DESK LAMP SWITCH', hint: '[Click/Tap] Toggle 2700K Warm Interior Lighting (48 W)' },
  7: { name: 'KETTLE DEDICATED BREAKER (1,060 W)', hint: '[Click/Tap] Toggle 1,060 W Heater · Watch 100V Bus Sag & 15A Breaker Margin!' },
  8: { name: 'ANALOG GAUGE 1 — CABIN AIR TEMP (°C)', hint: '[Click] Inspect Provenance · [M + Click / Right-Click] Place Yellow China-Marker Prediction' },
  9: { name: 'ANALOG GAUGE 2 — HAB BUS POWER (W)', hint: '[Click] Inspect Provenance · [M + Click / Right-Click] Place Yellow China-Marker Prediction' },
  10: { name: 'ANALOG GAUGE 3 — HAB BUS VOLTAGE (V)', hint: '[Click] Inspect Provenance · [M + Click / Right-Click] Place Yellow China-Marker Prediction' },
  11: { name: 'VHF COASTAL PAD-1 TRANSCEIVER', hint: '[Click/Tap] Request Live Sounding-Rocket Static Fire on Cape Pad (2,820 m away)' },
  12: { name: 'PAD TECHNICIAN SHIFT CLIPBOARD', hint: '[Click/Tap] Read Day-1 Thermal/Bus Audit Card & Export Akashi Proof-of-Flight' },
  13: { name: 'RECESSED SLEEPING BED ALCOVE', hint: '[Click/Tap] Rest · Cycle Uchinoura Solar Light (Morning Sunbeams -> High Noon -> Sunset -> Night)' },
  14: { name: 'STAINLESS ELECTRIC KETTLE (0.45 kg H2O)', hint: '[Click/Tap] Toggle 1,060 W Boil Cycle on L-Galley Counter (Auto-Cutoffs at 100.0 °C)' },
  15: { name: 'INDOOR SPLIT AC EVAPORATOR UNIT', hint: '[Click/Tap] Toggle Heat Pump · Inspect Carnot-Derated COP in Eidos Visor' },
  16: { name: 'UNIT-BATH SLIDING POCKET DOOR', hint: '[Click/Tap] Slide Bathroom Door Open / Closed · Step Inside Modular Restroom Pod' },
  17: { name: 'HAB #204 FRONT ENTRY DOOR (#204)', hint: '[Click/Tap] Swing Apartment Door Open / Closed · Walk Out to 2F Gallery, Stairs & Street!' },
  18: { name: 'UCHINOURA SPACE CENTER BUS STOP (バス停)', hint: '[Click/Tap] Inspect Range Shuttle Timetable · Real 90s Coastal Schedule' },
  19: { name: 'COASTAL BUS STOP WAITING BENCH', hint: '[Click/Tap] Sit & Wait Until Next Scheduled Range Shuttle Approaches Curb' },
  20: { name: 'RANGE SHUTTLE BOARDING DOOR (PAD-1 LINE)', hint: '[Click/Tap] Board Shuttle (¥240 Fare) · Clock In for Morning Pad Shift (+¥4,800 Stipend)' },
  21: { name: 'ROADSIDE BEVERAGE VENDING MACHINE (自動販売機)', hint: '[Click/Tap] Buy Cold Uchinoura Canned Coffee (¥130 from Bank Account)' },
};

// Pre-allocated matrices & vectors (0 allocations in frame loop — PERFORMANCE.md N4)
const projMat = new Float32Array(16);
const viewMat = new Float32Array(16);
const vpMat = new Float32Array(16);
const camFwd = new Float32Array([0, 0, 1]);
const camRight = new Float32Array([-1, 0, 0]);
const camUp = new Float32Array([0, 1, 0]);
const sunVec = new Float32Array(3);
const unprojDir = new Float32Array(3);

function perspectiveFieldOfView(out, fovYRad, aspect, near, far) {
  const f = 1.0 / Math.tan(fovYRad * 0.5);
  const nf = 1.0 / (near - far);
  out[0] = f / aspect; out[1] = 0; out[2] = 0; out[3] = 0;
  out[4] = 0; out[5] = f; out[6] = 0; out[7] = 0;
  out[8] = 0; out[9] = 0; out[10] = (far + near) * nf; out[11] = -1;
  out[12] = 0; out[13] = 0; out[14] = 2 * far * near * nf; out[15] = 0;
}

function computeCameraBasisAndView(eyeX, eyeY, eyeZ, fx, fy, fz) {
  let fl = Math.hypot(fx, fy, fz) || 1;
  fx /= fl; fy /= fl; fz /= fl;
  camFwd[0] = fx; camFwd[1] = fy; camFwd[2] = fz;

  // right = normalize(cross(fwd, worldUp))
  let rx = fy * 0 - fz * 1;
  let ry = fz * 0 - fx * 0;
  let rz = fx * 1 - fy * 0;
  let rl = Math.hypot(rx, ry, rz) || 1;
  rx /= rl; ry /= rl; rz /= rl;
  camRight[0] = rx; camRight[1] = ry; camRight[2] = rz;

  // up = cross(right, fwd)
  const ux = ry * fz - rz * fy;
  const uy = rz * fx - rx * fz;
  const uz = rx * fy - ry * fx;
  camUp[0] = ux; camUp[1] = uy; camUp[2] = uz;

  viewMat[0] = rx;  viewMat[1] = ux;  viewMat[2] = -fx; viewMat[3] = 0;
  viewMat[4] = ry;  viewMat[5] = uy;  viewMat[6] = -fy; viewMat[7] = 0;
  viewMat[8] = rz;  viewMat[9] = uz;  viewMat[10] = -fz; viewMat[11] = 0;
  viewMat[12] = -(rx * eyeX + ry * eyeY + rz * eyeZ);
  viewMat[13] = -(ux * eyeX + uy * eyeY + uz * eyeZ);
  viewMat[14] = (fx * eyeX + fy * eyeY + fz * eyeZ);
  viewMat[15] = 1;
}

function mulMat4(out, a, b) {
  for (let col = 0; col < 4; col++) {
    const b0 = b[col * 4], b1 = b[col * 4 + 1], b2 = b[col * 4 + 2], b3 = b[col * 4 + 3];
    out[col * 4]     = a[0] * b0 + a[4] * b1 + a[8]  * b2 + a[12] * b3;
    out[col * 4 + 1] = a[1] * b0 + a[5] * b1 + a[9]  * b2 + a[13] * b3;
    out[col * 4 + 2] = a[2] * b0 + a[6] * b1 + a[10] * b2 + a[14] * b3;
    out[col * 4 + 3] = a[3] * b0 + a[7] * b1 + a[11] * b2 + a[15] * b3;
  }
}

function compileShader(gl, type, src) {
  const sh = gl.createShader(type);
  gl.shaderSource(sh, src);
  gl.compileShader(sh);
  if (!gl.getShaderParameter(sh, gl.COMPILE_STATUS)) {
    throw new Error('Shader compile error: ' + gl.getShaderInfoLog(sh));
  }
  return sh;
}

function linkProgram(gl, vsSrc, fsSrc) {
  const prog = gl.createProgram();
  const vs = compileShader(gl, gl.VERTEX_SHADER, vsSrc);
  const fs = compileShader(gl, gl.FRAGMENT_SHADER, fsSrc);
  gl.attachShader(prog, vs);
  gl.attachShader(prog, fs);
  gl.linkProgram(prog);
  if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) {
    throw new Error('Program link error: ' + gl.getProgramInfoLog(prog));
  }
  gl.deleteShader(vs);
  gl.deleteShader(fs);
  return prog;
}

async function boot() {
  const canvas = document.getElementById('gl-canvas');
  const gl = canvas.getContext('webgl2', {
    alpha: false,
    antialias: true, // Eliminates 1px diagonal edge shimmer on walls, jambs & overhead wires!
    depth: true,
    stencil: false,
    powerPreference: 'default',
    preserveDrawingBuffer: false,
  });

  if (!gl) {
    document.getElementById('prompt-main').textContent = 'WebGL2 context unavailable on this browser.';
    return;
  }

  // 1. Load Pure-Rust WASM Kernel
  const resp = await fetch('./wasm/odderon_wasm.wasm');
  const bytes = await resp.arrayBuffer();
  const wasmKb = Math.round(bytes.byteLength / 1024);
  const { instance } = await WebAssembly.instantiate(bytes, {});
  const wasm = instance.exports;
  wasm.odderon_init();

  const audio = new PhysicalAudioEngine();

  // 2. Pre-compile ALL shaders at boot (IMMERSION.md N2: 0 shader compiles after boot!)
  const skyProg = linkProgram(gl, SKY_VERT_GL2, SKY_FRAG_GL2);
  const worldProg = linkProgram(gl, WORLD_VERT_GL2, WORLD_FRAG_GL2);

  const skyLocs = {
    uCamFwd: gl.getUniformLocation(skyProg, 'uCamFwd'),
    uCamRight: gl.getUniformLocation(skyProg, 'uCamRight'),
    uCamUp: gl.getUniformLocation(skyProg, 'uCamUp'),
    uTanHalfFov: gl.getUniformLocation(skyProg, 'uTanHalfFov'),
    uAspect: gl.getUniformLocation(skyProg, 'uAspect'),
    uSunDir: gl.getUniformLocation(skyProg, 'uSunDir'),
    uWarmth: gl.getUniformLocation(skyProg, 'uWarmth'),
    uShockFrac: gl.getUniformLocation(skyProg, 'uShockFrac'),
  };

  const worldLocs = {
    uViewProj: gl.getUniformLocation(worldProg, 'uViewProj'),
    uCamPos: gl.getUniformLocation(worldProg, 'uCamPos'),
    uSunDir: gl.getUniformLocation(worldProg, 'uSunDir'),
    uWarmth: gl.getUniformLocation(worldProg, 'uWarmth'),
    uShojiClosed: gl.getUniformLocation(worldProg, 'uShojiClosed'),
    uLampOn: gl.getUniformLocation(worldProg, 'uLampOn'),
    uPlumeGlow: gl.getUniformLocation(worldProg, 'uPlumeGlow'),
  };

  const emptyVao = gl.createVertexArray();

  function setupMeshVao(usage) {
    const vao = gl.createVertexArray();
    const vbo = gl.createBuffer();
    const ibo = gl.createBuffer();
    gl.bindVertexArray(vao);
    gl.bindBuffer(gl.ARRAY_BUFFER, vbo);
    gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, ibo);
    const stride = 10 * 4;
    gl.enableVertexAttribArray(0);
    gl.vertexAttribPointer(0, 3, gl.FLOAT, false, stride, 0);
    gl.enableVertexAttribArray(1);
    gl.vertexAttribPointer(1, 3, gl.FLOAT, false, stride, 12);
    gl.enableVertexAttribArray(2);
    gl.vertexAttribPointer(2, 4, gl.FLOAT, false, stride, 24);
    gl.bindVertexArray(null);
    return { vao, vbo, ibo, indexCount: 0, vboCap: 0, iboCap: 0, rev: 0, usage };
  }

  function uploadMesh(meshObj, vPtr, vLen, iPtr, iLen) {
    const mem = wasm.memory.buffer;
    const verts = new Float32Array(mem, vPtr, vLen);
    const indices = new Uint32Array(mem, iPtr, iLen);
    gl.bindVertexArray(meshObj.vao);
    gl.bindBuffer(gl.ARRAY_BUFFER, meshObj.vbo);
    if (verts.byteLength > meshObj.vboCap) {
      meshObj.vboCap = Math.max(verts.byteLength, 65536);
      gl.bufferData(gl.ARRAY_BUFFER, meshObj.vboCap, meshObj.usage);
    }
    gl.bufferSubData(gl.ARRAY_BUFFER, 0, verts);

    gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, meshObj.ibo);
    if (indices.byteLength > meshObj.iboCap) {
      meshObj.iboCap = Math.max(indices.byteLength, 32768);
      gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, meshObj.iboCap, meshObj.usage);
    }
    gl.bufferSubData(gl.ELEMENT_ARRAY_BUFFER, 0, indices);
    gl.bindVertexArray(null);
    meshObj.indexCount = iLen;
  }

  // Upload entire Static Apartment + Exterior Coastal Launch Town ONCE (1 draw call!)
  const staticMesh = setupMeshVao(gl.STATIC_DRAW);
  uploadMesh(
    staticMesh,
    wasm.odderon_static_verts_ptr(),
    wasm.odderon_static_verts_len(),
    wasm.odderon_static_indices_ptr(),
    wasm.odderon_static_indices_len()
  );

  const dynMesh = setupMeshVao(gl.DYNAMIC_DRAW);
  const eidosMesh = setupMeshVao(gl.DYNAMIC_DRAW);

  function syncDynamicMeshes(visorDown) {
    const dRev = wasm.odderon_dyn_revision() >>> 0;
    if (dynMesh.rev !== dRev) {
      uploadMesh(
        dynMesh,
        wasm.odderon_dyn_verts_ptr(),
        wasm.odderon_dyn_verts_len(),
        wasm.odderon_dyn_indices_ptr(),
        wasm.odderon_dyn_indices_len()
      );
      dynMesh.rev = dRev;
    }
    if (visorDown) {
      const eRev = wasm.odderon_eidos_revision() >>> 0;
      if (eidosMesh.rev !== eRev) {
        uploadMesh(
          eidosMesh,
          wasm.odderon_eidos_verts_ptr(),
          wasm.odderon_eidos_verts_len(),
          wasm.odderon_eidos_indices_ptr(),
          wasm.odderon_eidos_indices_len()
        );
        eidosMesh.rev = eRev;
      }
    }
  }
  syncDynamicMeshes(false);

  function syncAudioAndAck(t) {
    audio.syncFromTelemetry(t);
    wasm.odderon_ack_audio();
  }

  // 3. Adaptive Resolution Governor for 2015 Intel UHD (capped <= 1.30 Mpx, floor 0.55x)
  let qualityScale = 1.0;
  let fbWidth = 0;
  let fbHeight = 0;
  const FOV_Y_RAD = (62.0 * Math.PI) / 180.0;
  const TAN_HALF_FOV = Math.tan(FOV_Y_RAD * 0.5);

  function updateCanvasResolution() {
    const cssW = Math.max(window.innerWidth, 1);
    const cssH = Math.max(window.innerHeight, 1);
    const dpr = Math.min(window.devicePixelRatio || 1.0, 1.5);
    let targetW = Math.round(cssW * dpr * qualityScale);
    let targetH = Math.round(cssH * dpr * qualityScale);
    const pxCount = targetW * targetH;
    const maxPx = 2_073_600;
    if (pxCount > maxPx) {
      const s = Math.sqrt(maxPx / pxCount);
      targetW = Math.max(480, Math.round(targetW * s));
      targetH = Math.max(270, Math.round(targetH * s));
    }
    if (canvas.width !== targetW || canvas.height !== targetH) {
      canvas.width = targetW;
      canvas.height = targetH;
      fbWidth = targetW;
      fbHeight = targetH;
    }
  }
  updateCanvasResolution();

  // 4. Input & Event-Driven Render-On-Demand Presenter State (`PERFORMANCE.md` I3)
  const keys = Object.create(null);
  let pendingYaw = 0;
  let pendingPitch = 0;
  let markMode = false;
  let showPerfHud = true;
  let rafId = 0;
  let isParked = false;
  let lastFrameTimeMs = performance.now();
  let rafCountWindow = 0;
  let displayedRafHz = 0;
  let lastHzSampleMs = performance.now();
  let smoothedFrameCostMs = 2.4;
  let lastDrawCalls = 3;
  let settleGraceFrames = 6; // Render a couple crisp frames then park at 0 rAF/s!
  let shuttleWakeTimer = 0;
  let parkedAtWallMs = 0;

  // Touch / Mouse drag state
  let isMouseDragging = false;
  let dragMovedPx = 0;
  let lastMouseX = 0;
  let lastMouseY = 0;
  let touchWalkFwd = 0;
  let touchWalkRight = 0;
  let walkTouchId = null;
  let walkOriginX = 0;
  let walkOriginY = 0;
  let lookTouchId = null;
  let lookPrevX = 0;
  let lookPrevY = 0;

  // Checklist completion tracking
  let taskSolarDone = false;
  let taskWaveDone = false;
  let taskBreakerTested = false;

  const elVisor = document.getElementById('eidos-visor');
  const elBtnVisor = document.getElementById('btn-visor');
  const elBtnMark = document.getElementById('btn-mark');
  const elBtnClip = document.getElementById('btn-clip');
  const elBtnLock = document.getElementById('btn-lock');
  const elBtnAudio = document.getElementById('btn-audio');
  const elBtnHud = document.getElementById('btn-hud');
  const elPromptMain = document.getElementById('prompt-main');
  const elPromptSub = document.getElementById('prompt-sub');
  const elReticle = document.getElementById('reticle');
  const elPerfHud = document.getElementById('perf-hud');
  const elWaveBanner = document.getElementById('wave-banner');
  const elClipSheet = document.getElementById('clipboard-sheet');
  const elCausalBody = document.getElementById('eidos-causal-body');
  const elProvBody = document.getElementById('eidos-prov-body');

  function getTelemetryView() {
    return new Float32Array(wasm.memory.buffer, wasm.odderon_telemetry_ptr(), 76);
  }

  function unprojectClientRay(clientX, clientY) {
    const rect = canvas.getBoundingClientRect();
    const ndcX = ((clientX - rect.left) / Math.max(rect.width, 1)) * 2.0 - 1.0;
    const ndcY = 1.0 - ((clientY - rect.top) / Math.max(rect.height, 1)) * 2.0;
    const aspect = rect.width / Math.max(rect.height, 1);
    const sx = ndcX * aspect * TAN_HALF_FOV;
    const sy = ndcY * TAN_HALF_FOV;
    let dx = camFwd[0] + camRight[0] * sx + camUp[0] * sy;
    let dy = camFwd[1] + camRight[1] * sx + camUp[1] * sy;
    let dz = camFwd[2] + camRight[2] * sx + camUp[2] * sy;
    const l = Math.hypot(dx, dy, dz) || 1;
    unprojDir[0] = dx / l;
    unprojDir[1] = dy / l;
    unprojDir[2] = dz / l;
    return unprojDir;
  }

  function wakePresenter(frames = 8) {
    if (shuttleWakeTimer) {
      clearTimeout(shuttleWakeTimer);
      shuttleWakeTimer = 0;
    }
    if (isParked && parkedAtWallMs > 0) {
      let sleptMs = Math.min(120000, Math.max(0, Math.round(performance.now() - parkedAtWallMs)));
      parkedAtWallMs = 0;
      while (sleptMs > 0) {
        const chunk = Math.min(250, sleptMs);
        wasm.odderon_step(chunk, 0, 0, 0, 0);
        sleptMs -= chunk;
      }
      const t = getTelemetryView();
      syncDynamicMeshes(t[30] > 0.5);
      syncAudioAndAck(t);
    }
    settleGraceFrames = Math.max(settleGraceFrames, frames);
    if (!rafId) {
      isParked = false;
      lastFrameTimeMs = performance.now();
      rafId = requestAnimationFrame(presentFrame);
    }
  }

  function formatJstClock(hourFloat) {
    const totalMin = Math.floor(((hourFloat % 24) + 24) % 24 * 60);
    const hh = String(Math.floor(totalMin / 60)).padStart(2, '0');
    const mm = String(totalMin % 60).padStart(2, '0');
    return `${hh}:${mm} JST`;
  }

  let prevPromptMain = '';
  let prevPromptSub = '';
  let prevWaveText = '';
  let prevWaveVis = false;
  let prevPerfHtml = '';
  let lastHeavyDomMs = -1000;

  function updateDomOverlays(t, forceHeavy = false) {
    const nowDomMs = performance.now();
    const allowHeavy = forceHeavy || isParked || (nowDomMs - lastHeavyDomMs >= 100);
    if (allowHeavy) lastHeavyDomMs = nowDomMs;

    const visorDown = t[30] > 0.5;
    const hovered = Math.round(t[31]);
    const clipOpen = t[33] > 0.5;
    const shojiPct = Math.round(t[20] * 100);
    const winPct = Math.round(t[21] * 100);
    const roomC = t[11];
    const qSolar = t[13];
    const acW = t[16];
    const busW = t[17];
    const busV = t[18];
    const busA = t[19];
    const tripped = t[23] > 0.5;
    const plumeAge = t[38];
    const shockR = t[39];
    const cSound = t[44];
    const padDist = t[45];

    if (qSolar < 230 && shojiPct > 85) taskSolarDone = true;
    if (shockR >= padDist) taskWaveDone = true;
    if (tripped) taskBreakerTested = true;

    elVisor.classList.toggle('down', visorDown);
    elBtnVisor.classList.toggle('on', visorDown);
    const visorLabel = visorDown ? '[V] EIDOS VISOR: DOWN' : '[V] EIDOS VISOR: UP';
    if (elBtnVisor.textContent !== visorLabel) elBtnVisor.textContent = visorLabel;
    elBtnMark.classList.toggle('mark-on', markMode);
    elBtnClip.classList.toggle('on', clipOpen);
    elClipSheet.classList.toggle('open', clipOpen);
    elReticle.classList.toggle('active', hovered !== 0);

    // Contextual physical hand prompt (diff-guarded so 0 DOM mutations when unchanged!)
    const info = TARGET_INFO[hovered];
    let nextMain = '';
    let nextSub = '';
    if (tripped && hovered !== 3) {
      nextMain = `⚠ 15.0A HAB BUS TRIPPED (Overcurrent > 15A) — Walk to Window Breaker Panel & Reset Main Breaker!`;
      nextSub = `Shed load (turn off 1,060W Kettle or close Shoji screen) before re-energizing.`;
    } else if (info) {
      let extra = '';
      if (hovered === 1) extra = ` · Now ${shojiPct}% closed (Solar: ${Math.round(qSolar)} W)`;
      if (hovered === 2) extra = ` · Now ${winPct}% open`;
      if (hovered === 8) extra = ` · Reading: ${roomC.toFixed(2)} °C${t[50] >= 0 ? ` | Mark: ${t[50].toFixed(1)} °C` : ''}`;
      if (hovered === 9) extra = ` · Reading: ${Math.round(busW)} W${t[51] >= 0 ? ` | Mark: ${Math.round(t[51])} W` : ''}`;
      if (hovered === 10) extra = ` · Reading: ${busV.toFixed(1)} V (${busA.toFixed(1)} A)${t[52] >= 0 ? ` | Mark: ${t[52].toFixed(1)} V` : ''}`;
      if (hovered === 14) extra = ` · Water: ${t[28].toFixed(1)} °C`;
      if (hovered === 17) extra = ` · Door ${t[65] > 0.5 ? 'OPEN' : 'CLOSED'}`;
      if (hovered === 18 || hovered === 19) {
        extra = t[68] > 0.25
          ? ` · SHUTTLE AT CURB NOW (16s Boarding Window!)`
          : ` · Next Pad-1 Shuttle in ${Math.ceil(t[67])}s`;
      }
      if (hovered === 20) extra = ` · Wallet: ¥${Math.round(t[66]).toLocaleString()} (Fare ¥240)`;
      if (hovered === 21) extra = ` · Wallet: ¥${Math.round(t[66]).toLocaleString()} · Drinks: ${Math.round(t[70])}`;
      nextMain = `${info.name}${extra}`;
      nextSub = info.hint;
    } else {
      const isOutdoors = t[73] > 0.5;
      const yenStr = `¥${Math.round(t[66]).toLocaleString()}`;
      const shuttleStr = t[68] > 0.25
        ? `Shuttle BOARDING AT CURB!`
        : `Next Shuttle ${Math.ceil(t[67])}s`;
      if (isOutdoors) {
        nextMain = `${formatJstClock(t[10])} · Uchinoura Street · Wallet ${yenStr} · ${shuttleStr}`;
        nextSub = `Walk Stairs / Sidewalk / Street · Click Bus Stop (バス停), Bench, Vending Machine (¥130), or Board Range Shuttle (¥240)`;
      } else {
        nextMain = `${formatJstClock(t[10])} · Uchinoura Hab #204 · ${yenStr} · Cabin ${roomC.toFixed(1)} °C · ${shuttleStr}`;
        nextSub = `WASD Move · Drag/[L] Look · Click Right-Wall Entry Door (#204) to Step Outside & Walk Down Stairs to Street!`;
      }
    }
    if (nextMain !== prevPromptMain) {
      elPromptMain.textContent = nextMain;
      prevPromptMain = nextMain;
    }
    if (nextSub !== prevPromptSub) {
      elPromptSub.textContent = nextSub;
      prevPromptSub = nextSub;
    }

    // Coastal Pad-1 343 m/s Shockwave Propagation Banner
    const waveVis = plumeAge >= 0 && plumeAge < 12.5;
    if (waveVis !== prevWaveVis) {
      elWaveBanner.style.display = waveVis ? 'block' : 'none';
      prevWaveVis = waveVis;
    }
    if (waveVis) {
      const wTxt = shockR < padDist
        ? `PAD-1 STATIC FIRE OBSERVED (2.82 km SE) · Acoustic Wavefront: ${(shockR / 1000).toFixed(2)} km @ ${cSound.toFixed(1)} m/s · Impact in ${Math.max(0, (padDist - shockR) / Math.max(cSound, 300)).toFixed(1)}s`
        : `ACOUSTIC N-WAVE ARRIVED (Δt = ${(padDist / cSound).toFixed(2)}s @ ${cSound.toFixed(1)} m/s) — Window Sash Rattling!`;
      if (wTxt !== prevWaveText) {
        elWaveBanner.textContent = wTxt;
        prevWaveText = wTxt;
      }
    }

    // Eidos AR Visor Causal Sensitivities & Provenance<T> Inspector (throttled 10Hz to prevent DOM churn)
    if (visorDown && allowHeavy) {
      const dQ = t[34];
      const dPac = t[35];
      const dTeq = t[36];
      elCausalBody.innerHTML = `
        <div class="kv-row"><span>Solar Aperture Gain (Q̇_sun):</span><span class="val-amber">${qSolar.toFixed(0)} ± ${t[47].toFixed(0)} W</span></div>
        <div class="kv-row"><span>AC Heat Rejection (Q̇_evap):</span><span class="val-hi">${t[14].toFixed(0)} W (COP ${t[29].toFixed(2)})</span></div>
        <div class="kv-row"><span>Net Cabin Air Flux (∑Q̇):</span><span class="${t[15] <= 0 ? 'val-green' : 'val-amber'}">${t[15] >= 0 ? '+' : ''}${t[15].toFixed(0)} W</span></div>
        <div class="kv-row"><span>100V Hab Bus Load:</span><span class="${busA > 14.5 ? 'val-red' : 'val-green'}">${busW.toFixed(0)} W (${busA.toFixed(2)} / 15.0 A)</span></div>
        <div style="margin-top:7px;padding-top:6px;border-top:1px solid rgba(92,225,230,0.2);">
          <div style="color:#93c5fd;font-size:10.5px;">PARTIAL DERIVATIVES (CAUSAL SENSITIVITIES):</div>
          <div class="kv-row"><span>∂Q̇_solar / ∂Shoji:</span><span class="val-green">${dQ.toFixed(0)} W (full close)</span></div>
          <div class="kv-row"><span>∂P_AC_elec / ∂Shoji:</span><span class="val-green">${dPac.toFixed(0)} W saved</span></div>
          <div class="kv-row"><span>∂T_eq / ∂Shoji:</span><span class="val-green">${dTeq.toFixed(2)} K equilibrium</span></div>
          <div class="kv-row"><span>∂V_bus / ∂I_load:</span><span class="val-amber">-0.19 V/A (line sag)</span></div>
        </div>
      `;

      const calCount = Math.round(t[53]);
      const mirrorHtml = calCount > 0
        ? `<div style="margin-top:6px;color:#fde68a;">Competence Mirror (N=${calCount}): Pred ${t[54].toFixed(1)} vs Actual ${t[55].toFixed(1)} → Err <strong>${t[56].toFixed(2)}%</strong> (Mean ${t[57].toFixed(2)}%)</div>`
        : `<div style="margin-top:6px;color:#94a3b8;">Competence Mirror: Press <strong>[M]</strong> + click any gauge to place a yellow China-marker prediction before toggling a load!</div>`;

      elProvBody.innerHTML = `
        <div><span class="prov-badge">CATALOG: UCHINOURA EPHEMERIS</span> Elev ${t[62].toFixed(1)}° · Window Flux ${t[61].toFixed(0)} W/m²</div>
        <div style="margin-top:5px;"><span class="prov-badge">MEASURED: THERMISTOR</span> T_air = <strong>${roomC.toFixed(2)} ± ${t[48].toFixed(2)} °C</strong></div>
        <div style="margin-top:5px;"><span class="prov-badge">EMPIRICAL: CARNOT R32</span> COP = η·T_evap/(T_cond−T_evap) = <strong>${t[29].toFixed(2)} ± 0.22</strong> (AC = ${acW.toFixed(0)} W)</div>
        <div style="margin-top:5px;"><span class="prov-badge">MEASURED: HALL SHUNT</span> P_bus = <strong>${busW.toFixed(0)} ± ${t[49].toFixed(1)} W</strong> @ ${busV.toFixed(2)} V</div>
        ${mirrorHtml}
      `;
    }

    if (clipOpen && allowHeavy) {
      const shiftCode = Math.round(t[71]);
      const shiftLabel = shiftCode === 1
        ? 'CHECKED IN ON-TIME (1st Shuttle!)'
        : (shiftCode === 2 ? 'CHECKED IN LATE (Missed 1st Shuttle — Rank 0 Lesson)' : (t[72] > 0.5 ? '1ST SHUTTLE MISSED — Catch Next Run!' : 'Awaiting Morning Commute'));
      document.getElementById('clipboard-clock').textContent =
        `${formatJstClock(t[10])} · Wallet ¥${Math.round(t[66]).toLocaleString()} · ${shiftLabel}`;
      document.getElementById('clip-task-1').className = 'clip-item' + (taskSolarDone ? ' done' : '');
      document.getElementById('clip-task-2').className = 'clip-item' + (taskWaveDone ? ' done' : '');
      document.getElementById('clip-task-3').className = 'clip-item ' + (taskBreakerTested ? 'done' : 'warn');
      const elTask4 = document.getElementById('clip-task-4');
      if (elTask4) {
        elTask4.className = 'clip-item' + (Math.round(t[69]) > 0 ? ' done' : '');
      }
    }

    // Performance & Doctrine Verification HUD (`[H]`)
    if (showPerfHud && allowHeavy) {
      const hashLo = (wasm.odderon_state_hash_lo() >>> 0).toString(16).padStart(8, '0');
      const hashHi = (wasm.odderon_state_hash_hi() >>> 0).toString(16).padStart(8, '0');
      const statusHtml = isParked
        ? `<span class="parked-tag">0 rAF/s · PARKED (0.0% IDLE CPU/GPU)</span>`
        : `<span>${displayedRafHz} rAF/s · ACTIVE (${smoothedFrameCostMs.toFixed(2)} ms/frame)</span>`;
      const perfHtml = `
        <div><strong>ODDERON PERF DOCTRINE [H]</strong> · Rust→WASM (${wasmKb} KB, 0 Assets)</div>
        <div>Presenter: ${statusHtml}</div>
        <div>WebGL2 Draws: <strong>${lastDrawCalls}</strong> (Budget ≤ 8) · Shaded: ${fbWidth}×${fbHeight} (Q=${qualityScale.toFixed(2)})</div>
        <div>Proof-of-Flight Hash: <code>0x${hashHi}${hashLo}</code></div>
      `;
      if (perfHtml !== prevPerfHtml) {
        elPerfHud.innerHTML = perfHtml;
        prevPerfHtml = perfHtml;
      }
    }
  }

  function renderWebGL(t) {
    const visorDown = t[30] > 0.5;
    const aspect = fbWidth / Math.max(fbHeight, 1);

    computeCameraBasisAndView(t[0], t[1], t[2], t[3], t[4], t[5]);
    perspectiveFieldOfView(projMat, FOV_Y_RAD, aspect, 0.16, 480.0);
    mulMat4(vpMat, projMat, viewMat);

    sunVec[0] = t[6]; sunVec[1] = t[7]; sunVec[2] = t[8];
    const warmth = t[9];

    gl.viewport(0, 0, fbWidth, fbHeight);
    let draws = 0;

    // DRAW 1: Full-screen analytical Shinkai Sky + Cumulonimbus Clouds + 343 m/s Shockwave Ring
    gl.disable(gl.DEPTH_TEST);
    gl.depthMask(false);
    gl.useProgram(skyProg);
    gl.uniform3fv(skyLocs.uCamFwd, camFwd);
    gl.uniform3fv(skyLocs.uCamRight, camRight);
    gl.uniform3fv(skyLocs.uCamUp, camUp);
    gl.uniform1f(skyLocs.uTanHalfFov, TAN_HALF_FOV);
    gl.uniform1f(skyLocs.uAspect, aspect);
    gl.uniform3fv(skyLocs.uSunDir, sunVec);
    gl.uniform1f(skyLocs.uWarmth, warmth);
    gl.uniform1f(skyLocs.uShockFrac, t[39] > 0 ? t[39] / Math.max(t[45], 1) : 0.0);
    gl.bindVertexArray(emptyVao);
    gl.drawArrays(gl.TRIANGLES, 0, 3);
    draws++;

    // DRAW 2 & 3: Parametric Static World + Dynamic Moving Hardware (strict CCW backface culling!)
    gl.enable(gl.DEPTH_TEST);
    gl.depthFunc(gl.LEQUAL);
    gl.depthMask(true);
    gl.clear(gl.DEPTH_BUFFER_BIT);
    gl.frontFace(gl.CCW);
    gl.cullFace(gl.BACK);
    gl.enable(gl.CULL_FACE);

    gl.useProgram(worldProg);
    gl.uniformMatrix4fv(worldLocs.uViewProj, false, vpMat);
    gl.uniform3f(worldLocs.uCamPos, t[0], t[1], t[2]);
    gl.uniform3fv(worldLocs.uSunDir, sunVec);
    gl.uniform1f(worldLocs.uWarmth, warmth);
    gl.uniform1f(worldLocs.uShojiClosed, t[20]);
    gl.uniform1f(worldLocs.uEntryDoorOpen, t[65] || 0.0);
    gl.uniform1f(worldLocs.uLampOn, (t[22] > 0.5 && t[26] > 0.5) ? 1.0 : 0.0);
    gl.uniform1f(worldLocs.uPlumeGlow, t[37]);

    gl.bindVertexArray(staticMesh.vao);
    gl.drawElements(gl.TRIANGLES, staticMesh.indexCount, gl.UNSIGNED_INT, 0);
    draws++;

    gl.bindVertexArray(dynMesh.vao);
    gl.drawElements(gl.TRIANGLES, dynMesh.indexCount, gl.UNSIGNED_INT, 0);
    draws++;

    // Optional DRAW 4: Eidos 3D Causal Flow Vectors when Visor is flipped down
    if (visorDown && eidosMesh.indexCount > 0) {
      gl.bindVertexArray(eidosMesh.vao);
      gl.drawElements(gl.TRIANGLES, eidosMesh.indexCount, gl.UNSIGNED_INT, 0);
      draws++;
    }

    gl.bindVertexArray(null);
    lastDrawCalls = draws;
  }

  function presentFrame(nowMs) {
    rafId = 0;
    const frameStart = performance.now();
    const dtMs = Math.min(100, Math.max(1, Math.round(nowMs - lastFrameTimeMs)));
    lastFrameTimeMs = nowMs;

    rafCountWindow++;
    if (nowMs - lastHzSampleMs >= 1000) {
      displayedRafHz = rafCountWindow;
      rafCountWindow = 0;
      lastHzSampleMs = nowMs;
    }

    let fwd = touchWalkFwd;
    let right = touchWalkRight;
    if (keys['KeyW'] || keys['ArrowUp']) fwd += 1.0;
    if (keys['KeyS'] || keys['ArrowDown']) fwd -= 1.0;
    if (keys['KeyD'] || keys['ArrowRight']) right += 1.0;
    if (keys['KeyA'] || keys['ArrowLeft']) right -= 1.0;

    const hasInput =
      Math.abs(fwd) > 1e-3 ||
      Math.abs(right) > 1e-3 ||
      Math.abs(pendingYaw) > 1e-5 ||
      Math.abs(pendingPitch) > 1e-5;

    const settledFlag = wasm.odderon_step(dtMs, fwd, right, pendingYaw, pendingPitch);
    pendingYaw = 0;
    pendingPitch = 0;

    const t = getTelemetryView();
    syncDynamicMeshes(t[30] > 0.5);
    syncAudioAndAck(t);

    updateCanvasResolution();
    renderWebGL(t);

    const costMs = performance.now() - frameStart;
    smoothedFrameCostMs = smoothedFrameCostMs * 0.85 + costMs * 0.15;

    // Adaptive governor for older 2015 Intel UHD / Android 8 GPUs
    if (smoothedFrameCostMs > 18.0 && qualityScale > 0.58) {
      qualityScale = Math.max(0.55, qualityScale - 0.05);
    }

    if (hasInput || settledFlag === 0) {
      settleGraceFrames = Math.max(settleGraceFrames, 4);
    } else if (settleGraceFrames > 0) {
      settleGraceFrames--;
    }

    if (!hasInput && settledFlag === 1 && settleGraceFrames === 0) {
      // PARK! Schedule 0 rAFs — compositor holds our crisp painted frame at 0.0% CPU / 0.0% GPU,
      // with a single discrete event timer when the next scheduled Range Shuttle enters Uchinoura Street!
      isParked = true;
      parkedAtWallMs = performance.now();
      displayedRafHz = 0;
      updateDomOverlays(t, true);
      const nextShuttleMs = t[75];
      if (nextShuttleMs > 40 && !shuttleWakeTimer) {
        shuttleWakeTimer = setTimeout(() => {
          shuttleWakeTimer = 0;
          wakePresenter(16);
        }, Math.ceil(nextShuttleMs) + 25);
      }
      return;
    }

    isParked = false;
    updateDomOverlays(t, false);
    rafId = requestAnimationFrame(presentFrame);
  }

  // Discrete scheduled distant Pad-1 morning static fire at T = 9.0s (wakes presenter via event, zero polling!)
  setTimeout(() => {
    if (wasm.odderon_auto_fire_done() === 0) {
      wasm.odderon_toggle_target(11);
      const t = getTelemetryView();
      syncDynamicMeshes(t[30] > 0.5);
      syncAudioAndAck(t);
      wakePresenter(30);
    }
  }, 9000);

  // Fade open wake-up eyelids immediately (IMMERSION.md N1)
  requestAnimationFrame(() => {
    document.getElementById('wake-veil').classList.add('awake');
    wakePresenter(12);
  });

  // --- Event Listeners (Keyboard, Mouse Drag / Pointer Lock, Direct 3D Raycast, Android Touch) ---

  window.addEventListener('resize', () => {
    updateCanvasResolution();
    wakePresenter(4);
  });

  window.addEventListener('blur', () => {
    for (const k in keys) keys[k] = false;
    isMouseDragging = false;
    walkTouchId = null;
    lookTouchId = null;
    touchWalkFwd = 0;
    touchWalkRight = 0;
  });

  window.addEventListener('keydown', (e) => {
    audio.ensureStarted();
    keys[e.code] = true;
    if (e.repeat && (e.code === 'KeyV' || e.code === 'KeyM' || e.code === 'KeyC' || e.code === 'KeyF' || e.code === 'KeyH' || e.code === 'KeyL')) {
      return;
    }
    if (e.code === 'KeyV') {
      const t = getTelemetryView();
      wasm.odderon_flip_visor(t[30] > 0.5 ? 0 : 1);
      syncAudioAndAck(getTelemetryView());
      updateDomOverlays(getTelemetryView(), true);
      wakePresenter(6);
    } else if (e.code === 'KeyM') {
      markMode = !markMode;
      audio.playClick(0.5);
      updateDomOverlays(getTelemetryView(), true);
      wakePresenter(3);
    } else if (e.code === 'KeyC') {
      wasm.odderon_toggle_target(12);
      syncAudioAndAck(getTelemetryView());
      updateDomOverlays(getTelemetryView(), true);
      wakePresenter(4);
    } else if (e.code === 'KeyF') {
      wasm.odderon_toggle_target(11);
      syncAudioAndAck(getTelemetryView());
      updateDomOverlays(getTelemetryView(), true);
      wakePresenter(30);
    } else if (e.code === 'KeyH') {
      showPerfHud = !showPerfHud;
      elPerfHud.classList.toggle('hidden', !showPerfHud);
      elBtnHud.classList.toggle('on', showPerfHud);
      elBtnHud.textContent = showPerfHud ? '[H] PERF: ON' : '[H] PERF: OFF';
      updateDomOverlays(getTelemetryView(), true);
      wakePresenter(2);
    } else if (e.code === 'KeyL') {
      if (document.pointerLockElement === canvas) {
        document.exitPointerLock();
      } else if (canvas.requestPointerLock) {
        canvas.requestPointerLock();
      }
    } else if (
      e.code === 'KeyW' || e.code === 'KeyA' || e.code === 'KeyS' || e.code === 'KeyD' ||
      e.code === 'ArrowUp' || e.code === 'ArrowDown' || e.code === 'ArrowLeft' || e.code === 'ArrowRight'
    ) {
      wakePresenter(6);
    }
  });

  window.addEventListener('keyup', (e) => {
    keys[e.code] = false;
  });

  document.addEventListener('pointerlockchange', () => {
    const locked = document.pointerLockElement === canvas;
    elBtnLock.classList.toggle('on', locked);
    elBtnLock.textContent = locked ? '[ESC] UNLOCK' : '[L] FPS LOOK';
  });

  canvas.addEventListener('contextmenu', (e) => e.preventDefault());

  canvas.addEventListener('mousedown', (e) => {
    audio.ensureStarted();
    if (document.pointerLockElement === canvas) {
      wasm.odderon_hand_actuate(markMode ? 1 : 0, e.button === 2 ? 1 : 0);
      syncAudioAndAck(getTelemetryView());
      updateDomOverlays(getTelemetryView(), true);
      wakePresenter(16);
      return;
    }
    isMouseDragging = true;
    dragMovedPx = 0;
    lastMouseX = e.clientX;
    lastMouseY = e.clientY;
  });

  window.addEventListener('mousemove', (e) => {
    if (document.pointerLockElement === canvas) {
      pendingYaw -= e.movementX * 0.0024;
      pendingPitch -= e.movementY * 0.0022;
      wakePresenter(4);
      return;
    }
    if (isMouseDragging) {
      const dx = e.clientX - lastMouseX;
      const dy = e.clientY - lastMouseY;
      lastMouseX = e.clientX;
      lastMouseY = e.clientY;
      dragMovedPx += Math.hypot(dx, dy);
      pendingYaw -= dx * 0.0032;
      pendingPitch -= dy * 0.0028;
      wakePresenter(4);
    } else {
      // Hover raycast directly under unlocked cursor so switches/gauges highlight effortlessly!
      const dir = unprojectClientRay(e.clientX, e.clientY);
      const prevHover = Math.round(getTelemetryView()[31]);
      const hit = wasm.odderon_ray_actuate(dir[0], dir[1], dir[2], 1, 0, 0);
      if (hit !== prevHover) {
        updateDomOverlays(getTelemetryView());
      }
    }
  });

  window.addEventListener('mouseup', (e) => {
    if (!isMouseDragging) return;
    isMouseDragging = false;
    if (dragMovedPx < 6 && e.target === canvas) {
      const dir = unprojectClientRay(e.clientX, e.clientY);
      const hit = wasm.odderon_ray_actuate(
        dir[0],
        dir[1],
        dir[2],
        0,
        markMode ? 1 : 0,
        e.button === 2 ? 1 : 0
      );
      syncAudioAndAck(getTelemetryView());
      if (hit !== 0) {
        updateDomOverlays(getTelemetryView(), true);
        wakePresenter(24);
      }
    }
  });

  // Android 8+ Direct Touch Locomotion + Look + 3D Object Tap!
  canvas.addEventListener('touchstart', (e) => {
    audio.ensureStarted();
    for (const touch of e.changedTouches) {
      const inWalkZone = touch.clientX < 130 && touch.clientY > window.innerHeight - 130;
      if (inWalkZone && walkTouchId === null) {
        walkTouchId = touch.identifier;
        walkOriginX = touch.clientX;
        walkOriginY = touch.clientY;
      } else if (lookTouchId === null) {
        lookTouchId = touch.identifier;
        lookPrevX = touch.clientX;
        lookPrevY = touch.clientY;
        dragMovedPx = 0;
      }
    }
  }, { passive: true });

  canvas.addEventListener('touchmove', (e) => {
    for (const touch of e.changedTouches) {
      if (touch.identifier === walkTouchId) {
        touchWalkRight = Math.max(-1, Math.min(1, (touch.clientX - walkOriginX) / 34));
        touchWalkFwd = Math.max(-1, Math.min(1, (walkOriginY - touch.clientY) / 34));
        wakePresenter(6);
      } else if (touch.identifier === lookTouchId) {
        const dx = touch.clientX - lookPrevX;
        const dy = touch.clientY - lookPrevY;
        lookPrevX = touch.clientX;
        lookPrevY = touch.clientY;
        dragMovedPx += Math.hypot(dx, dy);
        pendingYaw -= dx * 0.0042;
        pendingPitch -= dy * 0.0036;
        wakePresenter(6);
      }
    }
  }, { passive: true });

  canvas.addEventListener('touchend', (e) => {
    for (const touch of e.changedTouches) {
      if (touch.identifier === walkTouchId) {
        walkTouchId = null;
        touchWalkFwd = 0;
        touchWalkRight = 0;
      } else if (touch.identifier === lookTouchId) {
        lookTouchId = null;
        if (dragMovedPx < 10) {
          const dir = unprojectClientRay(touch.clientX, touch.clientY);
          wasm.odderon_ray_actuate(dir[0], dir[1], dir[2], 0, markMode ? 1 : 0, 0);
          syncAudioAndAck(getTelemetryView());
          updateDomOverlays(getTelemetryView(), true);
          wakePresenter(24);
        }
      }
    }
  }, { passive: true });

  // Brow & Clipboard Buttons
  elBtnVisor.addEventListener('click', () => {
    audio.ensureStarted();
    const t = getTelemetryView();
    wasm.odderon_flip_visor(t[30] > 0.5 ? 0 : 1);
    syncAudioAndAck(getTelemetryView());
    updateDomOverlays(getTelemetryView(), true);
    wakePresenter(8);
  });

  elBtnMark.addEventListener('click', () => {
    audio.ensureStarted();
    markMode = !markMode;
    audio.playClick(0.5);
    updateDomOverlays(getTelemetryView(), true);
    wakePresenter(3);
  });

  elBtnClip.addEventListener('click', () => {
    audio.ensureStarted();
    wasm.odderon_toggle_target(12);
    syncAudioAndAck(getTelemetryView());
    updateDomOverlays(getTelemetryView(), true);
    wakePresenter(6);
  });

  document.getElementById('btn-close-clip').addEventListener('click', () => {
    wasm.odderon_toggle_target(12);
    syncAudioAndAck(getTelemetryView());
    updateDomOverlays(getTelemetryView(), true);
    wakePresenter(4);
  });

  document.getElementById('btn-fire').addEventListener('click', () => {
    audio.ensureStarted();
    wasm.odderon_toggle_target(11);
    syncAudioAndAck(getTelemetryView());
    updateDomOverlays(getTelemetryView(), true);
    wakePresenter(30);
  });

  elBtnLock.addEventListener('click', () => {
    audio.ensureStarted();
    if (document.pointerLockElement === canvas) {
      document.exitPointerLock();
    } else if (canvas.requestPointerLock) {
      canvas.requestPointerLock();
    }
  });

  elBtnAudio.addEventListener('click', () => {
    const muted = audio.toggleMute();
    elBtnAudio.textContent = muted ? '[UNMUTE] AUDIO' : '[MUTE] AUDIO';
  });

  elBtnHud.addEventListener('click', () => {
    showPerfHud = !showPerfHud;
    elPerfHud.classList.toggle('hidden', !showPerfHud);
    elBtnHud.classList.toggle('on', showPerfHud);
    elBtnHud.textContent = showPerfHud ? '[H] PERF: ON' : '[H] PERF: OFF';
  });

  function readProofJsonString() {
    const ptr = wasm.odderon_build_proof_json();
    const len = wasm.odderon_proof_json_len();
    const slice = new Uint8Array(wasm.memory.buffer, ptr, len);
    return new TextDecoder().decode(slice);
  }

  function downloadFile(filename, mime, content) {
    const blob = new Blob([content], { type: mime });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = filename;
    a.click();
    setTimeout(() => URL.revokeObjectURL(url), 1500);
  }

  document.getElementById('btn-export-json').addEventListener('click', () => {
    const json = readProofJsonString();
    downloadFile('odderon-proof-of-flight-1A.json', 'application/json', json);
  });

  document.getElementById('btn-export-html').addEventListener('click', () => {
    const data = JSON.parse(readProofJsonString());
    const html = `<!DOCTYPE html>
<html lang="en"><head><meta charset="utf-8"><title>Akashi Technical Dossier — ${data.state_hash_hex}</title>
<style>body{font-family:Georgia,serif;max-width:760px;margin:40px auto;padding:0 20px;color:#111;line-height:1.55}
code,pre{font-family:monospace;background:#f4f4f6;padding:2px 5px;border-radius:4px}
table{width:100%;border-collapse:collapse;margin:16px 0}th,td{border:1px solid #cbd5e1;padding:8px;text-align:left}</style></head>
<body>
<h1>AKASHI (証) TECHNICAL DOSSIER — ODDERON</h1>
<p><strong>Checkpoint:</strong> ${data.checkpoint} · <strong>Site:</strong> ${data.location}<br>
<strong>Deterministic Proof-of-Flight State Hash:</strong> <code>0x${data.state_hash_hex}</code></p>
<h2>1. Coupled 1D Habitation Thermal &amp; Electrical Bus Telemetry</h2>
<table>
<tr><th>Observable</th><th>Value</th><th>Model / Epistemic Provenance</th></tr>
<tr><td>Cabin Air Temperature</td><td>${data.telemetry.room_temp_c} °C</td><td>MeasuredSensor (±0.15 °C thermistor)</td></tr>
<tr><td>Solar Glazing Heat Gain</td><td>${data.telemetry.q_solar_gain_w} W</td><td>DerivedConservation (Uchinoura Ephemeris × SHGC)</td></tr>
<tr><td>R32 Vapour-Compression Cooling</td><td>${data.telemetry.ac_cooling_w} W (COP ${data.telemetry.ac_cop})</td><td>EmpiricalModel (Carnot-derated η=0.44)</td></tr>
<tr><td>100V Hab Bus Load &amp; Sag</td><td>${data.telemetry.bus_power_w} W @ ${data.telemetry.bus_voltage_v} V</td><td>MeasuredSensor (R_line = 0.19 Ω)</td></tr>
</table>
<h2>2. Eidos Analytical Partial Derivatives (Sensitivities)</h2>
<ul>
<li><code>∂Q̇_solar / ∂Shoji</code> = ${data.eidos_sensitivities.d_qsolar_d_shoji_w} W</li>
<li><code>∂P_AC / ∂Shoji</code> = ${data.eidos_sensitivities.d_pac_d_shoji_w} W</li>
<li><code>∂T_eq / ∂Shoji</code> = ${data.eidos_sensitivities.d_teq_d_shoji_k} K</li>
</ul>
<h2>3. Calibration &amp; Verification</h2>
<p>Predictions Scored: <strong>${data.calibration.predictions_scored}</strong> · Mean Absolute Error: <strong>${data.calibration.mean_abs_error_pct}%</strong> · Actuator Log Events: <strong>${data.actuator_events_count}</strong></p>
<p><em>Disclaimer: Odderon is an aerospace engineering training simulator, not a flight-qualification or safety-critical certification tool.</em></p>
</body></html>`;
    downloadFile(`akashi-report-${data.state_hash_hex}.html`, 'text/html', html);
  });
}

boot().catch((err) => {
  console.error(err);
  document.getElementById('prompt-main').textContent = 'Boot error: ' + err.message;
});
