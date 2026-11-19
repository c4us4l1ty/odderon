import test from 'node:test';
import assert from 'node:assert/strict';
import { readdir, readFile } from 'node:fs/promises';

test('application has no recurring timers, animation loops, or stray frame schedulers',async()=>{
  const files=(await readdir('web')).filter(name=>/\.(css|js|html)$/.test(name));
  for(const file of files){
    const source=await readFile(`web/${file}`,'utf8');
    assert.doesNotMatch(source,/@keyframes\b|\bsetInterval\s*\(/,file);
    // Only the demand schedulers may arm a frame: the shared Governor and the
    // Flight Deck instrument layer, which coalesces its repaints into one
    // pending frame and never arms another. Every other file must be inert.
    if(!['governor.js','instrument.js'].includes(file))
      assert.doesNotMatch(source,/\brequestAnimationFrame\s*\(/,file);
  }
});
