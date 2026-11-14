import test from 'node:test';
import assert from 'node:assert/strict';
import { renderSize } from '../../web/resolution.js';

test('interactive quality halves the capped final dimensions', () => {
  for (const [w,h,dpr] of [[800,600,1],[1800,1000,2],[4000,3000,1],[500,1000,3]]) {
    const final=renderSize(w,h,dpr,'final'), interactive=renderSize(w,h,dpr,'interactive');
    assert.ok(Math.max(...final)<=2048);
    for(let i=0;i<2;i++)assert.ok(Math.abs(interactive[i]*2-final[i])<=1,`${w}x${h} at ${dpr}: ${final}, ${interactive}`);
  }
});
