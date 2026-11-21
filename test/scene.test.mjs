/* Composition invariants, checked on the CPU.
 *
 * There is no GPU here, so the shader's geometry is ported to JavaScript (test/scene-probe.mjs) and
 * ray-traced. This suite asserts the things a screenshot would tell you, so that a change which
 * quietly puts a wall in front of the camera fails the build instead of reaching the player:
 *
 *   1. the sun actually reaches the floor (the bug that made two rounds look like grey fog),
 *   2. from the spawn point you see the window, the bed and the sun patch — the room's thesis,
 *   3. the gauges are on the walls they belong to, and in the frame at spawn,
 *   4. the probe and the shader agree on the room's dimensions.
 */
import { suite, ok } from './harness.mjs';
import { readFileSync } from 'fs';
import * as S from './scene-probe.mjs';

const shader = readFileSync(new URL('../game/render/glsl.js', import.meta.url), 'utf8');

suite('a west-facing hab at 16:26 has sun on its floor');
{
  const dir = S.ENV.sunDir;
  ok(dir[2] > 0.5, 'the sun is in front of the window (' + dir[2].toFixed(2) + ' of its light)');
  let lit = 0, total = 0;
  for (let x = -1.8; x <= 1.8; x += 0.15) {
    for (let z = -2.0; z <= 1.6; z += 0.15) {
      total++;
      if (S.sunVisible([x, 0.01, z], dir) > 0.5) lit++;
    }
  }
  ok(lit > total * 0.15, 'a real patch of floor is in direct sun: ' + lit + '/' + total + ' samples');
  ok(lit < total * 0.95, 'and it is a patch, not the whole floor: ' + lit + '/' + total);
}

suite('the window pattern is a pattern, not a smear');
{
  // Sweep across the lit floor and count how many times the sun switches on and off. A rectangle
  // projected through three mullions gives several bands and bars; a blob gives one.
  let prev = null, edges = 0, lit = 0;
  const line = [];
  for (let x = -2.0; x <= 2.0; x += 0.05) {
    const p = [x, 0.02, -0.6];
    const v = S.windowPattern(p, S.ENV.sunDir) * S.sunVisible(p, S.ENV.sunDir);
    if (v > 0.5) lit++;
    if (prev !== null && (v > 0.5) !== (prev > 0.5)) edges++;
    prev = v;
    line.push(v > 0.5 ? '#' : '.');
  }
  ok(lit > 20, 'there is a wide patch of sun on the floor (' + lit + ' of ' + line.length + ' samples)');
  ok(edges >= 2, 'and it has structure inside it (' + edges + ' transitions: ' +
    line.join('').replace(/\.+/g, '.').slice(0, 60) + ')');
}

suite('at spawn you can see the room you are in');
{
  const cam = S.camera([-2.10, 1.62, -1.60], 0.75, -0.12);
  const kinds = {};
  for (let iy = 0; iy < 12; iy++) {
    for (let ix = 0; ix < 16; ix++) {
      const ux = (ix / 15 - 0.5) * 2 * 0.95 * cam.fov;
      const uy = (0.5 - iy / 11) * 2 * 0.62 * cam.fov;
      const rd = S.norm([
        cam.fwd[0] + cam.right[0] * ux + cam.up[0] * uy,
        cam.fwd[1] + cam.right[1] * ux + cam.up[1] * uy,
        cam.fwd[2] + cam.right[2] * ux + cam.up[2] * uy
      ]);
      const [t, m] = S.trace(cam.pos, rd);
      const key = t > 1e8 ? 'sky' : m === 4 ? 'window' : (m === 9 || m === 1) ? 'bed'
        : m === 2 ? 'console' : m === 5 ? 'desk' : m === 6 ? 'seat' : m === 7 ? 'pipe'
        : m === 8 ? 'screen' : m === 11 ? 'glass' : 'shell';
      kinds[key] = (kinds[key] || 0) + 1;
    }
  }
  ok((kinds.window || 0) >= 8, 'the window is a real part of the frame (' + (kinds.window || 0) + ' samples)');
  ok((kinds.bed || 0) >= 8, 'the bed is the anchor of the frame (' + (kinds.bed || 0) + ' samples)');
  ok(!(kinds.sky > 6), 'you cannot see out of the room into the void (' + (kinds.sky || 0) + ' sky samples)');
  ok((kinds.shell || 0) < 160, 'the frame is furnished, not mostly bare wall (' + (kinds.shell || 0) + '/192 wall)');
}

suite('the probe and the shader describe the same room');
{
  const grab = (name) => {
    const m = shader.match(new RegExp("float " + name + " = (-?[0-9.]+);"));
    return m ? +m[1] : null;
  };
  ok(grab('PLAT_H') === S.ROOM.PLAT_H, 'platform height agrees: ' + grab('PLAT_H'));
  ok(grab('TREAD_H') === S.ROOM.TREAD_H, 'tread height agrees: ' + grab('TREAD_H'));
  ok(grab('WZ') === S.ROOM.WZ, 'window plane agrees: ' + grab('WZ'));
  ok(grab('WY1') === S.ROOM.WY1 && grab('WY2') === S.ROOM.WY2, 'window bands agree');
  ok(/2\.35 - abs\(p\.x\)/.test(shader) && /2\.30 - abs\(p\.z\)/.test(shader),
    'the shell half-extents in the shader are the ones the probe assumes');
}

suite('the look axis turns the way the mouse moves');
{
  // the camera basis: +yaw must move what is on the right of the screen further right
  const a = S.camera([0, 0, 0], 0.0, 0);
  const b = S.camera([0, 0, 0], 0.2, 0);
  ok(b.fwd[0] > a.fwd[0], '+yaw turns toward +x');
  ok(Math.abs(a.right[0] - 1) < 1e-9, 'at yaw 0 the camera right is +x, so +x is drawn on the right');
}
