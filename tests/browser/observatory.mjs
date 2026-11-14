// Real Chromium/WebGL2 integration and demand-rendering gate.
import { chromium } from 'playwright-core';
import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { resolve, extname } from 'node:path';
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
const root=resolve('web');
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
  const missionBox=await page.locator('#plan-canvas').boundingBox();
  await page.mouse.click(missionBox.x+missionBox.width/2+(6378137+260000)*missionBox.height/(2.45*(6378137+550000)),missionBox.y+missionBox.height/2);
  assert.equal(await page.evaluate(()=>missionDiagnostics.nodes.length),1,'clicking the path should place a maneuver');
  await page.locator('#remove-node').click();
  await page.locator('#add-node').click();
  await page.locator('#node-t').fill('82.38547669');
  await page.locator('#add-node').click();
  await page.locator('#node-time').fill('2979.885241');
  await page.locator('#node-t').fill('81.50942505');
  assert.equal(await page.evaluate(()=>missionDiagnostics.nodes.length),2);
  await page.locator('#node-w').focus();
  assert.match(await page.locator('#lens-text').textContent(),/perpendicular to the orbital plane/);
  const handle=await page.evaluate(()=>missionDiagnostics.handles.find(h=>h.key==='t').end);
  const canvasBox=await page.locator('#plan-canvas').boundingBox();
  await page.mouse.move(canvasBox.x+handle[0],canvasBox.y+handle[1]);await page.mouse.down();await page.mouse.move(canvasBox.x+handle[0]+5,canvasBox.y+handle[1]+5,{steps:3});await page.mouse.up();
  assert.notEqual(await page.evaluate(()=>missionDiagnostics.nodes[1].dv[0]),81.50942505,'dragging a handle should change the maneuver');
  await page.locator('#node-t').fill('81.50942505');
  await page.locator('#prefill').click();
  await page.locator('#execute').click();
  await page.waitForFunction(()=>!document.getElementById('debrief').hidden,null,{timeout:120000});
  const debrief=await page.locator('#debrief-content').textContent();
  const score=Number(debrief.match(/([0-9.]+)%Mission score/)?.[1]);
  assert.ok(score>80,`rescue score: ${score}; ${debrief}`);
  assert.equal(await page.evaluate(()=>missionDiagnostics.locked),true);
  await page.waitForFunction(()=>!missionDiagnostics.pending,null,{timeout:10000});
  const missionIdle=await page.evaluate(()=>({frames:testFrameCallbacks,messages:testWorkerMessages,draws:missionDiagnostics.draws}));
  await page.waitForTimeout(2000);
  assert.deepEqual(await page.evaluate(()=>({frames:testFrameCallbacks,messages:testWorkerMessages,draws:missionDiagnostics.draws})),missionIdle,'mission must settle without frames, redraws, or worker messages');
  await page.locator('#unlock').click();
  assert.equal(await page.evaluate(()=>missionDiagnostics.locked),false);
  assert.deepEqual(errors,[]);console.log('Browser viewport, interactions, bounds, WebGL, responsive layout and mission prototype passed.');
} finally{if(browser)await browser.close();await new Promise(r=>server.close(r));}
