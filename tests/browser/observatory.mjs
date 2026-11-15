// Real Chromium/WebGL2 integration and demand-rendering gate.
import { chromium } from 'playwright-core';
import { createServer } from 'node:http';
import { readFile, writeFile, mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { resolve, extname } from 'node:path';
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import jsQR from 'jsqr';
const root=resolve('web');

// Rebuild the QR matrix from the emitted vector rectangles and decode it with
// an independent decoder. This proves the printed code carries a payload that
// actually verifies, rather than trusting the encoder a second time.
function decodeQrFromSvg(svg){
  const size=Number(svg.match(/data-qr-size="(\d+)"/)[1]);
  const quiet=Number(svg.match(/data-qr-quiet="(\d+)"/)[1]);
  const outer=size+2*quiet;
  const start=svg.indexOf('<g fill="#000000">');
  assert.ok(start>=0,'the QR must emit a dark-module group');
  // Close the group relative to its own start: the trajectory figure earlier
  // in the document has groups too.
  const group=svg.slice(start,svg.indexOf('</g>',start));
  const grid=Array.from({length:outer},()=>Array(outer).fill(false));
  for(const match of group.matchAll(/<rect x="(\d+)" y="(\d+)" width="(\d+)" height="1"\/>/g)){
    const [,x,y,width]=match.map(Number);
    for(let offset=0;offset<width;offset++)grid[y][x+offset]=true;
  }
  const scale=6,pixels=outer*scale;
  const data=new Uint8ClampedArray(pixels*pixels*4);
  for(let y=0;y<pixels;y++)for(let x=0;x<pixels;x++){
    const dark=grid[Math.floor(y/scale)][Math.floor(x/scale)];
    const at=(y*pixels+x)*4,value=dark?0:255;
    data[at]=value;data[at+1]=value;data[at+2]=value;data[at+3]=255;
  }
  const decoded=jsQR(data,pixels,pixels);
  assert.ok(decoded,'an independent decoder must read the emitted QR');
  return decoded.data;
}

// One unit in the last place, computed on the raw IEEE-754 bits.
function nextUlp(value){
  const buffer=new ArrayBuffer(8),view=new DataView(buffer);
  view.setFloat64(0,value);
  view.setBigUint64(0,view.getBigUint64(0)+1n);
  return view.getFloat64(0);
}
const server=createServer(async (req,res)=>{
  const path=resolve(root,'.'+new URL(req.url,'http://test').pathname.replace(/\/$/,'/index.html'));
  if(!path.startsWith(root+'/')){res.writeHead(403).end();return;}
  try{const data=await readFile(path);res.setHeader('Content-Type',({'.html':'text/html','.js':'text/javascript','.css':'text/css','.wasm':'application/wasm'})[extname(path)]||'application/octet-stream');res.end(data);}catch{res.writeHead(404).end();}
});
await new Promise(r=>server.listen(0,'127.0.0.1',r));
let browser;
try{
  browser=await chromium.launch({executablePath:process.env.CHROME_BIN||execFileSync('which',['google-chrome'],{encoding:'utf8'}).trim(),headless:true,args:['--no-sandbox','--enable-webgl','--use-angle=swiftshader','--enable-unsafe-swiftshader','--disable-dev-shm-usage']});
  const page=await browser.newPage({viewport:{width:1440,height:1000},deviceScaleFactor:1});
  // Date controls cannot race the asynchronous WebAssembly download.
  const slow=await browser.newPage();let release;const barrier=new Promise(r=>{release=r;});let requestStarted;const requested=new Promise(r=>{requestStarted=r;});
  await slow.route('**/odd_view.wasm',async route=>{requestStarted();await barrier;await route.continue();});
  await slow.goto(`http://127.0.0.1:${server.address().port}/`,{waitUntil:'domcontentloaded'});await requested;
  assert.equal(await slow.locator('#date').isDisabled(),true);assert.equal(await slow.locator('#next').isDisabled(),true);
  release();await slow.waitForFunction(()=>window.odderonDiagnostics?.draws>0,null,{timeout:30000});assert.equal(await slow.locator('#date').isEnabled(),true);await slow.close();
  // Capture the actual MediaQueryList objects so we can deliver a monitor
  // change notification even on headless Chrome versions whose CDP metrics
  // override updates matchMedia results without emitting a change event.
  await page.addInitScript(()=>{const raf=window.requestAnimationFrame.bind(window);window.testFrameCallbacks=0;window.requestAnimationFrame=fn=>raf(time=>{window.testFrameCallbacks++;fn(time);});window.testWorkerMessages=0;window.testWorldPasses=0;window.testInstancedPasses=0;const proto=WebGL2RenderingContext.prototype;const drawArrays=proto.drawArrays;proto.drawArrays=function(...args){window.testWorldPasses++;return drawArrays.apply(this,args);};const instanced=proto.drawArraysInstanced;proto.drawArraysInstanced=function(...args){window.testInstancedPasses++;return instanced.apply(this,args);};const NativeWorker=window.Worker;window.Worker=class extends NativeWorker{constructor(...args){super(...args);this.addEventListener('message',()=>window.testWorkerMessages++);}postMessage(...args){window.testWorkerMessages++;return super.postMessage(...args);}};const original=window.matchMedia.bind(window);window.testDensityQueries=[];window.matchMedia=query=>{const result=original(query);if(query.startsWith('(resolution: '))window.testDensityQueries.push(result);return result;};});
  const errors=[];page.on('pageerror',e=>errors.push(String(e)));page.on('console',m=>{if(m.type()==='error')errors.push(m.text());});
  await page.goto(`http://127.0.0.1:${server.address().port}/`);
  await page.waitForFunction(()=>window.odderonDiagnostics?.draws>0,{timeout:30000});
  await page.waitForFunction(()=>odderonDiagnostics.quality.includes('RESTING')&&!odderonDiagnostics.pendingFrame,null,{timeout:60000});
  assert.equal(await page.evaluate(()=>window.testWorldPasses),1,'initial view must use one world pass');
  assert.equal(await page.evaluate(()=>window.testInstancedPasses),0,'no brushstroke batches');
  assert.equal(await page.locator('#error').isHidden(),true);
  assert.equal(await page.locator('#distance').textContent(),'361,026');
  // Telemetry-only controls must not re-run the expensive world layer.
  const layerBefore=await page.evaluate(()=>({world:odderonDiagnostics.draws,overlay:odderonDiagnostics.overlayDraws}));
  await page.locator('#show-path').uncheck();await page.waitForFunction(n=>odderonDiagnostics.overlayDraws>n,layerBefore.overlay);
  assert.equal(await page.evaluate(()=>odderonDiagnostics.draws),layerBefore.world,'path toggle repainted the world');
  await page.locator('#show-path').check();await page.waitForFunction(n=>odderonDiagnostics.overlayDraws>n,layerBefore.overlay+1);
  assert.equal(await page.evaluate(()=>odderonDiagnostics.draws),layerBefore.world,'path toggle repainted the world');
  // Check actual GPU pixels at the analytically projected centers, not just
  // a draw counter: shader compilation or a blank framebuffer must fail.
  const pixels=await page.locator('#world').evaluate(canvas=>{
    const gl=canvas.getContext('webgl2'), circles=window.odderonDiagnostics.circles;
    const read=(x,y)=>{const out=new Uint8Array(4);gl.readPixels(x,y,1,1,gl.RGBA,gl.UNSIGNED_BYTE,out);return [...out];};
    const sample=c=>read(Math.floor((canvas.width+c[0]*canvas.height)/2),Math.floor((canvas.height+c[1]*canvas.height)/2));
    return {earth:sample(circles.earth),moon:sample(circles.moon),background:read(10,10),error:gl.getError()};
  });
  assert.equal(pixels.error,0);for(const body of ['earth','moon'])assert.ok(pixels[body].slice(0,3).some((v,i)=>Math.abs(v-pixels.background[i])>12),`${body} center is indistinguishable from empty sky: ${JSON.stringify(pixels)}`);
  const stats=await page.evaluate(()=>({draws:odderonDiagnostics.draws,pendingFrame:odderonDiagnostics.pendingFrame,quality:odderonDiagnostics.quality}));
  assert.equal(stats.pendingFrame,false);assert.match(stats.quality,/RESTING/);
  const before=await page.evaluate(()=>({frames:window.testFrameCallbacks,messages:window.testWorkerMessages,renderFrames:odderonDiagnostics.frameCount}));
  await page.waitForTimeout(2000);
  const after=await page.evaluate(()=>({frames:window.testFrameCallbacks,messages:window.testWorkerMessages,renderFrames:odderonDiagnostics.frameCount}));
  assert.deepEqual(after,before,'idle must schedule no animation callbacks or worker messages');
  assert.equal(await page.evaluate(()=>odderonDiagnostics.pendingFrame),false);
  console.log('Idle 2s after single frame: 0 animation callbacks and 0 worker messages.');
  const cdp=await page.context().newCDPSession(page);
  const box=await page.locator('#scene').boundingBox();
  await page.mouse.move(box.x+box.width*.5,box.y+box.height*.5);await page.mouse.down();await page.mouse.move(box.x+box.width*.6,box.y+box.height*.55,{steps:4});await page.waitForTimeout(200);
  assert.equal(await page.evaluate(()=>odderonDiagnostics.quality),'INTERACTIVE QUALITY');
  assert.ok(await page.evaluate(()=>odderonDiagnostics.draws)>stats.draws);
  const interactiveWidth=await page.locator('#world').evaluate(c=>c.width);await page.mouse.up();await page.waitForFunction(()=>odderonDiagnostics.quality.includes('RESTING')&&!odderonDiagnostics.pendingFrame,null,{timeout:60000});
  assert.match(await page.evaluate(()=>odderonDiagnostics.quality),/RESTING/);
  const released=await page.evaluate(()=>({frames:window.testFrameCallbacks,messages:window.testWorkerMessages,renderFrames:odderonDiagnostics.frameCount}));
  await page.waitForTimeout(2000);
  const releasedAfter=await page.evaluate(()=>({frames:window.testFrameCallbacks,messages:window.testWorkerMessages,renderFrames:odderonDiagnostics.frameCount}));
  assert.deepEqual(releasedAfter,released,'camera release must settle with zero frames and worker messages');
  assert.ok(await page.locator('#world').evaluate(c=>c.width)>interactiveWidth,'full resolution after drag');
  // DPR can change without a CSS resize (e.g. moving between monitors).
  const initialSize=await page.locator('#world').evaluate(c=>({width:c.width,css:c.getBoundingClientRect().width}));
  const beforeDensity=await page.evaluate(()=>odderonDiagnostics.draws);
  await cdp.send('Emulation.setDeviceMetricsOverride',{width:1440,height:1000,deviceScaleFactor:2,mobile:false});
  assert.equal(await page.evaluate(()=>devicePixelRatio),2);
  await page.evaluate(()=>window.testDensityQueries[0].dispatchEvent(new Event('change')));
  await page.waitForFunction(n=>odderonDiagnostics.draws>n,beforeDensity,{timeout:10000});
  const denseSize=await page.locator('#world').evaluate(c=>({width:c.width,css:c.getBoundingClientRect().width}));
  assert.equal(denseSize.css,initialSize.css);assert.ok(denseSize.width>initialSize.width,'DPR change did not resize the world canvas');
  const beforeRestoreDensity=await page.evaluate(()=>odderonDiagnostics.draws);
  assert.ok(await page.evaluate(()=>window.testDensityQueries.length)>=2,'density watcher did not re-arm');
  const queryIndex=await page.evaluate(()=>window.testDensityQueries.length-1);
  await cdp.send('Emulation.setDeviceMetricsOverride',{width:1440,height:1000,deviceScaleFactor:1,mobile:false});
  assert.equal(await page.evaluate(()=>devicePixelRatio),1);
  await page.evaluate(i=>window.testDensityQueries[i].dispatchEvent(new Event('change')),queryIndex);
  await page.waitForFunction(n=>odderonDiagnostics.draws>n,beforeRestoreDensity,{timeout:10000});
  await page.waitForFunction(()=>odderonDiagnostics.quality.includes('RESTING')&&!odderonDiagnostics.pendingFrame,null,{timeout:60000});
  assert.equal(await page.locator('#world').evaluate(c=>c.width),initialSize.width);
  // The displayed test orbit is calculated in odd-astro, not a drawn ellipse.
  assert.deepEqual(await page.evaluate(()=>odderonDiagnostics.orbit),{samples:180,closed:true,markers:['PE','AP','AN','DN']});
  await page.locator('#show-orbit').check();
  await page.waitForFunction(()=>odderonDiagnostics.quality.includes('RESTING')&&!odderonDiagnostics.pendingFrame,null,{timeout:60000});
  assert.equal(await page.evaluate(()=>odderonDiagnostics.orbit.samples),180);
  await page.waitForFunction(()=>odderonDiagnostics.trajectory.samples===241&&!odderonDiagnostics.trajectory.pending,null,{timeout:120000});
  assert.match(await page.locator('#trajectory-status').textContent(),/86400 SECOND MODEL/);
  await page.waitForFunction(()=>odderonDiagnostics.quality.includes('RESTING')&&!odderonDiagnostics.pendingFrame,null,{timeout:60000});
  const afterWorker=await page.evaluate(()=>({frames:window.testFrameCallbacks,messages:window.testWorkerMessages}));
  await page.waitForTimeout(2000);
  assert.deepEqual(await page.evaluate(()=>({frames:window.testFrameCallbacks,messages:window.testWorkerMessages})),afterWorker,'trajectory completion must return to idle');
  const startedBurn=await page.evaluate(()=>odderonSetOrbit({position:[7e6,0,0],velocity:[0,7600,2000]},3.986004418e14,null,{duration:2400,mass:500,burn:{start:1200,end:1260,force:[0,5,0],isp:450}}));
  assert.equal(startedBurn,true);
  await page.waitForFunction(()=>odderonDiagnostics.trajectory.samples===241&&!odderonDiagnostics.trajectory.pending,null,{timeout:120000});
  const finalMass=await page.evaluate(()=>odderonDiagnostics.trajectory.finalMass);
  assert.ok(Math.abs(finalMass-(500-300/(450*9.80665)))<1e-8,'worker burn mass must honor exact duration');
  const fit=await page.evaluate(()=>{
    const radius=2e8,mu=3.986004418e14;
    const accepted=window.odderonSetOrbit({position:[radius,0,0],velocity:[0,Math.sqrt(mu/radius),0]},mu);
    const p=odderonDiagnostics.orbitProjection, rect=document.getElementById('scene').getBoundingClientRect();
    return {accepted,p,aspect:rect.width/rect.height};
  });
  assert.ok(fit.accepted&&Math.abs(fit.p[0])<fit.aspect&&Math.abs(fit.p[1])<1,'distant craft must fit the viewport');

  // Rejected inputs and cancelled workers must not leave stale model results.
  assert.equal(await page.evaluate(()=>odderonSetOrbit(null,3.986004418e14)),false);
  assert.equal(await page.locator('#trajectory-status').textContent(),'L3 · NOT CALCULATED');
  assert.equal(await page.evaluate(()=>odderonDiagnostics.trajectory.samples),0);
  assert.equal(await page.evaluate(()=>odderonSetOrbit({position:[7e6,0,0],velocity:[0,7600,2000]},3.986004418e14)),true);
  await page.locator('#show-orbit').uncheck();
  assert.equal(await page.locator('#trajectory-status').textContent(),'L3 · NOT CALCULATED');
  assert.equal(await page.evaluate(()=>odderonDiagnostics.trajectory.pending),false);
  const beforeDate=await page.locator('#distance').textContent();await page.locator('#next').click();assert.notEqual(await page.locator('#distance').textContent(),beforeDate);
  assert.equal(await page.locator('#date').inputValue(),'2026-01-02');
  await page.locator('#date').fill('2020-01-01');await page.locator('#date').dispatchEvent('change');assert.equal(await page.locator('#prev').isDisabled(),true);
  await page.locator('#date').fill('2035-12-31');await page.locator('#date').dispatchEvent('change');assert.equal(await page.locator('#next').isDisabled(),true);
  await page.locator('#date').fill('2019-01-01');await page.locator('#date').dispatchEvent('change');assert.equal(await page.locator('#error').isVisible(),true);assert.equal(await page.locator('#distance').textContent(),'—');assert.equal(await page.locator('#world').isVisible(),false);assert.equal(await page.locator('#overlay').isVisible(),false);assert.equal(await page.locator('#next').isDisabled(),true);
  await page.locator('#today-reset').click();await page.waitForFunction(()=>getComputedStyle(document.getElementById('world')).visibility==='visible');assert.equal(await page.locator('#error').isHidden(),true);await page.locator('#show-path').uncheck();await page.locator('#show-velocity').check();await page.locator('#show-vehicle').check();await page.locator('#zoom-in').click();await page.locator('#reset').click();
  await page.locator('#scene').focus();await page.keyboard.press('ArrowRight');
  await page.waitForTimeout(300);
  const glResult=await page.locator('#world').evaluate(canvas=>{const gl=canvas.getContext('webgl2');return {error:gl.getError(),w:canvas.width,h:canvas.height};});
  assert.equal(glResult.error,0,'WebGL error');assert.ok(glResult.w>200&&glResult.h>200);
  if(process.env.ODDERON_SCREENSHOT)await page.screenshot({path:process.env.ODDERON_SCREENSHOT,fullPage:true});
  const ext=await page.locator('#world').evaluate(c=>{window.testLoseContext=c.getContext('webgl2').getExtension('WEBGL_lose_context');return !!window.testLoseContext;});
  if(ext){await page.locator('#world').evaluate(c=>window.testLoseContext.loseContext());await page.waitForFunction(()=>!document.getElementById('error').hidden);await page.locator('#date').fill('2019-01-01');await page.locator('#date').dispatchEvent('change');await page.locator('#world').evaluate(c=>window.testLoseContext.restoreContext());await page.waitForFunction(()=>document.getElementById('error').textContent.includes('Date outside'),{timeout:10000});await page.locator('#today-reset').click();await page.waitForFunction(()=>document.getElementById('error').hidden,{timeout:10000});}
  await page.setViewportSize({width:390,height:844});await page.waitForTimeout(300);assert.equal(await page.locator('#error').isHidden(),true);
  assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),true,'mobile horizontal overflow');
  // Experimental rescue mission: create two actual nodes, lock a prediction,
  // run the finite-burn worker, and ensure the completed screen goes idle.
  await page.setViewportSize({width:1440,height:1000});
  await page.goto(`http://127.0.0.1:${server.address().port}/mission.html`);
  await page.waitForFunction(()=>!document.getElementById('add-node').disabled,null,{timeout:30000});
  // WP 1.3 snapshot gate: every rendered registry acronym is wrapped and has
  // a live hover projection; the causal chains are the Rust engine's DAG.
  assert.deepEqual(await page.evaluate(()=>missionDiagnostics.acronymViolations),[]);
  assert.equal(await page.evaluate(()=>missionDiagnostics.causalValid),true);
  const acronymSnapshot=await page.evaluate(()=>[...document.querySelectorAll('abbr.term')].map(a=>a.dataset.term).sort());
  for(const term of ['RAAN','AOP','TWR','ISP'])assert.ok(acronymSnapshot.includes(term),`missing rendered acronym ${term}`);
  // Art keys must arrive from odd_app::lens::Projection::key over the bridge.
  // If that export broke, every term would silently fall back to the
  // tangential demo, so assert the distinct keys the registry should produce.
  const artKeys=await page.evaluate(async()=>{
    const {instance}=await WebAssembly.instantiate(await (await fetch('./odd_view.wasm')).arrayBuffer(),{});
    const w=instance.exports,dec=new TextDecoder(),keys={};
    for(let i=0;i<w.mission_acronym_count();i++){
      const read=(len,byte)=>dec.decode(Uint8Array.from({length:len},(_,j)=>byte(j)));
      const term=read(w.mission_acronym_len(i,0),j=>w.mission_acronym_byte(i,0,j));
      keys[term]=read(w.mission_acronym_key_len(i),j=>w.mission_acronym_key_byte(i,j));
    }
    return keys;
  });
  assert.equal(artKeys.TWR,'twr');
  assert.equal(artKeys.TNW,'tnw');
  assert.equal(artKeys.LEO,'leo');
  assert.equal(artKeys.DAG,'dag');
  assert.equal(artKeys.PDR,'pdr');
  assert.ok(Object.values(artKeys).every(k=>k.length>0),'every acronym needs an art key');

  // oracle_identity_roundtrip_and_rfc8032 (wasm32 half): the compiled module
  // must sign identically to the native build, not merely compile.
  const identity=await page.evaluate(async()=>{
    const {instance}=await WebAssembly.instantiate(await (await fetch('./odd_view.wasm')).arrayBuffer(),{});
    const w=instance.exports,dec=new TextDecoder(),enc=new TextEncoder();
    const read=(len,byte)=>dec.decode(Uint8Array.from({length:len},(_,j)=>byte(j)));
    const absorb=bytes=>{w.mission_input_reset();for(const b of bytes)if(!w.mission_input_absorb(b))throw Error('absorb failed');};
    const str=field=>read(w.mission_identity_string_len(field),i=>w.mission_identity_string_byte(field,i));
    const out={vectors:w.mission_ed25519_selftest(),words:w.mission_word_count()};
    out.loaded=w.mission_identity_from_entropy(0x00112233,0x44556677,0x8899aabb,0xccddeeff);
    out.key=str(2);out.mnemonic=str(1);out.publicId=str(0);
    out.pubkey=Array.from({length:32},(_,i)=>w.mission_identity_pubkey(i));
    // Restore through each encoding and confirm the same public key returns.
    absorb(enc.encode(out.mnemonic));
    out.mnemonicValid=w.mission_identity_mnemonic_valid();
    out.mnemonicRestore=w.mission_identity_from_mnemonic();
    out.fromMnemonic=Array.from({length:32},(_,i)=>w.mission_identity_pubkey(i));
    absorb(enc.encode(out.key));
    out.keyValid=w.mission_identity_key_valid();
    out.keyRestore=w.mission_identity_from_key();
    out.fromKey=Array.from({length:32},(_,i)=>w.mission_identity_pubkey(i));
    // A one-word substitution must be rejected by the checksum.
    const words=out.mnemonic.split(' ');
    const replacement=read(w.mission_word_len(0),i=>w.mission_word_byte(0,i));
    words[3]=words[3]===replacement?read(w.mission_word_len(1),i=>w.mission_word_byte(1,i)):replacement;
    absorb(enc.encode(words.join(' ')));
    out.corruptRejected=w.mission_identity_mnemonic_valid()===0;
    // Sign, verify, then flip one message bit.
    const message=enc.encode('odderon canonical flight data recorder');
    absorb(message);
    out.signed=w.mission_identity_sign();
    const signature=Array.from({length:w.mission_signature_len()},(_,i)=>w.mission_signature_byte(i));
    w.mission_verify_reset();
    for(const b of out.pubkey)w.mission_verify_absorb(0,b);
    for(const b of signature)w.mission_verify_absorb(1,b);
    out.verifies=w.mission_verify();
    absorb(enc.encode('odderon canonical flight data Recorder'));
    out.tamperRejected=w.mission_verify()===0;
    out.signatureLength=signature.length;
    return out;
  });
  assert.equal(identity.vectors,3,'RFC 8032 vectors must reproduce inside wasm32');
  assert.equal(identity.words,2048);
  assert.equal(identity.loaded,1);
  assert.equal(identity.key,'ODD-0011-2233-4455-6677-8899-AABB-CCDD-EEFF');
  assert.equal(identity.mnemonic.split(' ').length,12);
  assert.match(identity.publicId,/^ODD-ID-[A-Z2-7]{16}$/);
  assert.equal(identity.mnemonicValid,1);
  assert.equal(identity.mnemonicRestore,1);
  assert.equal(identity.keyValid,1);
  assert.equal(identity.keyRestore,1);
  assert.deepEqual(identity.fromMnemonic,identity.pubkey,'mnemonic must restore the same identity');
  assert.deepEqual(identity.fromKey,identity.pubkey,'flight key must restore the same identity');
  assert.equal(identity.corruptRejected,true,'a substituted word must fail the checksum');
  assert.equal(identity.signed,1);
  assert.equal(identity.signatureLength,64);
  assert.equal(identity.verifies,1);
  assert.equal(identity.tamperRejected,true,'a flipped message bit must not verify');
  // Let the one-shot flight worker finish first: it rebuilds the causal panel,
  // so hovering while it is still running would race a DOM replacement.
  await page.waitForFunction(()=>!missionDiagnostics.pending,null,{timeout:60000});
  await page.locator('abbr.term[data-term="TWR"]').first().hover();
  await page.waitForFunction(()=>missionDiagnostics.lens==='twr'&&!document.getElementById('lens-projection').hidden);
  assert.match(await page.locator('#lens-projection').textContent(),/Thrust-to-weight ratio/);
  const lensBefore=await page.evaluate(()=>missionDiagnostics.lensDraws);
  await page.waitForFunction(n=>missionDiagnostics.lensDraws>n,lensBefore);
  // The projection renders the hovered term inside itself. Re-entering the
  // active projection must not rewrite those nodes, or the element under the
  // pointer is destroyed and mouseover re-fires forever.
  const stable=await page.evaluate(async()=>{
    const proj=document.getElementById('lens-projection');
    const node=proj.querySelector('abbr.term[data-term="TWR"]');
    if(!node)return 'no-self-term';
    node.dispatchEvent(new MouseEvent('mouseover',{bubbles:true}));
    node.dispatchEvent(new MouseEvent('mouseover',{bubbles:true}));
    await new Promise(r=>setTimeout(r,120));
    return node.isConnected&&proj.querySelector('abbr.term[data-term="TWR"]')===node?'stable':'detached';
  });
  assert.equal(stable,'stable','hovering a term inside the projection must not detach it');
  // Travelling from a hovered term to the Close button crosses a leave
  // boundary. If that closes the lens, the button vanishes mid-click and can
  // never be pressed. Hover the projection's own surviving term first so the
  // pointer really does leave a `.term` on its way to the button.
  await page.locator('#lens-projection abbr.term[data-term="TWR"]').hover();
  assert.equal(await page.evaluate(()=>missionDiagnostics.lens),'twr');
  await page.locator('#lens-close').hover();
  assert.equal(await page.evaluate(()=>missionDiagnostics.lens),'twr','moving onto Close must not close the lens');
  assert.equal(await page.locator('#lens-close').isVisible(),true);
  await page.locator('#lens-close').click();
  await page.waitForFunction(()=>missionDiagnostics.lens===null&&!missionDiagnostics.lensPending);
  // Raw mouse coordinates are viewport-relative, so bring the canvas on
  // screen first instead of depending on the height of neighbouring panels.
  await page.locator('#plan-canvas').scrollIntoViewIfNeeded();
  const missionBox=await page.locator('#plan-canvas').boundingBox();
  await page.mouse.click(missionBox.x+missionBox.width/2+(6378137+260000)*missionBox.height/(2.45*(6378137+550000)),missionBox.y+missionBox.height/2);
  assert.equal(await page.evaluate(()=>missionDiagnostics.nodes.length),1,'clicking the path should place a maneuver');
  await page.locator('#remove-node').click();
  await page.locator('#add-node').click();

  // oracle_hardening_clamp_and_exact_sweep (browser half): typed entry and
  // pointer dragging must enforce one identical [t_min, t_max] boundary.
  {
    const [lo,hi]=await page.evaluate(()=>missionDiagnostics.nodeTimeRange);
    assert.equal(await page.locator('#node-time').getAttribute('min'),String(lo),'input min must match the shared clamp');
    assert.equal(await page.locator('#node-time').getAttribute('max'),String(hi),'input max must match the shared clamp');
    for(const [typed,expected] of [['-500',lo],['0',lo],[String(lo-1),lo],[String(hi+1),hi],['99999999',hi]]){
      await page.locator('#node-time').fill(typed);
      assert.equal(await page.evaluate(()=>missionDiagnostics.nodes[0].time),expected,`typed ${typed} should clamp to ${expected}`);
    }
    // A node at the upper boundary can still fail to compile because its
    // finite burn overruns the flight end; that is plan validation, not the
    // clamp. Restore an interior time so the preview is populated again.
    await page.locator('#node-time').fill('200');
    await page.waitForFunction(()=>missionDiagnostics.pathEnds!==null);
    await page.locator('#plan-canvas').scrollIntoViewIfNeeded();
    const box=await page.locator('#plan-canvas').boundingBox();
    const ends=await page.evaluate(()=>missionDiagnostics.pathEnds);
    assert.ok(ends,'preview exposes its endpoints');
    const marker=await page.evaluate(()=>missionDiagnostics.nodeMarker);
    assert.ok(marker,'selected node exposes a canvas marker');
    // Drag onto the first preview sample, whose raw time is 0. The pointer
    // path used to clamp this to a 60 s margin while the field allowed 1 s.
    await page.mouse.move(box.x+marker[0],box.y+marker[1]);
    await page.mouse.down();
    await page.mouse.move(box.x+ends[0][0],box.y+ends[0][1],{steps:8});
    await page.mouse.up();
    const draggedLow=await page.evaluate(()=>missionDiagnostics.nodes[0].time);
    assert.equal(draggedLow,lo,'dragging to the path start must clamp to the shared minimum');
    assert.ok(draggedLow<60,'the old 60 s pointer margin must be gone');
    await page.locator('#node-time').fill('-1000');
    assert.equal(await page.evaluate(()=>missionDiagnostics.nodes[0].time),draggedLow,'dragged and typed lower clamps must agree exactly');
    // Sweep the marker along the path. Every intermediate drag result must
    // already satisfy the shared clamp. The upper boundary itself is not
    // reachable by dragging: a node that close to the end fails plan
    // validation because its burn overruns the flight, which clears the
    // preview. That is physics validation, so the shared helper is checked
    // directly for the upper bound instead.
    await page.locator('#node-time').fill('200');
    for(const fraction of [0.25,0.5,0.75]){
      const live=await page.evaluate(()=>missionDiagnostics.pathEnds);
      const from=await page.evaluate(()=>missionDiagnostics.nodeMarker);
      await page.mouse.move(box.x+from[0],box.y+from[1]);
      await page.mouse.down();
      await page.mouse.move(box.x+live[0][0]+(live[1][0]-live[0][0])*fraction,box.y+live[0][1]+(live[1][1]-live[0][1])*fraction,{steps:4});
      await page.mouse.up();
      const t=await page.evaluate(()=>missionDiagnostics.nodes[0].time);
      assert.equal(await page.evaluate(v=>missionDiagnostics.clampNodeTime(v),t),t,`dragged time ${t} must already be inside the shared clamp`);
      await page.locator('#node-time').fill('200');
    }
    assert.equal(await page.evaluate(v=>missionDiagnostics.clampNodeTime(v),hi+1000),hi,'shared clamp must bind at the upper boundary');
    await page.locator('#node-time').fill('200');
  }

  // oracle_browser_cad_lab_and_idle_gate: design the engine on the canvas,
  // watch the Schlieren field and the dossier inputs respond, then fly it.
  {
    await page.waitForFunction(()=>missionDiagnostics.engine?.solution,null,{timeout:60000});
    const start=await page.evaluate(()=>missionDiagnostics.engine);
    assert.equal(start.code,1,'the contract baseline must solve');
    assert.equal(start.stations,96,'the axial flow field must be sampled');
    assert.equal(start.raster,true,'the Schlieren raster must be built in Rust');
    // Let the flight worker finish first: its causal panel sits above the lab
    // and changes height when it lands, which would move the canvas between
    // measuring the handle and pressing on it.
    await page.waitForFunction(()=>!missionDiagnostics.pending,null,{timeout:120000});
    // Moving the pointer off the planner closes any open physics-lens
    // projection, which shrinks the column above the lab. Settle that first
    // so the handle coordinates are measured against the final layout.
    await page.mouse.move(4,4);
    await page.locator('#engine-canvas').scrollIntoViewIfNeeded();
    await page.waitForFunction(()=>missionDiagnostics.engine?.handles.length===5&&!missionDiagnostics.engine.pending&&missionDiagnostics.lens===null,null,{timeout:30000});
    const labBox=await page.locator('#engine-canvas').boundingBox();
    const settled=await page.evaluate(()=>missionDiagnostics.engine);
    const exitHandle=settled.handles.find(h=>h.key==='exit');
    assert.ok(exitHandle,'the exit radius must be a draggable handle');
    await page.mouse.move(labBox.x+exitHandle.at[0],labBox.y+exitHandle.at[1]);
    await page.mouse.down();
    await page.mouse.move(labBox.x+exitHandle.at[0]+4,labBox.y+exitHandle.at[1]-24,{steps:6});
    await page.mouse.up();
    const widened=await page.evaluate(()=>missionDiagnostics.engine);
    assert.ok(widened.spec.exit>start.spec.exit,'dragging the exit handle must widen the bell');
    assert.ok(widened.draws>start.draws,'the cross-section must repaint');
    assert.ok(widened.solution.vacuumIsp!==start.solution.vacuumIsp,'a bigger bell must change vacuum Isp');
    // The spec requires O/F and chamber pressure to be draggable on the
    // cross-section itself, not only reachable through the numeric controls.
    {
      const dragHandle=async(key,dx,dy)=>{
        await page.waitForFunction(()=>missionDiagnostics.engine?.handles.length===5&&!missionDiagnostics.engine.pending,null,{timeout:30000});
        const box=await page.locator('#engine-canvas').boundingBox();
        const h=await page.evaluate(k=>missionDiagnostics.engine.handles.find(x=>x.key===k),key);
        assert.ok(h,`${key} must be a draggable handle on the cross-section`);
        await page.mouse.move(box.x+h.at[0],box.y+h.at[1]);
        await page.mouse.down();
        await page.mouse.move(box.x+h.at[0]+dx,box.y+h.at[1]+dy,{steps:6});
        await page.mouse.up();
      };
      const beforeOf=await page.evaluate(()=>missionDiagnostics.engine.spec.of);
      await dragHandle('of',-40,0);
      const afterOf=await page.evaluate(()=>missionDiagnostics.engine.spec.of);
      assert.ok(afterOf<beforeOf,`dragging the mixture track left must richen it: ${beforeOf} -> ${afterOf}`);
      const beforePc=await page.evaluate(()=>missionDiagnostics.engine.spec.pc);
      await dragHandle('pc',0,-40);
      const afterPc=await page.evaluate(()=>missionDiagnostics.engine.spec.pc);
      assert.ok(afterPc>beforePc,`dragging the injector up must raise chamber pressure: ${beforePc} -> ${afterPc}`);
    }
    // Mixture ratio: the efficiency peak is fuel-rich, the temperature peak
    // is not. Both come from the same solve the recorder will sign.
    await page.locator('#engine-propellant').selectOption('0');
    const mixture=[];
    for(const ratio of [4.8,6.0,7.94]){
      await page.locator('#engine-of').fill(String(ratio));
      await page.locator('#engine-of').dispatchEvent('input');
      const d=await page.evaluate(()=>missionDiagnostics.engine);
      mixture.push({ratio,isp:d.solution.vacuumIsp,tc:d.solution.tc,molar:d.solution.molar});
    }
    assert.ok(mixture[0].isp>mixture[2].isp,'fuel-rich must beat stoichiometric on Isp');
    assert.ok(mixture[2].tc>mixture[0].tc,'stoichiometric must run hotter');
    assert.ok(mixture[2].molar>mixture[0].molar,'fuel-rich exhaust must be lighter');
    assert.match(await page.locator('#engine-causal').textContent(),/near the temperature peak/);
    // Altitude: a big bell separates at sea level and re-attaches on the way up.
    await page.locator('#engine-propellant').selectOption('1');
    await page.locator('#engine-of').fill('2.36');await page.locator('#engine-of').dispatchEvent('input');
    await page.locator('#engine-pc').fill('5');
    await page.locator('#engine-throat').fill('50');
    await page.locator('#engine-exit').fill('447.2');
    await page.locator('#engine-altitude').fill('0');await page.locator('#engine-altitude').dispatchEvent('input');
    const sea=await page.evaluate(()=>missionDiagnostics.engine);
    assert.equal(sea.plume.regime,3,'sea level must separate inside the bell');
    assert.ok(sea.solution.separationX>0.1&&sea.solution.separationX<0.9);
    assert.match(await page.locator('#engine-causal').textContent(),/separates inside the bell/);
    await page.locator('#engine-altitude').fill('35');await page.locator('#engine-altitude').dispatchEvent('input');
    const high=await page.evaluate(()=>missionDiagnostics.engine);
    assert.notEqual(high.plume.regime,3,'the bell must flow full above 35 km');
    assert.ok(high.solution.vacuumIsp>sea.solution.isp,'vacuum Isp must beat the separated stand');
    // Fit the contract baseline back so the certified rescue stays flyable,
    // then apply a modest redesign and check the stage really moved.
    await page.locator('#engine-reset').click();
    await page.waitForFunction(()=>!missionDiagnostics.pending,null,{timeout:60000});
    await page.locator('#engine-of').fill('2.1');await page.locator('#engine-of').dispatchEvent('input');
    const redesign=await page.evaluate(()=>missionDiagnostics.engine);
    await page.locator('#engine-apply').click();
    await page.waitForFunction(()=>!missionDiagnostics.pending,null,{timeout:120000});
    const fitted=await page.evaluate(()=>({lab:missionDiagnostics.engine,applied:missionDiagnostics.appliedEngine,thrust:missionDiagnostics.wasm().mission_parameter(4),isp:missionDiagnostics.wasm().mission_parameter(5),wet:missionDiagnostics.wasm().mission_parameter(2)}));
    assert.ok(fitted.lab.solution.stageWet>0,'the stage must be rebuilt around the design');
    assert.equal(await page.evaluate(()=>missionDiagnostics.causalValid),true,'the combined causal graph must stay acyclic');
    assert.ok(Math.abs(fitted.lab.spec.of-redesign.spec.of)<1e-9,'the applied design must be the drawn one');
    // Regression: the planner, the worker and the recorder must all fly the
    // committed design. mission_contract() rebuilds the baseline, so a
    // reconfigure that forgets to re-apply would preview one stage and fly
    // another while still producing a valid signature.
    assert.ok(fitted.applied,'fitting must commit a design');
    assert.equal(fitted.thrust,fitted.lab.solution.vacuumThrust,'the planner stage must use the fitted engine');
    assert.equal(fitted.isp,fitted.lab.solution.vacuumIsp);
    assert.equal(fitted.wet,fitted.lab.solution.stageWet);
    // A lab edit that was never applied must not reach the flight.
    await page.locator('#engine-pc').fill('7');
    await page.locator('#engine-pc').dispatchEvent('input');
    await page.locator('#add-node').click();
    await page.waitForFunction(()=>!missionDiagnostics.pending,null,{timeout:120000});
    const unapplied=await page.evaluate(()=>({applied:missionDiagnostics.appliedEngine,labPc:missionDiagnostics.engine.spec.pc,thrust:missionDiagnostics.wasm().mission_parameter(4)}));
    assert.equal(unapplied.labPc,7e6,'the lab previews the edit');
    assert.equal(unapplied.applied.pc,fitted.applied.pc,'an unapplied edit must not be committed');
    assert.equal(unapplied.thrust,fitted.thrust,'an unapplied edit must not change the flown stage');
    await page.locator('#remove-node').click();
    await page.locator('#engine-pc').fill(String(fitted.applied.pc/1e6));
    await page.locator('#engine-pc').dispatchEvent('input');
    await page.waitForFunction(()=>!missionDiagnostics.pending,null,{timeout:120000});
  }

  // Fitting a new engine clears the plan: the old burn durations were computed
  // for the old thrust and mass, so they must not survive the redesign.
  assert.equal(await page.evaluate(()=>missionDiagnostics.nodes.length),0,'a redesign must discard the stale plan');
  await page.locator('#add-node').click();
  await page.locator('#node-t').fill('82.38547669');
  await page.locator('#add-node').click();
  await page.locator('#node-time').fill('2979.885241');
  await page.locator('#node-t').fill('81.50942505');
  assert.equal(await page.evaluate(()=>missionDiagnostics.nodes.length),2);
  await page.locator('#node-w').focus();
  assert.match(await page.locator('#lens-text').textContent(),/perpendicular to the orbital plane/);
  const handle=await page.evaluate(()=>missionDiagnostics.handles.find(h=>h.key==='t').end);
  await page.locator('#plan-canvas').scrollIntoViewIfNeeded();
  const canvasBox=await page.locator('#plan-canvas').boundingBox();
  await page.mouse.move(canvasBox.x+handle[0],canvasBox.y+handle[1]);await page.mouse.down();await page.mouse.move(canvasBox.x+handle[0]+5,canvasBox.y+handle[1]+5,{steps:3});await page.mouse.up();
  assert.notEqual(await page.evaluate(()=>missionDiagnostics.nodes[1].dv[0]),81.50942505,'dragging a handle should change the maneuver');
  await page.locator('#node-t').fill('81.50942505');
  await page.locator('#prefill').click();
  await page.locator('#execute').click();
  await page.waitForFunction(()=>!document.getElementById('debrief').hidden||(missionDiagnostics.locked&&!missionDiagnostics.pending),null,{timeout:120000});
  assert.equal(await page.locator('#debrief').isHidden(),false,`mission error: ${await page.locator('#execute-status').textContent()}, page errors: ${errors}`);
  const debrief=await page.locator('#debrief-content').textContent();
  const score=Number(debrief.match(/([0-9.]+)%Mission score/)?.[1]);
  assert.ok(score>80,`rescue score: ${score}; ${debrief}`);
  assert.equal(await page.evaluate(()=>missionDiagnostics.locked),true);
  // oracle_browser_persistence_and_idle_gate: the first execution of a
  // contract is ranked, moves Astro-Elo, and locks its certificate.
  const firstRated=await page.evaluate(()=>missionDiagnostics.profile());
  assert.ok(firstRated.ranked['101'],'contract 101 must hold a ranked certificate');
  assert.notEqual(firstRated.elo,1200,'a ranked flight must move Astro-Elo off its start value');
  assert.equal(firstRated.elo,firstRated.ranked['101'].elo);
  assert.equal(firstRated.flights,1);
  assert.match(await page.locator('#elo-result').textContent(),/Ranked flight certified/);
  // The executed flight is signed as a ranked certificate.
  assert.match(await page.locator('#recorder-status').textContent(),/Flight recorder signed by ODD-ID-/);
  const rankedDownload=page.waitForEvent('download');
  await page.locator('#export-fdr').click();
  const rankedFdr=JSON.parse(await readFile(await (await rankedDownload).path(),'utf8'));
  assert.equal(rankedFdr.schema_version,2);
  // oracle_fdr_engine_recomputation_anti_cheat (browser half): the signed
  // record carries the physical design, never a client thrust/Isp claim.
  assert.equal(typeof rankedFdr.engine_propellant,'string');
  assert.ok(Number.isInteger(rankedFdr.engine_propellant_id));
  for(const key of ['engine_of_ratio','engine_p_c_pa','engine_r_throat_m','engine_r_exit_m','engine_length_fraction']){
    assert.ok(Number.isFinite(rankedFdr[key])&&rankedFdr[key]>0,`${key} must be a physical design number`);
  }
  assert.ok(Math.abs(rankedFdr.engine_of_ratio-2.1)<1e-9,'the flight must carry the redesigned mixture ratio');
  // The signed design must be the committed one, and its performance must be
  // what that design solves to in vacuum — never a lab preview.
  const committed=await page.evaluate(()=>missionDiagnostics.appliedEngine);
  assert.ok(committed,'the certified flight must have a committed design');
  assert.equal(rankedFdr.engine_p_c_pa,committed.pc);
  assert.equal(rankedFdr.engine_r_throat_m,committed.throat);
  assert.equal(rankedFdr.engine_r_exit_m,committed.exit);
  assert.equal(rankedFdr.contract_id,101);
  assert.equal(rankedFdr.attempt_mode,'Ranked');
  assert.equal(rankedFdr.public_id,firstRated.publicId);
  assert.equal(rankedFdr.metrics.length,58);
  assert.equal(rankedFdr.ed25519_signature.length,128);
  assert.equal(rankedFdr.final_state_hash.length,64);
  assert.ok(rankedFdr.elo_after!==rankedFdr.elo_before,'the rating change must be inside the signed record');
  assert.ok(rankedFdr.command_timeline.length>=1);
  assert.ok(Number.isInteger(rankedFdr.command_timeline[0].time_us));
  // The portability claim is only real if a process with no browser, no wasm
  // and no DOM reaches the same verdict from the same bytes. Hand this exact
  // signed payload to the native CLI, then hand it a one-ulp engine forgery.
  const portable=await page.evaluate(()=>missionDiagnostics.recorder());
  const scratch=await mkdtemp(join(tmpdir(),'odderon-portable-'));
  try{
    const honest=join(scratch,'flight.txt');
    await writeFile(honest,portable.payload);
    const verdict=execFileSync('cargo',['run','--locked','--offline','-q','-p','odd-verify','--example','verify_record','--',honest],{encoding:'utf8'});
    assert.match(verdict,/VERIFIED/,'the native verifier must accept a browser-signed record');
    assert.match(verdict,/contract\s+101/);
    const forged=join(scratch,'forged.json');
    await writeFile(forged,JSON.stringify({...portable.document,engine_r_throat_m:nextUlp(portable.document.engine_r_throat_m)}));
    let rejected=false;
    try{execFileSync('cargo',['run','--locked','--offline','-q','-p','odd-verify','--example','verify_record','--',forged],{encoding:'utf8',stdio:'pipe'});}
    catch(error){rejected=error.status===1;}
    assert.ok(rejected,'a one-ulp engine forgery must be rejected by the native verifier');
    console.log('Portable replay: native CLI verifies the browser-signed record and rejects a one-ulp engine forgery.');
  }finally{await rm(scratch,{recursive:true,force:true});}
  await page.waitForFunction(()=>!missionDiagnostics.pending,null,{timeout:10000});
  const missionIdle=await page.evaluate(()=>({frames:testFrameCallbacks,messages:testWorkerMessages,draws:missionDiagnostics.draws}));
  await page.waitForTimeout(2000);
  assert.deepEqual(await page.evaluate(()=>({frames:testFrameCallbacks,messages:testWorkerMessages,draws:missionDiagnostics.draws})),missionIdle,'mission must settle without frames, redraws, or worker messages');
  await page.locator('#unlock').click();
  assert.equal(await page.evaluate(()=>missionDiagnostics.locked),false);
  await page.locator('#mission-select').selectOption('102');
  await page.locator('#example').click();
  assert.equal(await page.evaluate(()=>missionDiagnostics.nodes.length),1);
  await page.locator('#execute').click();
  await page.waitForFunction(()=>!document.getElementById('debrief').hidden||(missionDiagnostics.locked&&!missionDiagnostics.pending),null,{timeout:180000});
  assert.equal(await page.locator('#debrief').isHidden(),false,`mission error: ${await page.locator('#execute-status').textContent()}, page errors: ${errors}`);
  const sso=await page.locator('#debrief-content').textContent();
  const ssoScore=Number(sso.match(/([0-9.]+)%Mission score/)?.[1]);
  assert.ok(ssoScore>80,`sun-synchronous score: ${ssoScore}; ${sso}`);
  const j2=Number(sso.match(/Earth equatorial bulge: ([0-9.]+)%/)?.[1]);
  assert.ok(j2>85,`nodal Shapley attribution must be dominated by equatorial bulge: ${sso}`);
  const ssoReport=await page.evaluate(()=>missionDiagnostics.report);
  assert.ok(Math.abs(ssoReport.slice(6,10).reduce((a,b)=>a+b,0)-1)<1e-9);
  assert.ok(Math.abs(ssoReport[17])<1e-9);
  assert.deepEqual(await page.evaluate(()=>missionDiagnostics.acronymViolations),[]);
  await page.waitForFunction(()=>!missionDiagnostics.pending,null,{timeout:10000});
  // Contract 103: demonstrate the single-burn trap, then use the numerical
  // perigee splitter, lock a prediction, and verify a surviving apoapsis and
  // finite-burn-dominated sensitivity. The patched lunar preview is optional
  // for this epoch; a radius target is not itself a lunar encounter.
  await page.locator('#unlock').click();
  await page.locator('#mission-select').selectOption('103');
  await page.locator('#example').click();
  await page.waitForFunction(()=>!missionDiagnostics.pending&&missionDiagnostics.report?.[30]===1,null,{timeout:180000});
  assert.match(await page.locator('#causal-text').textContent(),/Impact/);
  await page.locator('#split-kick').click();
  await page.waitForFunction(()=>missionDiagnostics.nodes.length===3&&!missionDiagnostics.pending&&missionDiagnostics.report?.[30]===0,null,{timeout:240000});
  const tugNodes=await page.evaluate(()=>missionDiagnostics.nodes);
  assert.ok(tugNodes[1].guidance&&tugNodes[2].guidance,'later kicks must carry numerical guidance');
  // The tug now flies a physically derived engine (18.087 kN, 449.91 s) rather
  // than the card's rounded 18 kN / 450 s, so its burns are slightly shorter
  // and the targeted periapsis lands a couple of seconds later.
  assert.ok(Math.abs(tugNodes[1].time-10303.134747)<2,`second kick epoch ${tugNodes[1].time}`);
  await page.locator('#prefill').click();
  // Predict the achieved apoapsis radius convention in altitude fields.
  const tugReport=await page.evaluate(()=>missionDiagnostics.report);
  await page.locator('#predict-apo').fill(String(tugReport[1]/1000));
  await page.locator('#predict-peri').fill(String(tugReport[2]/1000));
  await page.locator('#predict-time').fill(String(tugReport[29]));
  await page.locator('#execute').click();
  await page.waitForFunction(()=>!document.getElementById('debrief').hidden||(missionDiagnostics.locked&&!missionDiagnostics.pending),null,{timeout:240000});
  assert.equal(await page.locator('#debrief').isHidden(),false,`mission error: ${await page.locator('#execute-status').textContent()}, page errors: ${errors}`);
  const tug=await page.locator('#debrief-content').textContent();
  const tugScore=Number(tug.match(/([0-9.]+)%Mission score/)?.[1]);
  assert.ok(tugScore>80,`tug score: ${tugScore}; ${tug}`);
  const tugFinite=Number(tug.match(/Finite-burn arc: ([0-9.]+)%/)?.[1]);
  assert.ok(tugFinite>70,`finite-burn apoapsis Shapley attribution: ${tug}`);
  const finalTug=await page.evaluate(()=>missionDiagnostics.report);
  assert.equal(finalTug[30],0);assert.ok(Math.abs(finalTug[1]+6378137-380000000)<15000000);
  assert.ok(Math.abs(finalTug.slice(6,10).reduce((a,b)=>a+b,0)-1)<1e-9);
  assert.ok(Math.abs(finalTug[17])<1e-6);
  assert.ok(Math.abs(finalTug[24])<1e-4,`finite-burn identity residual ${finalTug[24]} m/s`);
  assert.ok(Math.abs(finalTug[24])/finalTug[20]<1e-8);
  assert.deepEqual(await page.evaluate(()=>missionDiagnostics.acronymViolations),[]);
  assert.equal(await page.locator('#export-pdr').isVisible(),true);
  await page.waitForFunction(()=>!missionDiagnostics.pending,null,{timeout:10000});
  const tugIdle=await page.evaluate(()=>({frames:testFrameCallbacks,messages:testWorkerMessages,draws:missionDiagnostics.draws}));
  await page.waitForTimeout(2000);
  assert.deepEqual(await page.evaluate(()=>({frames:testFrameCallbacks,messages:testWorkerMessages,draws:missionDiagnostics.draws})),tugIdle,'Contract 103 must settle without frames, redraws, or worker messages');
  // oracle_browser_persistence_and_idle_gate: identity and rating must
  // survive a full document reload, not merely live in module state.
  const beforeReload=await page.evaluate(()=>missionDiagnostics.profile());
  assert.deepEqual(Object.keys(beforeReload.ranked).sort(),['101','102','103']);
  await page.reload();
  await page.waitForFunction(()=>!document.getElementById('add-node').disabled,null,{timeout:30000});
  const afterReload=await page.evaluate(()=>missionDiagnostics.profile());
  assert.equal(afterReload.publicId,beforeReload.publicId,'the license must survive a reload');
  assert.deepEqual(afterReload.publicKey,beforeReload.publicKey);
  assert.equal(afterReload.mnemonic,beforeReload.mnemonic);
  assert.equal(afterReload.elo,beforeReload.elo,'Astro-Elo must survive a reload');
  assert.deepEqual(afterReload.ranked,beforeReload.ranked);
  assert.equal(await page.locator('#topbar-elo').textContent(),beforeReload.elo.toFixed(0));

  // The profile dialog exposes the backup and restores from it.
  await page.locator('#open-profile').click();
  assert.equal(await page.locator('#profile-dialog').isVisible(),true);
  assert.equal(await page.locator('#profile-public-id').textContent(),beforeReload.publicId);
  await page.locator('#profile-reveal summary').click();
  assert.equal(await page.locator('#copy-mnemonic-value').textContent(),beforeReload.mnemonic);
  assert.equal(await page.locator('#copy-flight-key-value').textContent(),beforeReload.flightKey);
  const download=page.waitForEvent('download');
  await page.locator('#profile-download').click();
  const backup=JSON.parse(await readFile(await (await download).path(),'utf8'));
  assert.equal(backup.kind,'odderon-flight-license');
  assert.equal(backup.publicId,beforeReload.publicId);
  assert.equal(backup.mnemonic,beforeReload.mnemonic);
  assert.equal(backup.elo,beforeReload.elo);
  // Live validation must reject a corrupted mnemonic and accept the real one.
  await page.locator('#profile-restore-input').fill('not twelve real words at all');
  assert.equal(await page.locator('#profile-restore').isDisabled(),true);
  await page.locator('#profile-restore-input').fill(backup.mnemonic);
  assert.equal(await page.locator('#profile-restore').isEnabled(),true);
  await page.locator('#profile-restore').click();
  assert.match(await page.locator('#profile-restore-status').textContent(),/Restored ODD-ID-/);
  assert.equal(await page.evaluate(()=>missionDiagnostics.profile().elo),beforeReload.elo,'restoring the same license must keep its rating');
  await page.locator('#profile-callsign').fill('GATE-PILOT');
  await page.locator('#profile-callsign-form button[type=submit]').click();
  assert.equal(await page.evaluate(()=>missionDiagnostics.profile().callsign),'GATE-PILOT');
  await page.locator('#profile-close').click();
  assert.equal(await page.locator('#profile-dialog').isVisible(),false);
  await page.waitForFunction(()=>!missionDiagnostics.pending,null,{timeout:10000});
  const profileIdle=await page.evaluate(()=>({frames:testFrameCallbacks,messages:testWorkerMessages,draws:missionDiagnostics.draws}));
  await page.waitForTimeout(2000);
  assert.deepEqual(await page.evaluate(()=>({frames:testFrameCallbacks,messages:testWorkerMessages,draws:missionDiagnostics.draws})),profileIdle,'the profile dialog must settle to zero frames and zero worker messages');

  // The official ranked certificate must stay retrievable after the reload:
  // the rating is locked, so re-flying would only produce a practice record.
  await page.locator('#open-profile').click();
  const certificateDownload=page.waitForEvent('download');
  await page.locator('.certificate-button[data-contract="101"]').first().click();
  const certificate=JSON.parse(await readFile(await (await certificateDownload).path(),'utf8'));
  assert.equal(certificate.attempt_mode,'Ranked','the stored certificate must be the ranked record');
  assert.equal(certificate.contract_id,101);
  assert.equal(certificate.public_id,beforeReload.publicId);
  assert.equal(certificate.canonical_digest,rankedFdr.canonical_digest,'it must be the very document issued at certification');
  await page.locator('.certificate-verify[data-contract="101"]').click();
  await page.waitForFunction(()=>missionDiagnostics.verification()?.code===1,null,{timeout:180000});
  assert.equal(await page.locator('#verify-verdict').textContent(),'\u2713 CRYPTOGRAPHICALLY & PHYSICALLY VERIFIED');
  await page.locator('#verify-close').click();

  // Replaying a certified contract must be practice and must not farm Elo.
  assert.equal(await page.evaluate(()=>missionDiagnostics.isRanked('101')),true);
  assert.equal(await page.locator('#attempt-mode').inputValue(),'practice');
  assert.equal(await page.locator('#attempt-mode option[value=ranked]').isDisabled(),true);
  await page.locator('#example').click();
  await page.locator('#prefill').click();
  await page.locator('#execute').click();
  await page.waitForFunction(()=>!document.getElementById('debrief').hidden||(missionDiagnostics.locked&&!missionDiagnostics.pending),null,{timeout:120000});
  assert.equal(await page.locator('#debrief').isHidden(),false,`practice replay error: ${await page.locator('#execute-status').textContent()}`);
  assert.match(await page.locator('#elo-result').textContent(),/Practice flight — Astro-Elo unchanged/);
  const afterPractice=await page.evaluate(()=>missionDiagnostics.profile());
  assert.equal(afterPractice.elo,beforeReload.elo,'a replayed contract must not change Astro-Elo');
  assert.equal(afterPractice.flights,beforeReload.flights+1);
  assert.deepEqual(afterPractice.ranked,beforeReload.ranked,'the certified certificate must stay locked');
  // oracle_tamper_proof_fdr_verification and
  // oracle_odd_report_deterministic_dossier, in the browser.
  const practice=await page.evaluate(()=>missionDiagnostics.recorder());
  assert.ok(practice,'the practice flight must also produce a signed recorder');
  assert.equal(practice.document.attempt_mode,'Practice');
  assert.equal(practice.document.elo_before,practice.document.elo_after,'a practice record must show no rating change');
  await page.locator('#verify-flight').click();
  await page.waitForFunction(()=>missionDiagnostics.verification()!==null,null,{timeout:180000});
  const verified=await page.evaluate(()=>missionDiagnostics.verification());
  assert.equal(verified.code,1,`verification failed: ${verified.verdict}`);
  assert.equal(verified.verdict,'\u2713 CRYPTOGRAPHICALLY & PHYSICALLY VERIFIED');
  assert.equal(await page.locator('#verify-verdict').textContent(),'\u2713 CRYPTOGRAPHICALLY & PHYSICALLY VERIFIED');
  assert.ok(verified.dossier.includes('id="section-4"'));
  assert.ok(verified.dossier.includes('id="coalition-table"'));
  // Annex B prints the physical design and a vector cross-section, so a
  // reader can recompute the engine without trusting the claimed thrust.
  assert.ok(verified.dossier.includes('id="annex-b"'),'the dossier must print the engine annex');
  assert.ok(verified.dossier.includes('id="engine-table"'));
  assert.ok(verified.dossier.includes('id="engine-figure"'));
  assert.ok(verified.dossier.includes(practice.document.engine_propellant));
  const dossierDownload=page.waitForEvent('download');
  await page.locator('#verify-download').click();
  const dossierText=await readFile(await (await dossierDownload).path(),'utf8');
  assert.equal(dossierText,verified.dossier,'the downloaded dossier must be the verified one');
  // The PDF path prints the verified document itself at its own page geometry.
  await page.locator('#verify-print').click();
  await page.waitForFunction(()=>document.getElementById('dossier-print-frame')?.srcdoc?.includes('id="section-4"'),null,{timeout:30000});
  assert.equal(await page.evaluate(()=>document.getElementById('dossier-print-frame').srcdoc),verified.dossier);

  // The printed QR must decode to a payload that itself verifies.
  const qrPayload=decodeQrFromSvg(dossierText);
  assert.ok(qrPayload.startsWith('#verify='));
  assert.equal(qrPayload,practice.payload,'the QR must carry this flight’s payload');
  await page.locator('#verify-input').fill(qrPayload);
  await page.locator('#verify-run').click();
  await page.waitForFunction(()=>missionDiagnostics.verification()?.code===1&&missionDiagnostics.verification()?.dossier,null,{timeout:180000});
  assert.equal(await page.evaluate(()=>missionDiagnostics.verification().code),1,'the decoded QR payload must verify');

  // oracle_fdr_engine_recomputation_anti_cheat: claiming a better engine than
  // the signed geometry produces must be refused. Editing the throat radius
  // breaks the signature; forging a consistent record is covered natively by
  // odd-verify's engine_anti_cheat suite.
  const engineTampered=structuredClone(practice.document);
  engineTampered.engine_r_throat_m*=1.01;
  await page.locator('#verify-input').fill(JSON.stringify(engineTampered));
  await page.locator('#verify-run').click();
  await page.waitForFunction(()=>missionDiagnostics.verification()?.code===-1,null,{timeout:60000});
  assert.match(await page.locator('#verify-verdict').textContent(),/SignatureMismatch/);

  // A single edited bit must be rejected with SignatureMismatch.
  const tampered=structuredClone(practice.document);
  tampered.metrics[1]=nextUlp(tampered.metrics[1]);
  assert.notEqual(tampered.metrics[1],practice.document.metrics[1]);
  await page.locator('#verify-input').fill(JSON.stringify(tampered));
  await page.locator('#verify-run').click();
  await page.waitForFunction(()=>missionDiagnostics.verification()?.code===-1,null,{timeout:60000});
  assert.match(await page.locator('#verify-verdict').textContent(),/SignatureMismatch/);
  // One microsecond of command timing is equally fatal.
  const shifted=structuredClone(practice.document);
  shifted.command_timeline[0].time_us+=1;
  await page.locator('#verify-input').fill(JSON.stringify(shifted));
  await page.locator('#verify-run').click();
  await page.waitForFunction(()=>missionDiagnostics.verification()?.code===-1,null,{timeout:60000});
  assert.match(await page.locator('#verify-verdict').textContent(),/SignatureMismatch/);

  // A forger with a license of their own can re-sign a false metric. The
  // signature then passes and the physics replay must catch it.
  const forged=await page.evaluate(async ulp=>{
    const recorder=await import('./recorder.js');
    const current=missionDiagnostics.recorder();
    const {fields,callsign}=recorder.fieldsFromJson(current.document);
    // Locate the first metric in the flat transport layout.
    const metricsAt=recorder.HEADER+current.document.command_timeline.length*recorder.COMMAND_STRIDE;
    fields[metricsAt+1]=ulp;
    const {fields:forgedFields,callsign:forgedCallsign}=recorder.fieldsFromJson(recorder.sign(missionDiagnostics.wasm(),fields,callsign).document);
    return {fields:forgedFields,callsign:forgedCallsign};
  },nextUlp(practice.document.metrics[1]));
  await page.evaluate(async inputs=>{
    const verify=await import('./verify.js');
    verify.verifyFields(inputs.fields,inputs.callsign);
  },forged);
  await page.waitForFunction(()=>missionDiagnostics.verification()?.code===-2,null,{timeout:180000});
  assert.match(await page.locator('#verify-verdict').textContent(),/PhysicsReplayMismatch/);
  assert.equal(await page.evaluate(()=>missionDiagnostics.verification().mismatchIndex),1);
  await page.locator('#verify-close').click();
  await page.waitForFunction(()=>!missionDiagnostics.pending,null,{timeout:10000});
  const verifyIdle=await page.evaluate(()=>({frames:testFrameCallbacks,messages:testWorkerMessages,draws:missionDiagnostics.draws}));
  await page.waitForTimeout(2000);
  assert.deepEqual(await page.evaluate(()=>({frames:testFrameCallbacks,messages:testWorkerMessages,draws:missionDiagnostics.draws})),verifyIdle,'verification must return to zero frames and zero worker messages');

  // A shared #verify= link must verify on load with no account and no server.
  await page.goto(`http://127.0.0.1:${server.address().port}/mission.html${practice.payload}`);
  await page.waitForFunction(()=>missionDiagnostics.verification()?.code===1,null,{timeout:180000});
  assert.equal(await page.locator('#verify-dialog').isVisible(),true);
  assert.equal(await page.locator('#verify-verdict').textContent(),'\u2713 CRYPTOGRAPHICALLY & PHYSICALLY VERIFIED');
  // A damaged storage record must degrade to defaults, not brick the page.
  // These are the shapes a truncated write or a hand-edited record produces.
  const damaged=[
    ['null rating',{elo:null}],
    ['null flights',{flights:null}],
    ['null certificate',{ranked:{101:null}}],
    ['object callsign',{callsign:{unexpected:true}}],
  ];
  for(const [label,patch] of damaged){
    const probe=await browser.newPage({viewport:{width:1280,height:900}});
    const probeErrors=[];probe.on('pageerror',e=>probeErrors.push(String(e).split('\n')[0]));
    await probe.addInitScript(value=>{localStorage.setItem('odderon.flightLicense.v1',JSON.stringify(value));},{schema:1,entropyHex:'00112233445566778899aabbccddeeff',callsign:'KEEP ME',elo:1234.5,...patch});
    await probe.goto(`http://127.0.0.1:${server.address().port}/mission.html`);
    await probe.waitForFunction(()=>window.missionDiagnostics?.profile?.()!==null,null,{timeout:30000});
    const repaired=await probe.evaluate(()=>missionDiagnostics.profile());
    // Repair must never discard a usable secret: the identity is the account.
    assert.equal(repaired.flightKey,'ODD-0011-2233-4455-6677-8899-AABB-CCDD-EEFF',`${label}: identity must survive repair`);
    assert.equal(repaired.elo,label==='null rating'?1200:1234.5,`${label}: rating`);
    await probe.locator('#open-profile').click();
    assert.ok((await probe.locator('#profile-elo').textContent()).length>0,label);
    assert.deepEqual(probeErrors,[],`${label} must not raise a page error`);
    await probe.close();
  }
  // Switching identity must not destroy the identity being left. Erasing its
  // certificates would also let the same contract be certified twice, so this
  // is an integrity check, not only a convenience one.
  {
    const probe=await browser.newPage({viewport:{width:1280,height:900}});
    const probeErrors=[];probe.on('pageerror',e=>probeErrors.push(String(e).split('\n')[0]));
    await probe.addInitScript(()=>{localStorage.setItem('odderon.flightLicense.v1',JSON.stringify({
      schema:1,entropyHex:'00112233445566778899aabbccddeeff',callsign:'PILOT A',elo:1337.5,
      missionElo:{101:812.5},ranked:{101:{score:0.9,elo:1337.5,delta:17.5,at:1}},flights:[{contractId:'101'}]
    }));});
    await probe.goto(`http://127.0.0.1:${server.address().port}/mission.html`);
    await probe.waitForFunction(()=>window.missionDiagnostics?.profile?.(),null,{timeout:30000});
    const first=await probe.evaluate(()=>missionDiagnostics.profile());
    assert.equal(first.elo,1337.5);
    await probe.locator('#open-profile').click();
    // A seven-group key must be refused with a useful message.
    await probe.locator('#profile-restore-input').fill('ODD-FEED-DCBA-0099-8877-6655-4433-2211');
    assert.equal(await probe.locator('#profile-restore').isDisabled(),true);
    assert.match(await probe.locator('#profile-restore-status').textContent(),/eight 4-character hex groups/);
    await probe.locator('#profile-restore-input').fill('ODD-FEED-DCBA-0099-8877-6655-4433-2211-0123');
    assert.equal(await probe.locator('#profile-restore').isEnabled(),true);
    await probe.locator('#profile-restore').click();
    const second=await probe.evaluate(()=>missionDiagnostics.profile());
    assert.notEqual(second.publicId,first.publicId,'a different key must switch identity');
    assert.equal(second.elo,1200,'a new identity starts at the base rating');
    assert.deepEqual(second.ranked,{},'a new identity has no certificates');
    assert.equal(await probe.locator('#topbar-elo').textContent(),'1200');
    // Returning with the original twelve words must resume, not reset.
    await probe.locator('#profile-restore-input').fill(first.mnemonic);
    await probe.locator('#profile-restore').click();
    const resumed=await probe.evaluate(()=>missionDiagnostics.profile());
    assert.equal(resumed.publicId,first.publicId);
    assert.equal(resumed.elo,1337.5,'switching back must not erase the rating');
    assert.equal(resumed.callsign,'PILOT A');
    assert.deepEqual(Object.keys(resumed.ranked),['101'],'the certificate must survive the round trip');
    assert.equal(await probe.evaluate(()=>missionDiagnostics.isRanked('101')),true,'a certified contract must stay certified after a switch');
    assert.deepEqual(probeErrors,[]);
    await probe.close();
  }
  assert.deepEqual(errors,[]);console.log('Browser viewport, interactions, bounds, WebGL, responsive layout, three mission prototypes, license persistence, signed-dossier verification, damaged-storage repair and identity switching passed.');
} finally{if(browser)await browser.close();await new Promise(r=>server.close(r));}
