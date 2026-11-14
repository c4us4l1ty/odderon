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
  const errors=[];page.on('pageerror',e=>errors.push(String(e)));page.on('console',m=>{if(m.type()==='error')errors.push(m.text());});
  await page.goto(`http://127.0.0.1:${server.address().port}/`);
  await page.waitForFunction(()=>window.odderonDiagnostics?.draws>0,{timeout:30000});
  await page.waitForTimeout(500);
  assert.equal(await page.locator('#error').isHidden(),true);
  assert.equal(await page.locator('#distance').textContent(),'361,026');
  // Telemetry-only controls must not re-run the expensive world layer.
  const layerBefore=await page.evaluate(()=>({world:odderonDiagnostics.draws,overlay:odderonDiagnostics.overlayDraws}));
  await page.locator('#show-path').uncheck();await page.waitForFunction(n=>odderonDiagnostics.overlayDraws>n,layerBefore.overlay);
  assert.equal(await page.evaluate(()=>odderonDiagnostics.draws),layerBefore.world,'path toggle repainted the world');
  await page.locator('#show-path').check();await page.waitForFunction(n=>odderonDiagnostics.overlayDraws>n,layerBefore.overlay+1);
  assert.equal(await page.evaluate(()=>odderonDiagnostics.draws),layerBefore.world,'path toggle repainted the world');
  // Exercise the real filter against a synthetic two-region/noise fixture.
  // A constant image must stay constant; noise should decrease without
  // washing out a sharp edge. This is an implementation check, not an art score.
  const filter=await page.evaluate(async()=>{
    const {vertex,paintFragment}=await import('./shaders.js');const canvas=document.createElement('canvas');canvas.width=32;canvas.height=32;
    const gl=canvas.getContext('webgl2',{preserveDrawingBuffer:true});
    const shader=(kind,src)=>{const s=gl.createShader(kind);gl.shaderSource(s,src);gl.compileShader(s);if(!gl.getShaderParameter(s,gl.COMPILE_STATUS))throw Error(gl.getShaderInfoLog(s));return s;};
    const p=gl.createProgram();gl.attachShader(p,shader(gl.VERTEX_SHADER,vertex));gl.attachShader(p,shader(gl.FRAGMENT_SHADER,paintFragment));gl.linkProgram(p);if(!gl.getProgramParameter(p,gl.LINK_STATUS))throw Error(gl.getProgramInfoLog(p));gl.useProgram(p);
    const rgba=(generator)=>{const bytes=new Uint8Array(32*32*4);for(let y=0;y<32;y++)for(let x=0;x<32;x++){const i=(y*32+x)*4;const v=generator(x,y);bytes.set([v,v,v,255],i);}return bytes;};
    const texture=(index,bytes)=>{const t=gl.createTexture();gl.activeTexture(gl.TEXTURE0+index);gl.bindTexture(gl.TEXTURE_2D,t);gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_MIN_FILTER,gl.NEAREST);gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_MAG_FILTER,gl.NEAREST);gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_WRAP_S,gl.CLAMP_TO_EDGE);gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_WRAP_T,gl.CLAMP_TO_EDGE);gl.texImage2D(gl.TEXTURE_2D,0,gl.RGBA8,32,32,0,gl.RGBA,gl.UNSIGNED_BYTE,bytes);};
    const input=rgba((x,y)=>(x<16?48:205)+(((x*13+y*17)%11)-5)*3);const normal=new Uint8Array(32*32*4);for(let i=0;i<normal.length;i+=4)normal.set([128,128,179,255],i);
    texture(0,input);texture(1,normal);gl.uniform1i(gl.getUniformLocation(p,'uColor'),0);gl.uniform1i(gl.getUniformLocation(p,'uNormal'),1);gl.uniform2f(gl.getUniformLocation(p,'uResolution'),32,32);gl.uniform1f(gl.getUniformLocation(p,'uQuality'),1);gl.viewport(0,0,32,32);gl.drawArrays(gl.TRIANGLES,0,3);let output=new Uint8Array(input.length);gl.readPixels(0,0,32,32,gl.RGBA,gl.UNSIGNED_BYTE,output);
    const variance=(bytes)=>{let s=0,q=0,n=0;for(let y=4;y<28;y++)for(let x=3;x<12;x++){let v=bytes[(y*32+x)*4];s+=v;q+=v*v;n++;}return q/n-(s/n)**2;};
    const sample=(x)=>{let sum=0;for(let y=4;y<28;y++)sum+=output[(y*32+x)*4];return sum/24;};
    const result={inputVariance:variance(input),outputVariance:variance(output),left:sample(15),right:sample(16)};
    texture(0,rgba(()=>96));gl.drawArrays(gl.TRIANGLES,0,3);gl.readPixels(0,0,32,32,gl.RGBA,gl.UNSIGNED_BYTE,output);result.constant=output[(16*32+16)*4];result.error=gl.getError();return result;
  });
  assert.equal(filter.error,0);assert.ok(Math.abs(filter.constant-96)<=2,`constant field ${JSON.stringify(filter)}`);
  assert.ok(filter.outputVariance<filter.inputVariance*.8,`noise not reduced ${JSON.stringify(filter)}`);
  assert.ok(filter.left<85 && filter.right>170,`edge blurred ${JSON.stringify(filter)}`);
  // Check actual GPU pixels at the analytically projected centers, not just
  // a draw counter: shader compilation or a blank framebuffer must fail.
  const pixels=await page.locator('#world').evaluate(canvas=>{
    const gl=canvas.getContext('webgl2'), circles=window.odderonDiagnostics.circles;
    const read=(x,y)=>{const out=new Uint8Array(4);gl.readPixels(x,y,1,1,gl.RGBA,gl.UNSIGNED_BYTE,out);return [...out];};
    const sample=c=>read(Math.floor((canvas.width+c[0]*canvas.height)/2),Math.floor((canvas.height+c[1]*canvas.height)/2));
    return {earth:sample(circles.earth),moon:sample(circles.moon),background:read(10,10),error:gl.getError()};
  });
  assert.equal(pixels.error,0);for(const body of ['earth','moon'])assert.ok(pixels[body].slice(0,3).some((v,i)=>Math.abs(v-pixels.background[i])>12),`${body} center is indistinguishable from empty sky: ${JSON.stringify(pixels)}`);
  const stats=await page.evaluate(()=>({draws:odderonDiagnostics.draws,pendingFrame:odderonDiagnostics.pendingFrame,pendingTimer:odderonDiagnostics.pendingTimer,quality:odderonDiagnostics.quality}));
  assert.equal(stats.pendingFrame,false);assert.equal(stats.pendingTimer,false);assert.match(stats.quality,/RESTING/);
  const cdp=await page.context().newCDPSession(page);await cdp.send('Performance.enable');
  const metrics=async()=>Object.fromEntries((await cdp.send('Performance.getMetrics')).metrics.map(x=>[x.name,x.value]));
  const before=await metrics();await page.waitForTimeout(5200);const after=await metrics();
  assert.equal(await page.evaluate(()=>odderonDiagnostics.draws),stats.draws,'idle viewport drew frames');
  assert.equal(await page.evaluate(()=>odderonDiagnostics.pendingFrame||odderonDiagnostics.pendingTimer),false);
  console.log(`Idle 5.2s: 0 app frames; Chrome task time ${(1000*(after.TaskDuration-before.TaskDuration)).toFixed(2)} ms (not total system CPU).`);
  const box=await page.locator('#scene').boundingBox();
  await page.mouse.move(box.x+box.width*.5,box.y+box.height*.5);await page.mouse.down();await page.mouse.move(box.x+box.width*.6,box.y+box.height*.55,{steps:4});await page.waitForTimeout(200);
  assert.equal(await page.evaluate(()=>odderonDiagnostics.quality),'INTERACTIVE QUALITY');
  assert.ok(await page.evaluate(()=>odderonDiagnostics.draws)>stats.draws);
  const interactiveWidth=await page.locator('#world').evaluate(c=>c.width);await page.mouse.up();await page.waitForTimeout(300);
  assert.match(await page.evaluate(()=>odderonDiagnostics.quality),/RESTING/);
  assert.ok(await page.locator('#world').evaluate(c=>c.width)>interactiveWidth,'full resolution after drag');
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
  assert.deepEqual(errors,[]);console.log('Browser viewport, interactions, bounds, WebGL and responsive layout passed.');
} finally{if(browser)await browser.close();await new Promise(r=>server.close(r));}
