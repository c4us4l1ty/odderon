// Shared canvas typography. Every label painted into a canvas uses the same
// bundled faces as the DOM, so a scene can never fall back to a system UI font
// and quietly stop looking like an instrument. Kept separate from hud.css
// because a canvas cannot inherit a CSS font stack.
export const CANVAS_MONO = '"Odderon Data","Martian Mono",ui-monospace,monospace';
export const CANVAS_DISPLAY = '"Odderon Gauge",Oxanium,ui-sans-serif,sans-serif';
export const CANVAS_PANEL = '"Odderon Panel",Rajdhani,system-ui,sans-serif';

/// Paint a readout label: monospaced, tracked, upper case, in one place so the
/// tracking is never set by hand at a call site.
export function paintLabel(ctx, text, x, y, {
  color = '#849b90', size = 10, weight = 400, tracking = '0px', align = 'left',
} = {}) {
  ctx.save();
  ctx.fillStyle = color;
  ctx.font = `${weight} ${size}px ${CANVAS_MONO}`;
  if ('letterSpacing' in ctx) ctx.letterSpacing = tracking;
  ctx.textAlign = align;
  ctx.fillText(text, x, y);
  ctx.restore();
}

/// The faces must be resident before the first paint, or the browser draws the
/// fallback once and caches a blurry frame. This resolves when they are ready.
export async function loadCanvasFaces() {
  if (!document.fonts?.load) return;
  await Promise.all([
    document.fonts.load(`400 10px ${CANVAS_MONO}`),
    document.fonts.load(`600 10px ${CANVAS_MONO}`),
    document.fonts.load(`500 11px ${CANVAS_DISPLAY}`),
    document.fonts.load(`600 11px ${CANVAS_DISPLAY}`),
    document.fonts.load(`700 12px ${CANVAS_PANEL}`),
  ]).catch(() => {});
}
