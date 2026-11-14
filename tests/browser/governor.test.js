import test from 'node:test';
import assert from 'node:assert/strict';
import { Governor } from '../../web/governor.js';
function fakeClock(){let id=0;const frames=new Map(),timers=new Map();return {frames,timers,requestAnimationFrame:f=>{frames.set(++id,f);return id;},cancelAnimationFrame:i=>frames.delete(i),setTimeout:(f,ms)=>{timers.set(++id,{f,ms});return id;},clearTimeout:i=>timers.delete(i),flushFrame(){const old=[...frames.values()];frames.clear();old.forEach(f=>f());},flushTimers(){const old=[...timers.values()];timers.clear();old.forEach(t=>t.f());}};}
test('coalesces invalidations and has no idle work',()=>{const c=fakeClock(),r=[],g=new Governor(q=>r.push(q),c);g.invalidate();g.invalidate();assert.equal(c.frames.size,1);c.flushFrame();assert.deepEqual(r,['final']);assert.equal(c.frames.size,0);assert.equal(c.timers.size,0);});
test('interactive frames settle exactly once after release debounce',()=>{const c=fakeClock(),r=[],g=new Governor(q=>r.push(q),c);g.begin();g.move();c.flushFrame();g.move();g.end();assert.equal(c.frames.size,0);assert.equal([...c.timers.values()][0].ms,50);c.flushTimers();c.flushFrame();assert.deepEqual(r,['interactive','final']);assert.equal(c.timers.size,0);assert.equal(c.frames.size,0);});
test('new drag and suspension cancel stale work',()=>{const c=fakeClock(),r=[],g=new Governor(q=>r.push(q),c);g.begin();g.end();g.begin();assert.equal(c.timers.size,0);g.suspend();c.flushFrame();c.flushTimers();assert.deepEqual(r,[]);g.resume();c.flushFrame();assert.deepEqual(r,['final']);});

test('a click without motion never paints an interactive frame',()=>{const c=fakeClock(),r=[],g=new Governor(q=>r.push(q),c);g.begin();assert.equal(c.frames.size,0);g.end();c.flushTimers();c.flushFrame();assert.deepEqual(r,['final']);});

test('an invalidation during release debounce supersedes the delayed final frame',()=>{const c=fakeClock(),r=[],g=new Governor(q=>r.push(q),c);g.begin();g.move();c.flushFrame();g.end();assert.equal(c.timers.size,1);g.invalidate();assert.equal(c.timers.size,0);c.flushFrame();c.flushTimers();assert.deepEqual(r,['interactive','final']);assert.equal(c.frames.size,0);});
