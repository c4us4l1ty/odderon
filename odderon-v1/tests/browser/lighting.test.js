import test from 'node:test';
import assert from 'node:assert/strict';
import {planetShine} from '../../web/lighting.js';
test('Lambert planet shine respects phase, distance and invalid geometry',()=>{
 const body=[0,0,0],sun=[1000,0,0];
 const full=planetShine(body,sun,[10,0,0],1,0.3),dark=planetShine(body,sun,[-10,0,0],1,0.3),far=planetShine(body,sun,[20,0,0],1,0.3);
 assert.ok(Math.abs(full.strength-.003)<1e-15);assert.ok(dark.strength<1e-15);assert.ok(Math.abs(far.strength/full.strength-.25)<1e-15);
 assert.equal(full.direction[0],-1);assert.equal(Math.abs(full.direction[1])+Math.abs(full.direction[2]),0);assert.equal(planetShine(body,sun,[0,0,0],1,.3).strength,0);
});
