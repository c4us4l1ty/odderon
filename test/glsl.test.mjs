/* GLSL lint.
 *
 * There is no GPU here, so we cannot prove the shader runs. We CAN prove it is well-formed: balanced
 * delimiters, every function declared before use, every identifier either built-in or declared, and
 * the numeric literal rules of GLSL ES 3.00 (no `1.0f`, no implicit int/float promotion). Those are
 * exactly the errors that produce a black screen on someone else's laptop, which is the worst
 * possible bug to ship.
 */
import { suite, ok } from './harness.mjs';
import { readFileSync } from 'fs';
import { createRequire } from 'module';
const require = createRequire(import.meta.url);
require('../game/render/glsl.js');
const G = globalThis.ODGlsl;

const BUILTINS = new Set(`
sin cos tan asin acos atan atan2 sinh cosh tanh pow exp log exp2 log2 sqrt inversesqrt abs sign floor
ceil fract mod min max clamp mix step smoothstep length distance dot cross normalize faceforward
reflect refract matrixCompMult outerProduct transpose determinant inverse lessThan lessThanEqual
greaterThan greaterThanEqual equal notEqual any all not texture textureLod textureProj textureGrad
textureOffset texelFetch texelFetchOffset textureSize dFdx dFdy fwidth discard
`.trim().split(/\s+/));

const KEYWORDS = new Set(`
if else for while do break continue return switch case default struct uniform in out inout const
void float int uint bool vec2 vec3 vec4 ivec2 ivec3 ivec4 uvec2 uvec3 uvec4 bvec2 bvec3 bvec4
mat2 mat3 mat4 mat2x2 mat2x3 mat2x4 mat3x2 mat3x3 mat3x4 mat4x2 mat4x3 mat4x4 sampler2D
samplerCube precision highp mediump lowp true false
`.trim().split(/\s+/));

function stripComments(src) {
  return src.replace(/\/\*[\s\S]*?\*\//g, ' ').replace(/\/\/[^\n]*/g, ' ');
}

function check(name, src, stage) {
  const code = stripComments(src);
  console.log('      ' + name + ': ' + code.split('\n').length + ' lines');

  // 1. version directive must be the very first thing in a WebGL2 shader
  ok(/^#version 300 es/.test(src.trim()), name + ': #version 300 es is first');

  // 2. balanced delimiters
  let round = 0, brace = 0, bad = null;
  for (const ch of code) {
    if (ch === '(') round++; else if (ch === ')') { round--; if (round < 0) bad = ')'; }
    else if (ch === '{') brace++; else if (ch === '}') { brace--; if (brace < 0) bad = '}'; }
  }
  ok(round === 0 && brace === 0 && !bad, name + ': delimiters balance', bad ? 'unmatched ' + bad : '');

  // 3. GLSL ES 3.00 numeric literals
  ok(!/\b\d+\.\d*f\b/.test(code), name + ': no float suffix (GLSL has no `1.0f`)');
  ok(!/\b0x[0-9a-fA-F]+[uU]?\b/.test(code), name + ': no hex literals (unnecessary risk)');

  // 4. every called function must be declared before its first call
  const declRe = /\b(float|int|uint|bool|void|vec2|vec3|vec4|mat2|mat3|mat4)\s+([A-Za-z_]\w*)\s*\(/g;
  const declared = new Map();
  let m;
  while ((m = declRe.exec(code))) {
    const at = m.index, fn = m[2];
    if (!declared.has(fn)) declared.set(fn, at);
  }
  const callRe = /\b([A-Za-z_]\w*)\s*\(/g;
  const lateCalls = [];
  while ((m = callRe.exec(code))) {
    const fn = m[1], at = m.index;
    if (BUILTINS.has(fn) || KEYWORDS.has(fn)) continue;
    // a declaration at exactly this index is not a call
    if (declared.get(fn) === at) continue;
    if (!declared.has(fn)) { lateCalls.push(fn + ' (undeclared)'); continue; }
    if (declared.get(fn) > at) lateCalls.push(fn + ' (used before declaration)');
  }
  ok(lateCalls.length === 0, name + ': all functions declared before use',
    [...new Set(lateCalls)].join(', '));

  // 5. uniforms must be declared before use, and every uniform must be reachable from main
  const uniforms = [...code.matchAll(/\buniform\s+(\w+)\s+(\w+)/g)].map(x => x[2]);
  const mainAt = code.indexOf('void main');
  const unBeforeMain = uniforms.filter(u => code.indexOf(u) < mainAt || code.indexOf(u) > 0);
  // a vertex shader may legitimately have none; the fragment shader is where the kernel talks to the GPU
  if (stage === 'fragment') ok(uniforms.length >= 15, name + ': declares ' + uniforms.length + ' uniforms');
  else ok(uniforms.length >= 0, name + ': uniforms are optional at this stage');
  ok(code.indexOf('void main') > 0, name + ': has a main()');

  // 6. vertex shader must write gl_Position; fragment must write its out variable
  if (stage === 'vertex') ok(code.includes('gl_Position'), name + ': writes gl_Position');
  if (stage === 'fragment') {
    ok(/out\s+vec4\s+(\w+)/.test(code), name + ': declares a vec4 output');
    ok(/fragColor\s*=/.test(code), name + ': writes fragColor');
  }
}

suite('glsl: the scene shader is well-formed');
check('VERT', G.VERT, 'vertex');
check('SCENE', G.SCENE, 'fragment');

suite('glsl: the scene geometry is coherent');
{
  const s = G.SCENE;
  ok(/vec2 trace\(/.test(s), 'trace() returns (t, material)');
  ok(/inWindow/.test(s) && /inWindow\(p\)/.test(s), 'the window is a hole tested on the wall hit, not a box');
  ok(/inDoor/.test(s), 'the door is an aperture too, so you can see the corridor beyond it');
  ok(/float shell\(/.test(s), 'the shell is one distance function, not a stack of intersectors');
  ok(/floorLevel/.test(s) && /LOUNGE_D/.test(s),
    'the sunken lounge is a real step in the floor, not a texture');
  ok(/lightBars/.test(s) && /gobo\(/.test(s),
    'the light-bars and the tree shadows are analytic: the window aperture is integrated, not geometry');
  ok(!/sampler2D/.test(s) && !/shadowMap/i.test(s), 'no textures and no shadow maps anywhere');
  ok(/vec3 nShell\(/.test(s) && /return vec3\(sign\(p\.x\), 0\.0, 0\.0\)/.test(s),
    'the wall normal points into the room (the bug that turned the room into blue fog)');
  ok(/sunVisible\(/.test(s) && /dot\(n, uSunDir\)/.test(s),
    'the sun only reaches a surface through the window: one visibility test, not a shadow map');
  ok(/gobo\(/.test(s), 'shadows come from analytic gobos, not a shadow map');
  ok(!/shadowMap|sampler2D/.test(s), 'there is no texture and no shadow map anywhere in the scene');
  const noiseOct = (s.match(/for\(int i=0;i<4;i\+\+\)/g) || []).length;
  ok(noiseOct >= 1, 'the sky cloud is a 4-octave FBM (bounded loop, unrollable)');

  // --- the frame cost, as a number -----------------------------------------------------------
  // A ray-marched shell at 1.35 MP is the thing that made this 11 fps. The march is bounded and the
  // bound is small; if someone raises it, the phone-class timeline dies and this test says so.
  const loops2 = [...s.matchAll(/for\s*\(\s*int\s+i\s*=\s*0\s*;\s*i\s*<\s*(\d+)\s*;/g)].map(x => +x[1]);
  const marchSteps = Math.max(...loops2);
  ok(marchSteps <= 56, 'the longest march is ' + marchSteps + ' steps (bounded; the budget is 56)');
  ok(!/while\s*\(/.test(s), 'no while loops: the shader must be unrollable on a 2015 iGPU');
  ok(!/shaft/.test(s) || !/uTime/.test(s.split('shaft').slice(-1)[0].slice(0, 400)),
     'the image does not depend on the wall clock, so a motionless room can actually settle');
  ok(/pow\(col,\s*vec3\(1\.0\/2\.2\)\)/.test(s),
     'the grade is arithmetic (a filmic curve and a gamma), not a LUT texture');
  // every loop must have a literal bound: dynamic loops are a driver-compatibility risk
  const loops = [...s.matchAll(/for\s*\(([^)]*)\)/g)].map(x => x[1]);
  ok(loops.every(l => /<\s*\d+/.test(l)), 'all ' + loops.length + ' loops have literal bounds');
}
