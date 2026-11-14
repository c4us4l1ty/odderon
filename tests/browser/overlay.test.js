import test from 'node:test';
import assert from 'node:assert/strict';
import { polyline, vector, apsis, bPlane } from '../../web/overlay.js';
function fake(){const calls=[];const ctx=new Proxy({}, {get:(o,k)=>k in o?o[k]:(...args)=>calls.push([k,...args]),set:(o,k,v)=>(o[k]=v,true)});return {ctx,calls};}
test('path breaks at invalid points and preserves graphics state',()=>{const {ctx,calls}=fake();polyline(ctx,[[0,0],[1,1],[NaN,0],[2,2],[3,3]],{dashed:true});assert.deepEqual(calls.filter(c=>c[0]==='moveTo'),[['moveTo',0,0],['moveTo',2,2]]);assert.equal(calls[0][0],'save');assert.equal(calls.at(-1)[0],'restore');});
test('vector, apsis and B-plane use only supplied geometry',()=>{const {ctx,calls}=fake();vector(ctx,[0,0],[20,10]);apsis(ctx,[5,6],'periapsis');bPlane(ctx,[100,100],20,{target:[103,102],label:'B-plane / km'});assert.ok(calls.some(c=>c[0]==='fillText'&&c[1]==='periapsis'));assert.ok(calls.some(c=>c[0]==='fillText'&&c[1]==='B-plane / km'));assert.equal(calls.filter(c=>c[0]==='arc').length,4);const length=calls.length;bPlane(ctx,[NaN,0],10);apsis(ctx,[0,Infinity],'bad');vector(ctx,[0,0],[NaN,1]);assert.equal(calls.length,length);});
