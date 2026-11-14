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
  const errors=[];page.on('pageerror',e=>errors.push(String(e)));page.on('console',m=>{if(m.type()==='error')errors.push(m.text());});
  await page.goto(`http://127.0.0.1:${server.address().port}/`);
  await page.waitForFunction(()=>window.odderonDiagnostics?.draws>0,{timeout:30000});
  await page.waitForTimeout(500);
  assert.equal(await page.locator('#error').isHidden(),true);
  assert.equal(await page.locator('#distance').textContent(),'361,026');
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
  await page.locator('#today-reset').click();await page.locator('#show-path').uncheck();await page.locator('#show-velocity').check();await page.locator('#show-vehicle').check();await page.locator('#zoom-in').click();await page.locator('#reset').click();
  await page.locator('#scene').focus();await page.keyboard.press('ArrowRight');
  await page.waitForTimeout(300);
  const glResult=await page.locator('#world').evaluate(canvas=>{const gl=canvas.getContext('webgl2');return {error:gl.getError(),w:canvas.width,h:canvas.height};});
  assert.equal(glResult.error,0,'WebGL error');assert.ok(glResult.w>200&&glResult.h>200);
  if(process.env.ODDERON_SCREENSHOT)await page.screenshot({path:process.env.ODDERON_SCREENSHOT,fullPage:true});
  const ext=await page.locator('#world').evaluate(c=>{window.testLoseContext=c.getContext('webgl2').getExtension('WEBGL_lose_context');return !!window.testLoseContext;});
  if(ext){await page.locator('#world').evaluate(c=>window.testLoseContext.loseContext());await page.waitForFunction(()=>!document.getElementById('error').hidden);await page.locator('#world').evaluate(c=>window.testLoseContext.restoreContext());await page.waitForFunction(()=>document.getElementById('error').hidden,{timeout:10000});}
  await page.setViewportSize({width:390,height:844});await page.waitForTimeout(300);assert.equal(await page.locator('#error').isHidden(),true);
  assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),true,'mobile horizontal overflow');
  assert.deepEqual(errors,[]);console.log('Browser viewport, interactions, bounds, WebGL and responsive layout passed.');
} finally{if(browser)await browser.close();await new Promise(r=>server.close(r));}
