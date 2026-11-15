// Engine CAD & Schlieren lab.
//
// Every number here comes from odd-propulsion through the scalar wasm bridge.
// This module draws; it never computes thrust, Isp, separation or a density
// gradient. The one arithmetic it does is unit conversion and screen mapping.
//
// Idle law (WP 1.3): a Governor owns the only animation frames this panel
// requests, and it is invalidated by input alone. There is no timer, no
// requestAnimationFrame loop, and no worker.
import { Governor } from './governor.js';

const FIELDS = {
  thrust: 0, isp: 1, massFlow: 2, cStar: 3, tc: 4, molar: 5, gamma: 6,
  bulkDensity: 7, expansion: 8, exitMach: 9, exitPressure: 10, exitTemperature: 11,
  divergence: 12, nozzleLength: 13, heatFlux: 14, wallTemperature: 15, film: 16,
  engineMass: 17, pumpPower: 18, chamberRadius: 19, chamberLength: 20,
  separationX: 21, separationArea: 22, sideLoad: 23, thermalFlag: 24,
  exhaustVelocity: 25, ambient: 26, flightReady: 27, of: 28, pc: 29,
  throatRadius: 30, exitRadius: 31, lengthFraction: 32, propellant: 33,
  stageDry: 34, stagePropellant: 35, stageWet: 36, volumeLimited: 37,
  vacuumThrust: 38, vacuumIsp: 39, stageDeltaV: 40,
};
const PLUME = {
  regime: 0, pressureRatio: 1, lipTurn: 2, lipShock: 3, lipDeflection: 4,
  jetMach: 5, cellLength: 6, machDisk: 7, originX: 8, originRadius: 9,
  originPressure: 10, originDensity: 11, ambientDensity: 12,
};
export const PROPULSION_ERRORS = {
  0: 'Unknown propellant pair.',
  '-1': 'Mixture ratio is outside the calibrated band for this propellant pair.',
  '-2': 'Chamber pressure is outside the calibrated range (0.2–40 MPa).',
  '-3': 'Nozzle geometry is out of range: the exit must be at least the throat, and the expansion ratio at most 250.',
  '-4': 'The flow solution did not converge for this design.',
  '-5': 'Solved, but the regeneratively cooled throat exceeds its 1350 K wall limit.',
};
const REGIMES = ['Adapted', 'Underexpanded', 'Overexpanded', 'Separated inside the bell'];
const BANDS = { 0: [2.5, 10], 1: [1.4, 3.6], 2: [2.0, 4.6], 3: [1.0, 3.0] };
const OPTIMAL = { 0: [4.5, 6.5], 1: [2.2, 2.7], 2: [3.2, 3.8], 3: [1.6, 2.2] };
const LABELS = { 0: 'LOX / LH2', 1: 'LOX / RP-1', 2: 'LOX / LCH4', 3: 'NTO / MMH' };
// Raster resolution. The gradient is sampled in Rust; a coarse grid keeps a
// drag interactive without asking the browser to differentiate anything.
const RASTER_W = 220, RASTER_H = 110;

const $ = id => document.getElementById(id);

export function createLab({ wasm, onApply, onDiagnose, isLocked, annotate }) {
  const canvas = $('engine-canvas');
  const ctx = canvas.getContext('2d');
  const controls = {
    propellant: $('engine-propellant'), of: $('engine-of'), pc: $('engine-pc'),
    throat: $('engine-throat'), exit: $('engine-exit'), length: $('engine-length'),
    altitude: $('engine-altitude'),
  };
  let spec = null, solution = null, plume = null, stations = [], raster = null;
  let view = null, drag = null, draws = 0, solves = 0, lastCode = 1, dirty = true;
  const governor = new Governor(() => paint());

  const read = index => wasm.mission_engine_field(index);
  const readPlume = index => wasm.mission_engine_plume(index);

  function controlsToSpec() {
    return {
      propellant: Number(controls.propellant.value),
      of: Number(controls.of.value),
      pc: Number(controls.pc.value) * 1e6,
      throat: Number(controls.throat.value) / 1000,
      exit: Number(controls.exit.value) / 1000,
      length: Number(controls.length.value),
    };
  }

  function specToControls(next) {
    controls.propellant.value = String(next.propellant);
    const band = BANDS[next.propellant] ?? [1, 10];
    controls.of.min = String(band[0]); controls.of.max = String(band[1]);
    controls.of.value = String(next.of);
    controls.pc.value = String(+(next.pc / 1e6).toFixed(3));
    controls.throat.value = String(+(next.throat * 1000).toFixed(2));
    controls.exit.value = String(+(next.exit * 1000).toFixed(2));
    controls.length.value = String(next.length);
    const optimum = OPTIMAL[next.propellant];
    $('engine-of-band').textContent = optimum
      ? `peak efficiency ${optimum[0]}–${optimum[1]}`
      : '—';
  }

  /// Ask Rust to solve the design on the stand, then read every field back.
  function solve() {
    spec = controlsToSpec();
    const altitude = Number(controls.altitude.value) * 1000;
    const ambient = wasm.mission_engine_ambient(altitude);
    $('engine-ambient').textContent = ambient >= 1000
      ? `${(ambient / 1000).toFixed(2)} kPa`
      : `${ambient.toFixed(ambient < 1 ? 4 : 1)} Pa`;
    solves++;
    lastCode = wasm.mission_engine_solve(
      spec.propellant, spec.of, spec.pc, spec.throat, spec.exit, spec.length, ambient,
    );
    if (lastCode !== 1 && lastCode !== -5) {
      solution = null; plume = null; stations = []; raster = null; dirty = true;
      render(); governor.invalidate(false);
      return false;
    }
    solution = {};
    for (const [key, index] of Object.entries(FIELDS)) solution[key] = read(index);
    plume = {};
    for (const [key, index] of Object.entries(PLUME)) plume[key] = readPlume(index);
    const count = wasm.mission_engine_station_count();
    stations = Array.from({ length: count }, (_, i) => ({
      x: wasm.mission_engine_station(i, 0),
      radius: wasm.mission_engine_station(i, 1),
      area: wasm.mission_engine_station(i, 2),
      mach: wasm.mission_engine_station(i, 3),
      pressure: wasm.mission_engine_station(i, 4),
      temperature: wasm.mission_engine_station(i, 5),
      density: wasm.mission_engine_station(i, 6),
      velocity: wasm.mission_engine_station(i, 7),
    }));
    dirty = true;
    render();
    governor.invalidate(false);
    return true;
  }

  /// Screen mapping. The whole assembly plus four exit radii of plume must
  /// fit, so the view rescales as the bell grows.
  function computeView(width, height) {
    const length = solution.nozzleLength;
    const chamber = solution.chamberLength;
    const radius = Math.max(solution.exitRadius, solution.chamberRadius);
    const xMin = -chamber - 0.25 * length;
    const xMax = length + 4 * solution.exitRadius;
    const pad = 26;
    const scale = Math.min((width - 2 * pad) / (xMax - xMin), (height - 2 * pad) / (2.6 * radius));
    const axis = height / 2;
    const originX = pad - xMin * scale;
    return { width, height, scale, axis, originX, xMin, xMax, rMax: 1.3 * radius };
  }
  const toScreen = (x, r) => [view.originX + x * view.scale, view.axis - r * view.scale];
  const toWorld = (px, py) => [(px - view.originX) / view.scale, (view.axis - py) / view.scale];

  /// The Schlieren raster is requested once per design/view change and cached
  /// as an ImageData. Repainting a drag never re-solves the field.
  function buildRaster() {
    const count = wasm.mission_engine_schlieren(
      RASTER_W, RASTER_H, view.xMin, view.xMax, view.rMax,
    );
    if (!count) { raster = null; return; }
    const width = wasm.mission_engine_schlieren_width();
    const height = wasm.mission_engine_schlieren_height();
    const image = new ImageData(width, height);
    for (let i = 0; i < count; i++) {
      // Classical greyscale Schlieren: bright where the knife edge passes
      // light, dark where the gradient bends it away.
      const v = wasm.mission_engine_schlieren_at(i);
      const level = Math.round(255 * Math.min(1, Math.max(0, v)));
      image.data[i * 4] = level;
      image.data[i * 4 + 1] = level;
      image.data[i * 4 + 2] = Math.round(level * 0.96);
      image.data[i * 4 + 3] = 255;
    }
    const buffer = document.createElement('canvas');
    buffer.width = width; buffer.height = height;
    buffer.getContext('2d').putImageData(image, 0, 0);
    raster = buffer;
  }

  /// Mixture ratio is drawn as a slider riding the chamber roof: richer to
  /// the left, leaner to the right, with the efficient band marked. Chamber
  /// pressure is the injector-face handle, dragged vertically. Both act on
  /// the cross-section itself, so the whole design is reachable by pointer.
  function ofTrack() {
    const band = BANDS[spec.propellant] ?? [1, 10];
    const x0 = -solution.chamberLength;
    const x1 = -0.35 * solution.nozzleLength;
    const r = solution.chamberRadius * 1.55;
    return { band, x0, x1, r };
  }
  function handles() {
    if (!solution || !view) return [];
    const length = solution.nozzleLength;
    const track = ofTrack();
    const span = track.band[1] - track.band[0];
    const frac = span > 0 ? (spec.of - track.band[0]) / span : 0;
    return [
      { key: 'throat', at: toScreen(0, solution.throatRadius), axis: 'r' },
      { key: 'exit', at: toScreen(length, solution.exitRadius), axis: 'r' },
      { key: 'length', at: toScreen(length, 0), axis: 'x' },
      { key: 'of', at: toScreen(track.x0 + frac * (track.x1 - track.x0), track.r), axis: 'of' },
      { key: 'pc', at: toScreen(-solution.chamberLength, 0), axis: 'pc' },
    ];
  }

  function paint() {
    const rect = canvas.getBoundingClientRect();
    if (!rect.width || !rect.height) return;
    const dpr = Math.min(devicePixelRatio || 1, 2);
    canvas.width = Math.round(rect.width * dpr);
    canvas.height = Math.round(rect.height * dpr);
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, rect.width, rect.height);
    draws++;
    ctx.fillStyle = '#08110f'; ctx.fillRect(0, 0, rect.width, rect.height);
    if (!solution) {
      ctx.fillStyle = '#edbc98'; ctx.font = '12px system-ui';
      ctx.fillText(PROPULSION_ERRORS[String(lastCode)] ?? 'No solution.', 16, rect.height / 2);
      return;
    }
    view = computeView(rect.width, rect.height);
    if (dirty) { buildRaster(); dirty = false; }
    if (raster) {
      const [x0, y0] = toScreen(view.xMin, view.rMax);
      const [x1, y1] = toScreen(view.xMax, -view.rMax);
      ctx.imageSmoothingEnabled = true;
      ctx.globalAlpha = 0.92;
      ctx.drawImage(raster, x0, y0, x1 - x0, y1 - y0);
      ctx.globalAlpha = 1;
    }
    const length = solution.nozzleLength;
    // Chamber and bell wall, drawn over the field as the CAD outline.
    ctx.strokeStyle = '#cbd7c9'; ctx.lineWidth = 2;
    for (const sign of [1, -1]) {
      ctx.beginPath();
      const chamberR = solution.chamberRadius;
      ctx.moveTo(...toScreen(-solution.chamberLength, sign * chamberR));
      ctx.lineTo(...toScreen(-0.35 * length, sign * chamberR));
      ctx.lineTo(...toScreen(0, sign * solution.throatRadius));
      for (const station of stations) {
        ctx.lineTo(...toScreen(station.x * length, sign * station.radius));
      }
      ctx.stroke();
    }
    // Axis.
    ctx.strokeStyle = '#3d5a4e'; ctx.lineWidth = 1; ctx.setLineDash([5, 5]);
    ctx.beginPath(); ctx.moveTo(...toScreen(view.xMin, 0)); ctx.lineTo(...toScreen(view.xMax, 0));
    ctx.stroke(); ctx.setLineDash([]);
    // Detachment line exactly at the solved x_sep, plus the separated shear
    // layer the plume model traces from there.
    if (Number.isFinite(solution.separationX)) {
      const x = solution.separationX * length;
      const r = Math.sqrt(solution.separationArea) * solution.throatRadius;
      ctx.strokeStyle = '#edbc98'; ctx.lineWidth = 2; ctx.setLineDash([3, 3]);
      for (const sign of [1, -1]) {
        ctx.beginPath(); ctx.moveTo(...toScreen(x, 0)); ctx.lineTo(...toScreen(x, sign * r));
        ctx.stroke();
      }
      ctx.setLineDash([]);
      ctx.fillStyle = '#edbc98'; ctx.font = '10px system-ui';
      const [lx, ly] = toScreen(x, r);
      ctx.fillText(`separation x/L ${solution.separationX.toFixed(3)}`, lx + 6, ly - 6);
    }
    // Prandtl-Meyer fan at the lip, or the oblique shock when compressed.
    const originX = plume.originX, originR = plume.originRadius;
    if (plume.lipTurn > 1e-6) {
      ctx.strokeStyle = 'rgba(112,215,220,.75)'; ctx.lineWidth = 1;
      const machAngle = Math.asin(Math.min(1, 1 / Math.max(1, solution.exitMach)));
      for (const sign of [1, -1]) {
        for (let i = 0; i <= 6; i++) {
          const angle = machAngle - (machAngle - plume.lipTurn) * (i / 6);
          const reach = 2.2 * originR;
          ctx.beginPath();
          ctx.moveTo(...toScreen(originX, sign * originR));
          ctx.lineTo(...toScreen(originX + reach * Math.cos(angle), sign * (originR + reach * Math.sin(angle))));
          ctx.stroke();
        }
      }
    } else if (Number.isFinite(plume.lipShock)) {
      ctx.strokeStyle = 'rgba(237,188,152,.85)'; ctx.lineWidth = 2;
      const reach = 2.4 * originR;
      for (const sign of [1, -1]) {
        ctx.beginPath();
        ctx.moveTo(...toScreen(originX, sign * originR));
        ctx.lineTo(...toScreen(
          originX + reach * Math.cos(plume.lipShock),
          sign * (originR - reach * Math.sin(plume.lipShock)),
        ));
        ctx.stroke();
      }
    }
    // Mach diamonds: one marker per Prandtl-Pack cell while the train survives.
    if (plume.cellLength > 1e-9) {
      ctx.strokeStyle = 'rgba(226,188,121,.7)'; ctx.lineWidth = 1;
      for (let cell = 1; cell <= 8; cell++) {
        const s = cell * plume.cellLength;
        if (originX + s > view.xMax) break;
        const r = wasm.mission_engine_plume_boundary(s);
        if (!Number.isFinite(r)) break;
        for (const sign of [1, -1]) {
          ctx.beginPath();
          ctx.moveTo(...toScreen(originX + s - plume.cellLength * 0.5, 0));
          ctx.lineTo(...toScreen(originX + s, sign * r));
          ctx.lineTo(...toScreen(originX + s + plume.cellLength * 0.5, 0));
          ctx.stroke();
        }
      }
    }
    if (Number.isFinite(plume.machDisk)) {
      const x = originX + plume.machDisk;
      const r = wasm.mission_engine_plume_boundary(plume.machDisk);
      ctx.strokeStyle = '#e2bc79'; ctx.lineWidth = 2;
      ctx.beginPath(); ctx.moveTo(...toScreen(x, -r)); ctx.lineTo(...toScreen(x, r)); ctx.stroke();
      ctx.fillStyle = '#e2bc79'; ctx.font = '10px system-ui';
      ctx.fillText('Mach disk', ...toScreen(x, r * 1.08));
    }
    // Mixture-ratio track above the chamber, with the efficient band shaded.
    const track = ofTrack();
    const optimum = OPTIMAL[spec.propellant];
    ctx.strokeStyle = '#3d5a4e'; ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.moveTo(...toScreen(track.x0, track.r));
    ctx.lineTo(...toScreen(track.x1, track.r));
    ctx.stroke();
    if (optimum) {
      const span = track.band[1] - track.band[0];
      const a = track.x0 + (optimum[0] - track.band[0]) / span * (track.x1 - track.x0);
      const b = track.x0 + (optimum[1] - track.band[0]) / span * (track.x1 - track.x0);
      ctx.strokeStyle = '#80d9cb'; ctx.lineWidth = 4;
      ctx.beginPath(); ctx.moveTo(...toScreen(a, track.r)); ctx.lineTo(...toScreen(b, track.r)); ctx.stroke();
    }
    ctx.fillStyle = '#849b90'; ctx.font = '10px system-ui';
    const [olx, oly] = toScreen(track.x0, track.r);
    ctx.fillText(`O/F ${spec.of.toFixed(2)} — drag along the track`, olx, oly - 10);
    const [plx, ply] = toScreen(-solution.chamberLength, 0);
    ctx.fillText(`Pc ${(spec.pc / 1e6).toFixed(2)} MPa — drag up/down`, plx + 10, ply - 8);
    // Draggable CAD handles.
    for (const handle of handles()) {
      ctx.beginPath(); ctx.arc(handle.at[0], handle.at[1], 6, 0, Math.PI * 2);
      ctx.fillStyle = drag?.key === handle.key ? '#dbebd3' : '#bbd1b3';
      ctx.fill(); ctx.strokeStyle = '#10201a'; ctx.lineWidth = 1.5; ctx.stroke();
    }
    ctx.fillStyle = '#849b90'; ctx.font = '10px system-ui';
    ctx.fillText(`${REGIMES[plume.regime] ?? '—'} · exit ${(solution.exitPressure / 1000).toFixed(2)} kPa · ambient ${(solution.ambient / 1000).toFixed(2)} kPa · exit Mach ${solution.exitMach.toFixed(2)}`, 14, rect.height - 12);
  }

  const number = (value, digits = 1) => Number.isFinite(value) ? value.toFixed(digits) : '—';

  function render() {
    const status = $('engine-status'), readout = $('engine-readout');
    readout.replaceChildren();
    if (!solution) {
      status.classList.add('failure');
      status.textContent = PROPULSION_ERRORS[String(lastCode)] ?? 'This design has no solution.';
      $('engine-apply').disabled = true;
      $('engine-causal').textContent = 'No solved design to check.';
      return;
    }
    status.classList.toggle('failure', lastCode === -5);
    status.textContent = lastCode === -5
      ? PROPULSION_ERRORS['-5']
      : `${LABELS[spec.propellant]} · expansion ratio ${number(solution.expansion, 1)} · ${REGIMES[plume.regime]}. Drag a handle or edit a control to redesign.`;
    const cells = [
      ['Thrust on this stand', `${number(solution.thrust / 1000, 2)} kN`, false],
      ['Isp on this stand', `${number(solution.isp, 1)} s`, false],
      ['Vacuum thrust', `${number(solution.vacuumThrust / 1000, 2)} kN`, false],
      ['Vacuum Isp', `${number(solution.vacuumIsp, 1)} s`, false],
      ['Mass flow', `${number(solution.massFlow, 3)} kg/s`, false],
      ['Chamber temperature', `${number(solution.tc, 0)} K`, false],
      ['Molar mass', `${number(solution.molar * 1000, 2)} g/mol`, false],
      ['Throat wall', `${number(solution.wallTemperature, 0)} K`, solution.thermalFlag === 1],
      ['Throat heat flux', `${number(solution.heatFlux / 1e6, 1)} MW/m²`, false],
      ['Engine dry mass', `${number(solution.engineMass, 2)} kg`, false],
      ['Side load', Number.isFinite(solution.sideLoad) ? `${number(solution.sideLoad, 3)} kN` : 'attached', Number.isFinite(solution.sideLoad)],
      ['Stage wet mass', `${number(solution.stageWet, 1)} kg`, false],
      ['Stage propellant', `${number(solution.stagePropellant, 1)} kg`, solution.volumeLimited === 1],
      ['Ideal stage delta-v', `${number(solution.stageDeltaV, 0)} m/s`, false],
    ];
    for (const [label, value, warning] of cells) {
      const cell = document.createElement('div');
      if (warning) cell.className = 'is-warning';
      const strong = document.createElement('strong'); strong.textContent = value;
      const span = document.createElement('span'); span.textContent = label;
      cell.append(strong, span); readout.append(cell);
    }
    $('engine-apply').disabled = lastCode === -5 || solution.flightReady !== 1 || isLocked();
    onDiagnose?.();
    // The acronym law applies to the lab too: every registered term must
    // carry its projection, including the ones in these readout labels.
    annotate?.(readout);
    annotate?.(status);
  }

  function refresh() { solve(); }

  function loadBaseline() {
    const code = wasm.mission_engine_baseline();
    if (code !== 1 && code !== -5) { $('engine-status').textContent = 'Load a contract before designing an engine.'; return false; }
    spec = {
      propellant: read(FIELDS.propellant), of: read(FIELDS.of), pc: read(FIELDS.pc),
      throat: read(FIELDS.throatRadius), exit: read(FIELDS.exitRadius),
      length: read(FIELDS.lengthFraction),
    };
    specToControls(spec);
    return solve();
  }

  function setLocked() {
    for (const control of Object.values(controls)) control.disabled = isLocked();
    $('engine-reset').disabled = isLocked();
    $('engine-apply').disabled = isLocked() || !solution || solution.flightReady !== 1;
    canvas.style.cursor = isLocked() ? 'default' : 'crosshair';
  }

  for (const [key, control] of Object.entries(controls)) {
    control.addEventListener('input', () => {
      if (isLocked()) return;
      if (key === 'propellant') {
        // Changing the pair moves the calibrated band; keep the ratio inside
        // it rather than silently solving an extrapolated design.
        const band = BANDS[Number(control.value)];
        const ratio = Number(controls.of.value);
        controls.of.min = String(band[0]); controls.of.max = String(band[1]);
        controls.of.value = String(Math.min(band[1], Math.max(band[0], ratio)));
        const optimum = OPTIMAL[Number(control.value)];
        $('engine-of-band').textContent = `peak efficiency ${optimum[0]}–${optimum[1]}`;
      }
      solve();
    });
  }

  const pointer = event => {
    const rect = canvas.getBoundingClientRect();
    return [event.clientX - rect.left, event.clientY - rect.top];
  };
  canvas.addEventListener('pointerdown', event => {
    if (isLocked() || !solution || event.button !== 0 || !view) return;
    const point = pointer(event);
    const hit = handles()
      .map(h => ({ h, d: Math.hypot(point[0] - h.at[0], point[1] - h.at[1]) }))
      .filter(x => x.d < 12)
      .sort((a, b) => a.d - b.d)[0]?.h;
    if (!hit) return;
    drag = { pointer: event.pointerId, key: hit.key, axis: hit.axis, start: point, initial: { ...spec } };
    governor.begin();
    canvas.setPointerCapture(event.pointerId);
  });
  canvas.addEventListener('pointermove', event => {
    if (!drag || drag.pointer !== event.pointerId || !view) return;
    const point = pointer(event);
    if (drag.axis === 'of') {
      const track = ofTrack();
      const [x] = toWorld(point[0], point[1]);
      const frac = Math.min(1, Math.max(0, (x - track.x0) / (track.x1 - track.x0)));
      const of = track.band[0] + frac * (track.band[1] - track.band[0]);
      controls.of.value = String(+of.toFixed(3));
    } else if (drag.axis === 'pc') {
      // Logarithmic: pressure spans two decades, so a linear drag would make
      // the low end unreachable. Anchored at the press so it does not jump.
      const dy = drag.start[1] - point[1];
      const decades = dy / 120;
      const pc = drag.initial.pc * Math.pow(10, decades);
      controls.pc.value = String(Math.max(0.2, Math.min(40, +(pc / 1e6).toFixed(3))));
    } else if (drag.axis === 'r') {
      const [, radius] = toWorld(point[0], point[1]);
      const value = Math.abs(radius);
      if (drag.key === 'throat') {
        controls.throat.value = String(Math.max(5, Math.min(1000, +(value * 1000).toFixed(2))));
      } else {
        controls.exit.value = String(Math.max(5, Math.min(4000, +(value * 1000).toFixed(2))));
      }
    } else {
      const [x] = toWorld(point[0], point[1]);
      // length = fraction * (re - rt) / tan(15°); invert for the fraction.
      const rise = Math.max(1e-4, Number(controls.exit.value) / 1000 - Number(controls.throat.value) / 1000);
      const fraction = x * Math.tan(15 * Math.PI / 180) / rise;
      controls.length.value = String(Math.max(0.4, Math.min(1.2, +fraction.toFixed(3))));
    }
    solve();
    governor.move();
  });
  function endDrag(event) {
    if (!drag || drag.pointer !== event.pointerId) return;
    drag = null; governor.end(); governor.invalidate(false);
  }
  canvas.addEventListener('pointerup', endDrag);
  canvas.addEventListener('pointercancel', endDrag);
  canvas.addEventListener('lostpointercapture', endDrag);

  $('engine-apply').addEventListener('click', () => {
    if (isLocked() || !solution) return;
    // Solve at vacuum before fitting: the stage flies in vacuum, and the
    // stand altitude is only a diagnostic.
    const ambient = solution.ambient;
    if (ambient !== 0) { controls.altitude.value = '100'; solve(); }
    const code = wasm.mission_engine_apply();
    if (code !== 1) {
      $('engine-status').classList.add('failure');
      $('engine-status').textContent = code === -2
        ? 'ThermalLimitExceeded: the throat wall is over its 1350 K limit. Lower chamber pressure or open the throat.'
        : code === -1
          ? 'This design cannot fly the contract: it produces no usable thrust.'
          : 'Load a contract before fitting an engine.';
      return;
    }
    onApply?.();
  });
  $('engine-reset').addEventListener('click', () => { if (!isLocked()) { loadBaseline(); onApply?.(true); } });

  new ResizeObserver(() => { dirty = true; governor.invalidate(false); }).observe(canvas.parentElement);

  return {
    refresh, loadBaseline, setLocked, solve,
    diagnostics: () => ({
      draws, solves, code: lastCode,
      spec: spec ? { ...spec } : null,
      solution: solution ? { ...solution } : null,
      plume: plume ? { ...plume } : null,
      stations: stations.length,
      raster: !!raster,
      pending: governor.frame !== null,
      handles: handles().map(h => ({ key: h.key, at: [...h.at] })),
    }),
  };
}
