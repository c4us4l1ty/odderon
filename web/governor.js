// A demand scheduler: one outstanding frame at most, no idle animation loop.
export class Governor {
  constructor(draw, clock = window) { this.draw = draw; this.clock = clock; this.frame = null; this.timer = null; this.dragging = false; this.suspended = false; }
  invalidate() {
    if (this.suspended || this.frame !== null) return;
    this.frame = this.clock.requestAnimationFrame(() => { this.frame = null; if (!this.suspended) this.draw(this.dragging ? 'interactive' : 'final'); });
  }
  begin() { this.dragging = true; this.cancelTimer(); this.invalidate(); }
  move() { if (this.dragging) this.invalidate(); }
  end() {
    if (!this.dragging) return;
    this.dragging = false;
    // If a low-resolution frame is still pending, let it render as interactive;
    // the settled full-resolution render happens only after the debounce.
    if (this.frame !== null) { this.clock.cancelAnimationFrame(this.frame); this.frame = null; }
    this.cancelTimer();
    this.timer = this.clock.setTimeout(() => { this.timer = null; this.invalidate(); }, 50);
  }
  cancelTimer() { if (this.timer !== null) { this.clock.clearTimeout(this.timer); this.timer = null; } }
  suspend() { this.suspended = true; this.dragging = false; this.cancelTimer(); if (this.frame !== null) this.clock.cancelAnimationFrame(this.frame); this.frame = null; }
  resume() { this.suspended = false; this.invalidate(); }
}
