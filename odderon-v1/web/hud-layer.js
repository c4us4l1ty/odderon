// Canvas HUD symbology. The DOM is the machine's frame; these drawers are the
// glass inside it: corner brackets, engraved tapes, target rings, prograde
// markers and horizon rails, painted over the live instrument canvases.
//
// A canvas cannot inherit a CSS font stack, so every face comes from
// `glyphs.js`. A drawer never hard-codes a system font: a fallback here is
// invisible to the DOM audit and makes a solved scene look like a web chart.

import { CANVAS_MONO, CANVAS_DISPLAY } from './glyphs.js';

const CY = 'rgba(99,226,244,';
const AM = 'rgba(255,180,63,';
const INK = 'rgba(169,190,198,';

/** Corner brackets and a faint inner vignette: the instrument bezel. */
export function bezel(ctx, rect, { color = CY, inset = 6, length = 16, alpha = .5 } = {}) {
  const { width: w, height: h } = rect;
  ctx.save();
  ctx.strokeStyle = `${color}${alpha})`;
  ctx.lineWidth = 1;
  const x0 = inset + .5, y0 = inset + .5, x1 = w - inset - .5, y1 = h - inset - .5;
  const arms = [[x0, y0, 1, 1], [x1, y0, -1, 1], [x0, y1, 1, -1], [x1, y1, -1, -1]];
  for (const [x, y, dx, dy] of arms) {
    ctx.beginPath();
    ctx.moveTo(x + dx * length, y); ctx.lineTo(x, y); ctx.lineTo(x, y + dy * length);
    ctx.stroke();
  }
  // A second, shorter tick set inside each bracket: machined, not decorative.
  ctx.strokeStyle = `${color}${alpha * .4})`;
  for (const [x, y, dx, dy] of arms) {
    ctx.beginPath();
    ctx.moveTo(x + dx * (length + 5), y); ctx.lineTo(x + dx * (length + 5), y + dy * 4);
    ctx.stroke();
  }
  ctx.restore();
}

/** A faint glass wash: an edge vignette drawn INTO the instrument frame, not a
    composited layer above it. Nothing here may sit over a live canvas. */
export function glass(ctx, rect, { alpha = .12 } = {}) {
  const { width: w, height: h } = rect;
  ctx.save();
  const grad = ctx.createRadialGradient(w / 2, h / 2, Math.min(w, h) * .2, w / 2, h / 2, Math.max(w, h) * .72);
  grad.addColorStop(0, 'rgba(0,0,0,0)');
  grad.addColorStop(1, `rgba(0,4,7,${alpha * 2.4})`);
  ctx.fillStyle = grad;
  ctx.fillRect(0, 0, w, h);
  ctx.restore();
}

/** Tracked monospaced caption. */
function caption(ctx, text, x, y, { color = `${INK}.85)`, size = 9, align = 'left', weight = 400, tracking = '0px', face = CANVAS_MONO } = {}) {
  ctx.save();
  ctx.fillStyle = color;
  ctx.font = `${weight} ${size}px ${face}`;
  if ('letterSpacing' in ctx) ctx.letterSpacing = tracking;
  ctx.textAlign = align;
  ctx.fillText(text, x, y);
  ctx.restore();
}

/** A vertical altitude tape: engraved ticks, a lit rail, and apsis tags.
    Deliberately unnumbered — the readouts below carry the values, so the tape
    stays a reference scale instead of a second, competing number column. */
export function altitudeTape(ctx, rect, { marks = [], side = 'right', unit = 'ALT' } = {}) {
  const { width: w, height: h } = rect;
  const top = 46, bottom = h - 74;
  const x = side === 'right' ? w - 30 : 30;
  const dir = side === 'right' ? -1 : 1;
  ctx.save();
  // Rail with a lit inner edge.
  ctx.strokeStyle = `${INK}.3)`;
  ctx.lineWidth = 1;
  ctx.beginPath(); ctx.moveTo(x + .5, top); ctx.lineTo(x + .5, bottom); ctx.stroke();
  ctx.strokeStyle = `${CY}.18)`;
  ctx.beginPath(); ctx.moveTo(x + dir * 3.5, top); ctx.lineTo(x + dir * 3.5, bottom); ctx.stroke();
  // Ticks: long every eighth, short between.
  for (let i = 0; i <= 32; i++) {
    const y = top + (bottom - top) * i / 32;
    const len = i % 8 === 0 ? 9 : i % 4 === 0 ? 6 : 3;
    ctx.strokeStyle = i % 8 === 0 ? `${INK}.5)` : `${INK}.26)`;
    ctx.beginPath(); ctx.moveTo(x + .5, y); ctx.lineTo(x + .5 + dir * len, y); ctx.stroke();
  }
  // Apsis tags: a filled pennant pointing at the rail, label outside it.
  for (const mark of marks) {
    const y = top + (bottom - top) * Math.max(0, Math.min(1, mark.value));
    const color = mark.caution ? AM : mark.accent ? CY : `${INK}.9)`;
    ctx.fillStyle = color;
    ctx.beginPath();
    ctx.moveTo(x + .5, y);
    ctx.lineTo(x + dir * 8, y - 5);
    ctx.lineTo(x + dir * 8, y + 5);
    ctx.closePath(); ctx.fill();
    const tx = x + dir * 12;
    caption(ctx, mark.label, tx, y - 1, { color, size: 9, weight: 600, align: side === 'right' ? 'right' : 'left', tracking: '.1em' });
    caption(ctx, mark.readout, tx, y + 9, { color: `${INK}.8)`, size: 9, align: side === 'right' ? 'right' : 'left' });
  }
  caption(ctx, unit, x + dir * 12, top - 9, { align: side === 'right' ? 'right' : 'left', color: `${INK}.55)`, tracking: '.16em', size: 9 });
  ctx.restore();
}

/** A centre cross and a slow-axis reticle at a projected point. */
export function reticle(ctx, x, y, { color = CY, size = 13, gap = 4 } = {}) {
  ctx.save();
  ctx.strokeStyle = `${color}.9)`;
  ctx.lineWidth = 1;
  for (const [dx, dy] of [[1, 0], [-1, 0], [0, 1], [0, -1]]) {
    ctx.beginPath();
    ctx.moveTo(x + dx * gap, y + dy * gap);
    ctx.lineTo(x + dx * size, y + dy * size);
    ctx.stroke();
  }
  ctx.strokeStyle = `${color}.35)`;
  ctx.beginPath(); ctx.arc(x, y, size + 5, 0, Math.PI * 2); ctx.stroke();
  ctx.restore();
}

/** The prograde marker: a circle with three fins, as on a navball. */
export function prograde(ctx, x, y, { color = CY, r = 6 } = {}) {
  ctx.save();
  ctx.strokeStyle = `${color}.95)`;
  ctx.fillStyle = `${color}.95)`;
  ctx.lineWidth = 1.4;
  ctx.beginPath(); ctx.arc(x, y, r, 0, Math.PI * 2); ctx.stroke();
  for (const a of [-Math.PI / 2, Math.PI / 6, Math.PI * 5 / 6]) {
    const c = Math.cos(a), s = Math.sin(a);
    ctx.beginPath();
    ctx.moveTo(x + c * (r + 1), y + s * (r + 1));
    ctx.lineTo(x + c * (r + 7), y + s * (r + 7));
    ctx.stroke();
  }
  ctx.restore();
}

/** A target ring: the shape of the objective on the glass. */
export function targetRing(ctx, x, y, radius, { color = AM, label = 'TGT', dashed = true } = {}) {
  if (!Number.isFinite(radius) || radius < 2) return;
  ctx.save();
  ctx.strokeStyle = `${color}.75)`;
  ctx.lineWidth = 1.2;
  if (dashed) ctx.setLineDash([6, 5]);
  ctx.beginPath(); ctx.arc(x, y, radius, 0, Math.PI * 2); ctx.stroke();
  ctx.setLineDash([]);
  // Corner ticks on the ring so it reads as a gun-camera box, not a chart.
  for (let i = 0; i < 4; i++) {
    const a = i * Math.PI / 2 + Math.PI / 4;
    const c = Math.cos(a), s = Math.sin(a);
    ctx.beginPath();
    ctx.moveTo(x + c * (radius - 6), y + s * (radius - 6));
    ctx.lineTo(x + c * (radius + 6), y + s * (radius + 6));
    ctx.stroke();
  }
  if (label) caption(ctx, label, x + radius * .72, y - radius * .72, { color: `${color}.95)`, size: 9, weight: 600, tracking: '.1em' });
  ctx.restore();
}

/** A bottom rail with numbered node tags: the burn schedule at a glance. */
export function nodeRail(ctx, rect, { nodes, selected, duration }) {
  const { width: w, height: h } = rect;
  const x0 = 26, x1 = w - 26, y = h - 13;
  ctx.save();
  ctx.strokeStyle = `${INK}.28)`;
  ctx.lineWidth = 1;
  ctx.beginPath(); ctx.moveTo(x0, y + .5); ctx.lineTo(x1, y + .5); ctx.stroke();
  for (let i = 0; i <= 12; i++) {
    const x = x0 + (x1 - x0) * i / 12;
    ctx.beginPath(); ctx.moveTo(x, y + .5); ctx.lineTo(x, y + (i % 3 === 0 ? 5.5 : 3)); ctx.stroke();
  }
  for (const node of nodes) {
    const t = Math.max(0, Math.min(1, node.time / duration));
    const x = x0 + (x1 - x0) * t;
    const active = node.index === selected;
    ctx.fillStyle = active ? `${AM}.95)` : `${INK}.8)`;
    ctx.beginPath();
    ctx.moveTo(x, y - 7); ctx.lineTo(x + 5, y - 3.5); ctx.lineTo(x, y); ctx.lineTo(x - 5, y - 3.5);
    ctx.closePath(); ctx.fill();
    if (active) { ctx.strokeStyle = `${AM}.6)`; ctx.beginPath(); ctx.moveTo(x, y - 7); ctx.lineTo(x, y + 2); ctx.stroke(); }
  }
  caption(ctx, 'BURN SCHEDULE', x0, y - 11, { size: 9, tracking: '.14em', color: `${INK}.6)` });
  caption(ctx, fmtClock(duration), x1, y - 11, { size: 9, align: 'right', color: `${INK}.6)` });
  ctx.restore();
}

export function fmtClock(seconds) {
  if (!Number.isFinite(seconds)) return '—';
  const s = Math.max(0, Math.round(seconds));
  const hh = Math.floor(s / 3600), mm = Math.floor((s % 3600) / 60), ss = s % 60;
  return `${String(hh).padStart(2, '0')}:${String(mm).padStart(2, '0')}:${String(ss).padStart(2, '0')}`;
}

/** A corner data block: a caption, then the value with its unit leading it.
    A soft scrim behind it keeps the stamp legible over orbit lines. */
export function cornerReadout(ctx, rect, { corner, caption: label, value, unit = '', accent = CY }) {
  const { width: w, height: h } = rect;
  const pad = 12;
  const right = corner.includes('right'), bottom = corner.includes('bottom');
  const x = right ? w - pad : pad;
  const y = bottom ? h - pad : pad;
  ctx.save();
  ctx.textAlign = right ? 'right' : 'left';
  // Scrim: a soft wedge of ink so the stamp survives a bright path under it.
  const grad = right
    ? ctx.createLinearGradient(x - 176, 0, x, 0)
    : ctx.createLinearGradient(x + 176, 0, x, 0);
  grad.addColorStop(0, 'rgba(2,6,9,0)');
  grad.addColorStop(1, 'rgba(2,6,9,.72)');
  ctx.fillStyle = grad;
  const top = bottom ? y - 34 : y - 2;
  ctx.fillRect(right ? x - 176 : x, Math.max(0, top), 176, 34);

  ctx.font = `400 9px ${CANVAS_MONO}`;
  if ('letterSpacing' in ctx) ctx.letterSpacing = '.16em';
  ctx.fillStyle = `${INK}.6)`;
  ctx.fillText(label, x, bottom ? y - 20 : y + 7);

  ctx.font = `600 16px ${CANVAS_DISPLAY}`;
  if ('letterSpacing' in ctx) ctx.letterSpacing = '.03em';
  const valueText = String(value);
  ctx.fillStyle = 'rgba(238,246,248,1)';
  ctx.fillText(valueText, x, bottom ? y - 3 : y + 24);
  if (unit) {
    const valueWidth = ctx.measureText(valueText).width;
    ctx.font = `400 9px ${CANVAS_MONO}`;
    if ('letterSpacing' in ctx) ctx.letterSpacing = '.1em';
    ctx.fillStyle = `${accent}.95)`;
    ctx.textAlign = right ? 'right' : 'left';
    ctx.fillText(unit, right ? x - valueWidth - 6 : x + valueWidth + 6, bottom ? y - 4 : y + 23);
  }
  ctx.restore();
}

/** A horizontal scale bar with its length labelled, as on a camera feed. */
export function scaleBar(ctx, rect, { caption: label = 'SCALE', fractions = 4, unit = '' } = {}) {
  const { width: w, height: h } = rect;
  const x = 16, y = h - 16, len = Math.min(120, w * .16);
  ctx.save();
  ctx.strokeStyle = `${INK}.7)`;
  ctx.lineWidth = 1;
  ctx.beginPath();
  ctx.moveTo(x, y); ctx.lineTo(x + len, y);
  ctx.moveTo(x + .5, y - 4); ctx.lineTo(x + .5, y + 2);
  ctx.moveTo(x + len - .5, y - 4); ctx.lineTo(x + len - .5, y + 2);
  for (let i = 1; i < fractions; i++) {
    const xm = x + len * i / fractions;
    ctx.moveTo(xm, y); ctx.lineTo(xm, y - 2);
  }
  ctx.stroke();
  caption(ctx, label, x, y - 8, { size: 9, tracking: '.14em', color: `${INK}.6)` });
  ctx.restore();
}

/** The frame a tracking instrument wears: bezel, glass, corner readouts. */
export function trackingFrame(ctx, rect) {
  glass(ctx, rect, { alpha: .1 });
  bezel(ctx, rect, { alpha: .42 });
}

/**
 * The console frame: the outline of the display unit itself — a recessed edge,
 * four corner brackets and a serial stamp. Painted into the instrument canvas
 * (never as a DOM layer over it) so the glass keeps the machine's frame while
 * the GPU stays idle when the game is.
 */
export function consoleFrame(ctx, rect, { label = '' } = {}) {
  const { width: w, height: h } = rect;
  ctx.save();
  // A recessed edge: dark outer line, then a lit inner one.
  ctx.strokeStyle = 'rgba(0,0,0,.8)';
  ctx.lineWidth = 2;
  ctx.strokeRect(1, 1, w - 2, h - 2);
  ctx.strokeStyle = 'rgba(157,196,208,.18)';
  ctx.lineWidth = 1;
  ctx.strokeRect(3.5, 3.5, w - 7, h - 7);
  // Corner brackets, heavier than the edge, each with a machined notch.
  const arm = 24, pad = 8;
  ctx.strokeStyle = 'rgba(182,204,212,.46)';
  ctx.lineWidth = 1;
  for (const [x, y, dx, dy] of [[pad, pad, 1, 1], [w - pad, pad, -1, 1], [pad, h - pad, 1, -1], [w - pad, h - pad, -1, -1]]) {
    ctx.beginPath();
    ctx.moveTo(x + dx * arm, y); ctx.lineTo(x, y); ctx.lineTo(x, y + dy * arm);
    ctx.stroke();
    ctx.beginPath();
    ctx.moveTo(x + dx * (arm + 7), y); ctx.lineTo(x + dx * (arm + 7), y + dy * 5);
    ctx.stroke();
  }
  // Serial stamp on the top edge, clear of both bottom corners (the plan and
  // the tape print into those).
  if (label) {
    ctx.font = '400 9px ui-monospace, monospace';
    ctx.textAlign = 'center';
    ctx.fillStyle = 'rgba(169,190,198,.32)';
    ctx.fillText(label, w / 2, 16);
  }
  ctx.restore();
}
