// Apply the quality reduction after the final-size cap, so even large or
// high-DPI viewports save three quarters of the pixel work while dragging.
export function renderSize(width, height, dpr, quality) {
  const scale = Math.min(dpr || 1, 1.5, 2048 / Math.max(width, height)) * (quality === 'interactive' ? 0.5 : 1);
  return [Math.max(1, Math.round(width * scale)), Math.max(1, Math.round(height * scale))];
}
