import test from 'node:test';
import assert from 'node:assert/strict';
import { readdir, readFile } from 'node:fs/promises';

test('application has no recurring timers or CSS animation loops',async()=>{
  const files=(await readdir('web')).filter(name=>/\.(css|js|html)$/.test(name));
  for(const file of files){
    const source=await readFile(`web/${file}`,'utf8');
    assert.doesNotMatch(source,/@keyframes\b|\bsetInterval\s*\(/,file);
    if(file!=='governor.js')assert.doesNotMatch(source,/\brequestAnimationFrame\s*\(/,file);
  }
});
