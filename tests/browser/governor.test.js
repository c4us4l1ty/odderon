import test from 'node:test';
import assert from 'node:assert/strict';
import { Governor } from '../../web/governor.js';
function fakeClock(){let id=0;const frames=new Map();return {frames,requestAnimationFrame:f=>{frames.set(++id,f);return id;},cancelAnimationFrame:i=>frames.delete(i),flush(){const old=[...frames.values()];frames.clear();old.forEach(f=>f());}};}
test('coalesces invalidations into one frame and rests',()=>{const c=fakeClock(),r=[],g=new Governor(q=>r.push(q),c);g.invalidate();g.invalidate();assert.equal(c.frames.size,1);c.flush();assert.deepEqual(r,['final']);assert.equal(c.frames.size,0);assert.equal(g.frameCount,1);});
test('release coalesces pending drag into one final frame',()=>{const c=fakeClock(),r=[],g=new Governor(q=>r.push(q),c);g.begin();g.move();c.flush();g.move();g.end();assert.equal(c.frames.size,1);c.flush();assert.deepEqual(r,['interactive','final']);assert.equal(c.frames.size,0);});
test('suspend cancels work and resume draws once',()=>{const c=fakeClock(),r=[],g=new Governor(q=>r.push(q),c);g.invalidate();g.suspend();c.flush();assert.deepEqual(r,[]);g.resume();c.flush();assert.deepEqual(r,['final']);assert.equal(c.frames.size,0);});
test('overlay-only invalidation never redraws world',()=>{const c=fakeClock(),r=[],g=new Governor(q=>r.push(q),c);g.invalidate(false);c.flush();assert.deepEqual(r,['overlay']);assert.equal(c.frames.size,0);});
test('draw may invalidate during a frame',()=>{const c=fakeClock(),r=[];let g;g=new Governor(q=>{r.push(q);if(r.length===1)g.invalidate();},c);g.invalidate();c.flush();assert.equal(c.frames.size,1);c.flush();assert.deepEqual(r,['final','final']);assert.equal(c.frames.size,0);});
test('draw failure does not loop',()=>{const c=fakeClock(),g=new Governor(()=>false,c);g.invalidate();c.flush();assert.equal(c.frames.size,0);});
