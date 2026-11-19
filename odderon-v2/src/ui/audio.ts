/* Small procedural audio bed: engine rumble, pump hum and interface blips.
 * No sample files, so the build stays self-contained. AudioContext creation is
 * deferred until the first user gesture, as browsers require. */

export class Audio {
  private ctx: AudioContext | null = null;
  private master: GainNode | null = null;
  private engineGain: GainNode | null = null;
  private engineFilter: BiquadFilterNode | null = null;
  private rumbleGain: GainNode | null = null;
  private enabled = true;
  private started = false;

  setEnabled(value: boolean): void {
    this.enabled = value;
    if (this.master && this.ctx) {
      this.master.gain.setTargetAtTime(value ? 0.7 : 0, this.ctx.currentTime, 0.08);
    }
  }

  /** Must be called from inside a user gesture the first time. */
  unlock(): void {
    if (!this.enabled) return;
    try {
      if (!this.ctx) this.build();
      void this.ctx?.resume();
    } catch {
      /* audio is optional */
    }
  }

  private build(): void {
    const Ctor = window.AudioContext ?? (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
    if (!Ctor) return;
    const ctx = new Ctor();
    this.ctx = ctx;

    const master = ctx.createGain();
    master.gain.value = this.enabled ? 0.7 : 0;
    master.connect(ctx.destination);
    this.master = master;

    // White noise buffer for the engine.
    const length = ctx.sampleRate * 2;
    const buffer = ctx.createBuffer(1, length, ctx.sampleRate);
    const data = buffer.getChannelData(0);
    let last = 0;
    for (let i = 0; i < length; i++) {
      const white = Math.random() * 2 - 1;
      last = (last + 0.02 * white) / 1.02;
      data[i] = last * 3.2;
    }
    const noise = ctx.createBufferSource();
    noise.buffer = buffer;
    noise.loop = true;

    const filter = ctx.createBiquadFilter();
    filter.type = "lowpass";
    filter.frequency.value = 320;
    filter.Q.value = 0.8;
    this.engineFilter = filter;

    const gain = ctx.createGain();
    gain.gain.value = 0;
    this.engineGain = gain;

    noise.connect(filter);
    filter.connect(gain);
    gain.connect(master);
    noise.start();

    // Low rumble oscillator.
    const osc = ctx.createOscillator();
    osc.type = "sine";
    osc.frequency.value = 42;
    const rumble = ctx.createGain();
    rumble.gain.value = 0;
    this.rumbleGain = rumble;
    osc.connect(rumble);
    rumble.connect(master);
    osc.start();

    this.started = true;
  }

  setThrottle(throttle: number): void {
    if (!this.ctx || !this.engineGain || !this.engineFilter || !this.rumbleGain) return;
    const t = Math.max(0, Math.min(1, throttle));
    const now = this.ctx.currentTime;
    this.engineGain.gain.setTargetAtTime(t * 0.26, now, 0.12);
    this.engineFilter.frequency.setTargetAtTime(280 + t * 900, now, 0.18);
    this.rumbleGain.gain.setTargetAtTime(t * 0.12, now, 0.2);
  }

  blip(freq = 880, ms = 45, type: OscillatorType = "square", gain = 0.05): void {
    if (!this.ctx || !this.enabled || !this.started || !this.master) return;
    const ctx = this.ctx;
    const osc = ctx.createOscillator();
    const g = ctx.createGain();
    osc.type = type;
    osc.frequency.value = freq;
    g.gain.setValueAtTime(gain, ctx.currentTime);
    g.gain.exponentialRampToValueAtTime(0.0001, ctx.currentTime + ms / 1000);
    osc.connect(g);
    g.connect(this.master);
    osc.start();
    osc.stop(ctx.currentTime + ms / 1000 + 0.02);
  }

  click(): void {
    this.blip(1320, 35, "square", 0.035);
  }

  warn(): void {
    this.blip(320, 140, "sawtooth", 0.05);
  }

  success(): void {
    this.blip(660, 90, "triangle", 0.06);
    window.setTimeout(() => this.blip(990, 140, "triangle", 0.05), 110);
  }
}
