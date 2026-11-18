// Instrument layer for the Flight Deck.
//
// The DOM gives the console its frame; a flight computer's glass also carries
// symbology that CSS cannot paint: an altitude tape, a range ring, a burn
// schedule rail, a reticle on the selected burn and stamped corner readouts.
// This module paints those on `#plan-hud`, the overlay canvas above the orbit
// instrument.
//
// Two rules:
//   1. Demand-driven. It repaints only when mission state, a field or the
//      frame changes. There is no animation loop, so the idle gate keeps
//      seeing zero scheduled frames.
//   2. No invented numbers. Every value comes from the Rust planner through
//      `window.missionDiagnostics` or from text the planner has already
//      written into the console.

import { loadCanvasFaces, CANVAS_MONO } from './glyphs.js';
import { consoleFrame, altitudeTape, nodeRail, reticle, cornerReadout } from './hud-layer.js';

const $ = id => document.getElementById(id);
const parse = text => {
  const match = /-?[\d,]+(?:\.\d+)?/.exec(text ?? '');
  return match ? Number(match[0].replace(/,/g, '')) : NaN;
};
const read = id => $(id)?.textContent ?? '';

let canvas = null, ctx = null, frame = null;
let lastPlan = '';

/** Log-altitude position so one tape serves a 260 km orbit and a cislunar arc. */
function altPos(alt, lo, hi) {
  const f = value => Math.log10(1 + Math.max(0, value) / 60);
  const top = f(hi), bottom = f(lo);
  if (!(top > bottom)) return .5;
  return 1 - (f(alt) - bottom) / (top - bottom);
}

/**
 * The range ring: a machined circle of bearing marks around the body, drawn in
 * screen space so it belongs to the instrument and not to the orbit solution.
 * It is engraved furniture, never a plotted value.
 */
function rangeRing(ctx, w, h) {
  const cx = w / 2, cy = h / 2;
  // The orbit instrument projects the body into a circle whose radius is a
  // fixed fraction of the glass height; the ring sits just outside it.
  const radius = Math.min(h * .41, w * .44);
  ctx.save();
  ctx.strokeStyle = 'rgba(157,196,208,.16)';
  ctx.lineWidth = 1;
  ctx.beginPath(); ctx.arc(cx, cy, radius, 0, Math.PI * 2); ctx.stroke();
  // Long bearing marks every 30°, short ticks every 10°.
  for (let i = 0; i < 36; i++) {
    const a = -Math.PI / 2 + i * Math.PI / 18;
    const long = i % 3 === 0;
    const r0 = radius + 3, r1 = radius + (long ? 11 : 6);
    ctx.strokeStyle = long ? 'rgba(157,196,208,.38)' : 'rgba(157,196,208,.2)';
    ctx.beginPath();
    ctx.moveTo(cx + Math.cos(a) * r0, cy + Math.sin(a) * r0);
    ctx.lineTo(cx + Math.cos(a) * r1, cy + Math.sin(a) * r1);
    ctx.stroke();
  }
  // Cardinal quadrant marks: heavier, with a cross tick.
  for (let i = 0; i < 4; i++) {
    const a = -Math.PI / 2 + i * Math.PI / 2;
    const r0 = radius - 2, r1 = radius + 16;
    ctx.strokeStyle = 'rgba(99,226,244,.4)';
    ctx.beginPath();
    ctx.moveTo(cx + Math.cos(a) * r0, cy + Math.sin(a) * r0);
    ctx.lineTo(cx + Math.cos(a) * r1, cy + Math.sin(a) * r1);
    ctx.stroke();
  }
  ctx.restore();
}

function paint() {
  if (!canvas || !ctx || !canvas.isConnected) return;
  const host = canvas.parentElement;
  const rect = host.getBoundingClientRect();
  if (rect.width < 60 || rect.height < 60) return;
  const dpr = Math.min(window.devicePixelRatio || 1, 2);
  const w = Math.round(rect.width), height = Math.round(rect.height);
  if (canvas.width !== Math.round(w * dpr) || canvas.height !== Math.round(height * dpr)) {
    canvas.width = Math.round(w * dpr);
    canvas.height = Math.round(height * dpr);
  }
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.clearRect(0, 0, w, height);
  const glassRect = { width: w, height };

  // Symbology only: the plate's vignette is CSS on the instrument bezel, so
  // this canvas never paints a full-area fill over the orbit solution.
  rangeRing(ctx, w, height);
  // The plate's serial is a DOM strip above the glass; only the frame's own
  // edge furniture belongs on this canvas.
  consoleFrame(ctx, glassRect);

  const diag = window.missionDiagnostics;
  const nodes = diag?.nodes ?? [];
  const selected = Number($('node-select')?.value ?? -1);

  // ---- altitude tape. The apsides are the values the plan already publishes.
  const apo = parse(read('predict-apo'));
  const peri = parse(read('predict-peri'));
  const marks = [];
  if (Number.isFinite(apo) || Number.isFinite(peri)) {
    const hi = Math.max(apo || 0, peri || 0, 600) * 1.3;
    if (Number.isFinite(apo)) marks.push({ value: altPos(apo, 60, hi), label: 'AP', readout: `${apo.toFixed(0)} KM`, accent: true });
    if (Number.isFinite(peri)) marks.push({ value: altPos(peri, 60, hi), label: 'PE', readout: `${peri.toFixed(0)} KM`, caution: peri < 180 });
  }
  altitudeTape(ctx, glassRect, { marks, side: 'right', unit: 'ALT' });

  // ---- burn schedule rail along the foot of the glass.
  const duration = Number($('node-time')?.max) || 0;
  if (duration > 0 && nodes.length) {
    nodeRail(ctx, glassRect, { duration, selected, nodes: nodes.map((node, index) => ({ time: node.time, index })) });
  }

  // ---- reticle on the selected burn, in the instrument's own projection.
  const marker = diag?.nodeMarker;
  if (marker && Array.isArray(marker)) {
    reticle(ctx, marker[0], marker[1], { color: 'rgba(255,180,63,', size: 15, gap: 6 });
  }

  // ---- corner stamps: the plan's own arithmetic, never a second estimate.
  const dvTotal = nodes.reduce((sum, node) => sum + Math.hypot(...node.dv), 0);
  cornerReadout(ctx, glassRect, {
    corner: 'bottom-left', caption: 'COMMANDED ΔV',
    value: nodes.length ? dvTotal.toFixed(1) : '—', unit: 'M/S',
  });
  const code = read('plan-check-code');
  cornerReadout(ctx, glassRect, {
    corner: 'bottom-right', caption: 'PLAN',
    value: code && code !== '—' ? code : 'READY', unit: `${nodes.length} BURN${nodes.length === 1 ? '' : 'S'}`,
  });
}

/** Repaint at most once per frame, and only for reasons that can matter. */
function schedule() {
  if (frame !== null) return;
  frame = requestAnimationFrame(() => { frame = null; paint(); });
}

function observe(node, init) {
  if (!node) return;
  new MutationObserver(() => schedule()).observe(node, init);
}

function attach() {
  canvas = $('plan-hud');
  if (!canvas) return;
  ctx = canvas.getContext('2d');
  if (!ctx) return;
  new ResizeObserver(() => schedule()).observe(canvas.parentElement);
  observe($('station-surface'), { attributes: true, attributeFilter: ['data-mode'] });
  for (const id of ['plan-status', 'check-dv', 'check-fuel', 'check-pad', 'check-nodes', 'plan-check-code', 'node-select']) {
    observe($(id), { childList: true, characterData: true, subtree: true, attributes: true });
  }
  for (const id of ['predict-apo', 'predict-peri', 'node-time', 'node-t', 'node-n', 'node-w']) {
    $(id)?.addEventListener('input', () => schedule());
    $(id)?.addEventListener('change', () => schedule());
  }
  // The planner draws on the same rAF cycle; one extra pass after a click on
  // the path keeps the reticle attached to a burn the player just placed.
  $('plan-canvas')?.addEventListener('pointerup', () => setTimeout(() => schedule(), 0));
  for (const id of ['add-node', 'remove-node', 'example', 'prefill', 'execute', 'unlock']) {
    $(id)?.addEventListener('click', () => setTimeout(() => schedule(), 0));
  }
  document.addEventListener('mission:plan', () => schedule());
  document.addEventListener('mission:flown', () => schedule());
  window.addEventListener('resize', () => schedule());
  loadCanvasFaces().then(() => schedule());
  schedule();
}

if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', attach, { once: true });
else attach();
