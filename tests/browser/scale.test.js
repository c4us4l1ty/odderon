import test from 'node:test';
import assert from 'node:assert/strict';
import { EARTH_RADIUS_M, MOON_RADIUS_M, projectedRadius, needsReticle } from '../../web/scale.js';

test('bodies and an orbit share one physical scale',()=>{
  const span=520e6;
  assert.equal(EARTH_RADIUS_M,6_378_137);assert.equal(MOON_RADIUS_M,1_737_400);
  const surface=projectedRadius(EARTH_RADIUS_M,span);
  const lowOrbit=projectedRadius(EARTH_RADIUS_M+200_000,span);
  assert.ok(lowOrbit>surface);
  assert.equal(needsReticle(projectedRadius(MOON_RADIUS_M,span),600),true);
  assert.equal(needsReticle(projectedRadius(EARTH_RADIUS_M,span),600),false);
});
