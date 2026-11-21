// Zero-Asset WebAudio Physical Sound Synthesizer for Odderon (PERFORMANCE.md I8 & IMMERSION.md N1).
// Generates all sound waves mathematically at 48 kHz directly from Rust kernel physics numbers:
// - Window AC compressor 60/120 Hz hum + evaporator louver airflow
// - Outdoor coastal summer cicada chorus attenuated by physical window sash opening
// - Electric kettle nucleate boiling hiss (75 C -> 100 C)
// - < 8ms tactile mechanical switch/valve/marker impulses
// - 343 m/s distant rocket static-fire sub-bass shockwave + 18 Hz glass-pane rattle

export class PhysicalAudioEngine {
  constructor() {
    this.ctx = null;
    this.started = false;
    this.muted = false;
    this.master = null;
    this.acHumOsc = null;
    this.acHarmOsc = null;
    this.acGain = null;
    this.airGain = null;
    this.cicadaGain = null;
    this.cicadaFilter = null;
    this.kettleGain = null;
    this.noiseBuffer = null;
  }

  ensureStarted() {
    if (this.started) {
      if (this.ctx && this.ctx.state === 'suspended') {
        this.ctx.resume();
      }
      return;
    }
    const AudioCtx = window.AudioContext || window.webkitAudioContext;
    if (!AudioCtx) return;

    const ctx = new AudioCtx();
    this.ctx = ctx;
    this.started = true;

    const master = ctx.createGain();
    master.gain.value = 0.55;
    master.connect(ctx.destination);
    this.master = master;

    // Shared 2-second white-noise buffer generated mathematically once at boot
    const sr = ctx.sampleRate;
    const buf = ctx.createBuffer(1, sr * 2, sr);
    const data = buf.getChannelData(0);
    let seed = 0x12345678;
    for (let i = 0; i < data.length; i++) {
      seed = (seed * 1664525 + 1013904223) | 0;
      data[i] = ((seed >>> 8) / 8388608.0) - 1.0;
    }
    this.noiseBuffer = buf;

    // 1. Window AC Compressor (60 Hz + 120 Hz motor harmonics)
    const acGain = ctx.createGain();
    acGain.gain.value = 0.0;
    acGain.connect(master);
    this.acGain = acGain;

    const acHum = ctx.createOscillator();
    acHum.type = 'sine';
    acHum.frequency.value = 59.5;
    acHum.connect(acGain);
    acHum.start();
    this.acHumOsc = acHum;

    const acHarm = ctx.createOscillator();
    acHarm.type = 'triangle';
    acHarm.frequency.value = 119.0;
    const harmAtten = ctx.createGain();
    harmAtten.gain.value = 0.35;
    acHarm.connect(harmAtten);
    harmAtten.connect(acGain);
    acHarm.start();
    this.acHarmOsc = acHarm;

    // Evaporator airflow filtered noise
    const airSrc = ctx.createBufferSource();
    airSrc.buffer = buf;
    airSrc.loop = true;
    const airFilter = ctx.createBiquadFilter();
    airFilter.type = 'bandpass';
    airFilter.frequency.value = 340;
    airFilter.Q.value = 0.9;
    const airGain = ctx.createGain();
    airGain.gain.value = 0.0;
    airSrc.connect(airFilter);
    airFilter.connect(airGain);
    airGain.connect(master);
    airSrc.start();
    this.airGain = airGain;

    // 2. Outdoor Japanese Summer Cicada Chorus (modulated bandpass noise, attenuated by window glass!)
    const cicSrc = ctx.createBufferSource();
    cicSrc.buffer = buf;
    cicSrc.loop = true;
    const cicBand = ctx.createBiquadFilter();
    cicBand.type = 'bandpass';
    cicBand.frequency.value = 4150;
    cicBand.Q.value = 5.5;

    const cicLowpass = ctx.createBiquadFilter();
    cicLowpass.type = 'lowpass';
    cicLowpass.frequency.value = 1100;
    this.cicadaFilter = cicLowpass;

    const cicGain = ctx.createGain();
    cicGain.gain.value = 0.012;
    this.cicadaGain = cicGain;

    // Natural 11 Hz summer cicada trill LFO
    const lfo = ctx.createOscillator();
    lfo.frequency.value = 11.5;
    const lfoDepth = ctx.createGain();
    lfoDepth.gain.value = 0.006;
    lfo.connect(lfoDepth);
    lfoDepth.connect(cicGain.gain);
    lfo.start();

    cicSrc.connect(cicBand);
    cicBand.connect(cicLowpass);
    cicLowpass.connect(cicGain);
    cicGain.connect(master);
    cicSrc.start();

    // 3. Electric Kettle Nucleate Boiling Hiss
    const ketSrc = ctx.createBufferSource();
    ketSrc.buffer = buf;
    ketSrc.loop = true;
    const ketFilter = ctx.createBiquadFilter();
    ketFilter.type = 'bandpass';
    ketFilter.frequency.value = 1350;
    ketFilter.Q.value = 1.4;
    const ketGain = ctx.createGain();
    ketGain.gain.value = 0.0;
    ketSrc.connect(ketFilter);
    ketFilter.connect(ketGain);
    ketGain.connect(master);
    ketSrc.start();
    this.kettleGain = ketGain;
  }

  toggleMute() {
    this.ensureStarted();
    this.muted = !this.muted;
    if (this.master && this.ctx) {
      this.master.gain.setTargetAtTime(this.muted ? 0.0 : 0.55, this.ctx.currentTime, 0.04);
    }
    return this.muted;
  }

  /** Immediate <8 ms tactile mechanical switch / slider / China-marker click */
  playClick(strength = 0.6) {
    if (!this.started || this.muted || !this.ctx) return;
    const now = this.ctx.currentTime;
    const osc = this.ctx.createOscillator();
    const g = this.ctx.createGain();
    osc.type = 'triangle';
    osc.frequency.setValueAtTime(920, now);
    osc.frequency.exponentialRampToValueAtTime(160, now + 0.028);
    g.gain.setValueAtTime(0.14 * strength, now);
    g.gain.exponentialRampToValueAtTime(0.0008, now + 0.032);
    osc.connect(g);
    g.connect(this.master);
    osc.start(now);
    osc.stop(now + 0.036);
  }

  /** Heavy solenoid snap when the 15A bi-metallic breaker trips under overload */
  playBreakerTrip() {
    if (!this.started || this.muted || !this.ctx) return;
    const now = this.ctx.currentTime;
    const osc = this.ctx.createOscillator();
    const g = this.ctx.createGain();
    osc.type = 'sawtooth';
    osc.frequency.setValueAtTime(340, now);
    osc.frequency.exponentialRampToValueAtTime(48, now + 0.11);
    g.gain.setValueAtTime(0.28, now);
    g.gain.exponentialRampToValueAtTime(0.001, now + 0.13);
    osc.connect(g);
    g.connect(this.master);
    osc.start(now);
    osc.stop(now + 0.14);
  }

  /** 343 m/s Distant Coastal Pad Static-Fire Shockwave Arrival + 18 Hz Window Glass Rattle! */
  playShockwave(intensity = 1.0) {
    if (!this.started || this.muted || !this.ctx) return;
    const now = this.ctx.currentTime;

    // Sub-bass atmospheric pressure N-wave boom
    const sub = this.ctx.createOscillator();
    const subG = this.ctx.createGain();
    sub.type = 'sine';
    sub.frequency.setValueAtTime(64, now);
    sub.frequency.exponentialRampToValueAtTime(24, now + 0.85);
    subG.gain.setValueAtTime(0.38 * intensity, now);
    subG.gain.exponentialRampToValueAtTime(0.001, now + 0.95);
    sub.connect(subG);
    subG.connect(this.master);
    sub.start(now);
    sub.stop(now + 1.0);

    // Window-pane wooden frame & glass 18 Hz rattle bursts
    for (let k = 0; k < 5; k++) {
      const t = now + k * 0.055;
      const clack = this.ctx.createOscillator();
      const cg = this.ctx.createGain();
      clack.type = 'square';
      clack.frequency.setValueAtTime(240 - k * 22, t);
      cg.gain.setValueAtTime(0.075 * intensity * (1.0 - k * 0.16), t);
      cg.gain.exponentialRampToValueAtTime(0.0008, t + 0.038);
      clack.connect(cg);
      cg.connect(this.master);
      clack.start(t);
      clack.stop(t + 0.042);
    }
  }

  /** Continuous physical synthesis update from Rust telemetry array */
  syncFromTelemetry(t) {
    if (!this.started || !this.ctx) return;
    const now = this.ctx.currentTime;

    const busLive = t[22] > 0.5;
    const acOn = busLive && t[24] > 0.5;
    const compMod = acOn ? t[46] : 0.0;
    const winOpen = t[21];
    const warmth = t[9];
    const kettleOn = busLive && t[27] > 0.5;
    const waterC = t[28];

    // Compressor RPM frequency & hum amplitude scale directly with thermodynamic load!
    this.acHumOsc.frequency.setTargetAtTime(48.0 + compMod * 24.0, now, 0.12);
    this.acHarmOsc.frequency.setTargetAtTime(96.0 + compMod * 48.0, now, 0.12);
    this.acGain.gain.setTargetAtTime(acOn ? (0.008 + compMod * 0.032) : 0.0, now, 0.12);
    this.airGain.gain.setTargetAtTime(acOn ? (0.006 + compMod * 0.015) : 0.0, now, 0.15);

    // Summer cicadas louder in daytime warmth and brighter when window sash slides open
    const cicLevel = (0.005 + winOpen * 0.024) * Math.max(0.15, warmth);
    this.cicadaGain.gain.setTargetAtTime(cicLevel, now, 0.15);
    this.cicadaFilter.frequency.setTargetAtTime(850 + winOpen * 4200, now, 0.15);

    // Kettle nucleate boiling cavitation hiss above 68 °C
    const boilFrac = kettleOn ? Math.max(0.0, Math.min(1.0, (waterC - 65.0) / 35.0)) : 0.0;
    this.kettleGain.gain.setTargetAtTime(boilFrac * 0.045, now, 0.12);

    // Discrete impulses emitted by Rust kernel
    if (t[40] > 0.05) {
      this.playClick(t[40]);
    }
    if (t[41] > 0.05) {
      this.playShockwave(t[41]);
    }
    if (t[42] > 0.5) {
      this.playBreakerTrip();
    }
  }
}
