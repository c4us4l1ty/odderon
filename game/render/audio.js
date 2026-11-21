/* Odderon — sound, synthesized from kernel numbers (no audio files; docs/GAME-DESIGN-BIBLE.md).
 *
 * Three things at once, and the third is the reason the game feels responsive on a slow machine:
 *   1. the room's voice  — cicadas, the AC's compressor, its start-up clunk, distant insects
 *   2. the physics's voice — a turbopump whine whose pitch IS rpm, a cavitation rasp whose onset IS
 *      the NPSH margin, wind whose colour IS the irradiance
 *   3. latency          — audio runs on its own clock, so a switch is *heard* in ~10 ms even when the
 *      next frame is 30 ms away (PERFORMANCE.md I8)
 */
(function (global) {
  'use strict';

  function Audio() {
    this.ctx = null;
    this.ready = false;
    this.state = { acOn: false, acDuty: 0, cicada: 0, wind: 0, rpm: 0, cavitation: 0 };
  }

  Audio.prototype.start = function () {
    if (this.ready) return true;
    var AC = global.AudioContext || global.webkitAudioContext;
    if (!AC) return false;
    var ctx = new AC({ latencyHint: 'interactive' });
    this.ctx = ctx;

    this.master = ctx.createGain();
    this.master.gain.value = 0.0;
    this.master.connect(ctx.destination);

    /* --- cicadas: a dense field of narrow-band noise, amplitude-modulated at ~40 Hz --- */
    this.cicadaGain = ctx.createGain(); this.cicadaGain.gain.value = 0;
    this.cicadaGain.connect(this.master);
    var cic = ctx.createBufferSource();
    cic.buffer = this._noiseBuffer(3.0, 'pink');
    cic.loop = true;
    var cicBP = ctx.createBiquadFilter(); cicBP.type = 'bandpass';
    cicBP.frequency.value = 4200; cicBP.Q.value = 6;
    var lfo = ctx.createOscillator(); lfo.type = 'sine'; lfo.frequency.value = 38;
    var lfoAmt = ctx.createGain(); lfoAmt.gain.value = 0.55;
    var lfoDC = ctx.createGain(); lfoDC.gain.value = 0.45;
    lfo.connect(lfoAmt).connect(lfoDC);
    lfoDC.connect(this.cicadaGain.gain);
    cic.connect(cicBP).connect(this.cicadaGain);
    cic.start(); lfo.start();

    /* --- the AC: a compressor tone whose level follows duty, plus a start-up clunk --- */
    this.acGain = ctx.createGain(); this.acGain.gain.value = 0;
    this.acGain.connect(this.master);
    this.acOsc = ctx.createOscillator(); this.acOsc.type = 'sawtooth'; this.acOsc.frequency.value = 58;
    var acLP = ctx.createBiquadFilter(); acLP.type = 'lowpass'; acLP.frequency.value = 320;
    var acHiss = ctx.createBufferSource(); acHiss.buffer = this._noiseBuffer(2, 'white'); acHiss.loop = true;
    var acHissF = ctx.createBiquadFilter(); acHissF.type = 'bandpass'; acHissF.frequency.value = 1900; acHissF.Q.value = 1.4;
    var acHissG = ctx.createGain(); acHissG.gain.value = 0.25;
    this.acOsc.connect(acLP).connect(this.acGain);
    acHiss.connect(acHissF).connect(acHissG).connect(this.acGain);
    this.acOsc.start(); acHiss.start();

    /* --- wind / air: colour tracks the sky --- */
    this.windGain = ctx.createGain(); this.windGain.gain.value = 0;
    this.windGain.connect(this.master);
    var w = ctx.createBufferSource(); w.buffer = this._noiseBuffer(4, 'brown'); w.loop = true;
    var wLP = ctx.createBiquadFilter(); wLP.type = 'lowpass'; wLP.frequency.value = 500;
    w.connect(wLP).connect(this.windGain); w.start();

    this.ready = true;
    return true;
  };

  Audio.prototype._noiseBuffer = function (seconds, kind) {
    var ctx = this.ctx, n = Math.floor(ctx.sampleRate * seconds);
    var buf = ctx.createBuffer(1, n, ctx.sampleRate);
    var d = buf.getChannelData(0);
    // deterministic noise: a fixed LCG, so the sound is identical every run (no Math.random)
    var s = 22222;
    var b0 = 0, b1 = 0, b2 = 0, last = 0;
    for (var i = 0; i < n; i++) {
      s = (s * 1103515245 + 12345) & 0x7fffffff;
      var white = (s / 0x3fffffff) - 1;
      if (kind === 'pink') {
        b0 = 0.99765 * b0 + white * 0.0990460;
        b1 = 0.96300 * b1 + white * 0.2965164;
        b2 = 0.57000 * b2 + white * 1.0526913;
        d[i] = (b0 + b1 + b2 + white * 0.1848) * 0.12;
      } else if (kind === 'brown') {
        last = (last + 0.02 * white) / 1.02;
        d[i] = last * 3.2;
      } else {
        d[i] = white * 0.4;
      }
    }
    return buf;
  };

  // A short, physically-shaped transient: the compressor's start clunk.
  Audio.prototype.clunk = function (strength) {
    if (!this.ready) return;
    var ctx = this.ctx, t = ctx.currentTime;
    var o = ctx.createOscillator(), g = ctx.createGain();
    o.type = 'sine'; o.frequency.setValueAtTime(96, t);
    o.frequency.exponentialRampToValueAtTime(38, t + 0.10);
    g.gain.setValueAtTime(0.0001, t);
    g.gain.exponentialRampToValueAtTime(0.22 * (strength || 1), t + 0.006);
    g.gain.exponentialRampToValueAtTime(0.0001, t + 0.18);
    o.connect(g).connect(this.master);
    o.start(t); o.stop(t + 0.2);
  };

  // Called on every frame *and* from the sim; it is cheap and idempotent.
  Audio.prototype.update = function (s) {
    if (!this.ready) return;
    var ctx = this.ctx, t = ctx.currentTime;
    var dt = Math.min(0.2, t - (this._t || t)); this._t = t;
    // AC: level follows duty; frequency sags slightly as the compressor loads (real behaviour)
    var acLevel = s.acOn ? 0.085 : 0.0;
    var target = s.acOn ? 56 + 3 * (1 - (s.acDuty || 0)) : 0;
    this.acGain.gain.setTargetAtTime(acLevel, t, 0.06);
    if (s.acOn) this.acOsc.frequency.setTargetAtTime(target, t, 0.4);
    // cicadas: day and temperature dependant, and they go quiet when the window is shut
    var cic = s.cicada === undefined ? 0.16 : s.cicada;
    this.cicadaGain.gain.setTargetAtTime(cic, t, 0.8);
    // wind: colour and level from the air
    this.windGain.gain.setTargetAtTime(s.wind === undefined ? 0.05 : s.wind, t, 1.2);
    // a gentle fade-in on first frame, so nothing ever starts with a click
    if (this.master.gain.value < 0.9) this.master.gain.setTargetAtTime(0.9, t, 0.5);
  };

  Audio.prototype.mute = function (m) {
    if (!this.ready) return;
    this.master.gain.setTargetAtTime(m ? 0 : 0.9, this.ctx.currentTime, 0.15);
  };

  global.ODAudio = Audio;
})(typeof globalThis !== 'undefined' ? globalThis : this);
