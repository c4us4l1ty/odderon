/* Emit the JavaScript kernel's answers so the Rust kernel can be held to them.
 *
 *   node test/vectors.mjs        -> writes test/vectors.json
 *
 * CI runs this before `cargo test`, so `kernel/tests/parity.rs` is always comparing against the
 * kernel that actually ships. This is the mechanism behind "one kernel, two implementations": the
 * claim is a file in the repo and a test that reads it, not a paragraph in a design doc.
 */
import { writeFileSync } from 'fs';
import { createRequire } from 'module';
const require = createRequire(import.meta.url);
const M = require('../game/sim/dmath.js');
const Th = require('../game/sim/thermo.js');

const sin_x = [], sin_y = [], exp_x = [], exp_y = [], ln_x = [], ln_y = [], atan_x = [], atan_y = [];
for (let i = -300; i <= 300; i++) {
  const x = i * 0.0173;
  sin_x.push(x); sin_y.push(M.sin(x));
}
for (let i = -200; i <= 200; i++) {
  const x = i * 0.031;
  exp_x.push(x); exp_y.push(M.exp(x));
}
for (let i = 1; i <= 400; i++) {
  const x = i * 0.05;
  ln_x.push(x); ln_y.push(M.log(x));
}
for (let i = -400; i <= 400; i++) {
  const x = i * 0.02;
  atan_x.push(x); atan_y.push(M.atan(x));
}

// The room, 7200 steps, sampled. This is a *pure* replay of the bang-bang thermostat -- the same
// eight lines of arithmetic exist in kernel/src/lib.rs -- so the two kernels can be compared
// number by number rather than by eye.
const room = { t: 297.0, c: 350000, ua: 160, tOut: 303, ap: 1.8, acW: 2000, cop: 3, sp: 299.15, hyst: 0.5, on: false };
const room_t = [], room_T = [];
for (let i = 0; i < 7200; i++) {
  const tau = i < 3600 ? 1.0 : 0.05;          // midday, then the curtain is drawn
  const qSol = 1006 * room.ap * tau;
  const qLoss = room.ua * (room.tOut - room.t);
  room.on = room.on ? room.t > room.sp - room.hyst : room.t > room.sp + room.hyst;
  const qAc = room.on ? -room.acW * room.cop : 0;
  room.t += (qSol + qLoss + qAc) / room.c;    // dt = 1 s; C is in J/K, so this is kelvin per second
  if (i % 600 === 0) { room_t.push(i); room_T.push(room.t); }
}
room_T.push(room.t);
room_t.push(7200);

writeFileSync(new URL('./vectors.json', import.meta.url), JSON.stringify({
  sin_x, sin_y, exp_x, exp_y, ln_x, ln_y, atan_x, atan_y, room_t, room_T,
  // the inputs, so the Rust side replays rather than guesses
  room_state: [297.0, 350000, 160, 303, 1.8, 2000, 3, 299.15, 0.5, 1006]
}, null, 0) + '\n');
console.log('vectors.json: ' + sin_x.length + ' sin, ' + exp_x.length + ' exp, '
  + ln_x.length + ' ln, ' + atan_x.length + ' atan, ' + room_t.length + ' room samples');
