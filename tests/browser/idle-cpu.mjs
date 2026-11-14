// Linux-only end-to-end WP 0.3 CPU gate. One busy calibration, then a camera
// drag/release, a settled-frame warmup, and a quiet browser-process-tree sample.
import { chromium } from 'playwright-core';
import { createServer } from 'node:http';
import { readFile as readFileAsync } from 'node:fs/promises';
import { readFileSync, readdirSync } from 'node:fs';
import { resolve, extname } from 'node:path';
import { execFileSync } from 'node:child_process';
import assert from 'node:assert/strict';

if(process.platform!=='linux')throw Error('The idle CPU gate needs Linux /proc scheduler accounting.');
const root=resolve('web');
const server=createServer(async(req,res)=>{const path=resolve(root,'.'+new URL(req.url,'http://test').pathname.replace(/\/$/,'/index.html'));if(!path.startsWith(root+'/')){res.writeHead(403).end();return;}try{const b=await readFileAsync(path);res.setHeader('content-type',({'.html':'text/html','.js':'text/javascript','.css':'text/css','.wasm':'application/wasm'})[extname(path)]||'application/octet-stream');res.end(b);}catch{res.writeHead(404).end();}});
await new Promise(r=>server.listen(0,'127.0.0.1',r));

// /proc/<pid>/stat comm may contain spaces and parentheses; parse after its
// closing parenthesis. Fields 4 and 22 are parent PID and process start time.
function stat(path){const s=readFileSync(path,'utf8');const parts=s.slice(s.lastIndexOf(')')+2).split(' ');return {parent:Number(parts[1]),start:parts[19]};}
function processTree(){
  const all=[];
  for(const id of readdirSync('/proc')){if(!/^\d+$/.test(id))continue;try{all.push({id:Number(id),...stat(`/proc/${id}/stat`)});}catch{ /* exited or inaccessible process */ }}
  const descendants=new Set([process.pid]);let changed=true;
  while(changed){changed=false;for(const p of all)if(descendants.has(p.parent)&&!descendants.has(p.id)){descendants.add(p.id);changed=true;}}
  return all.filter(p=>p.id!==process.pid&&descendants.has(p.id));
}
function accounting(){
  const processes=processTree();if(processes.length<2)throw Error('Cannot find Chromium process tree.');
  const threads=new Map();
  for(const p of processes){
    for(const tid of readdirSync(`/proc/${p.id}/task`)){
      const base=`/proc/${p.id}/task/${tid}`;
      // Do not ignore races or missing accounting: an incomplete sample cannot pass.
      const start=stat(`${base}/stat`).start;
      const nanoseconds=BigInt(readFileSync(`${base}/schedstat`,'utf8').split(' ')[0]);
      threads.set(`${p.id}:${p.start}:${tid}:${start}`,nanoseconds);
    }
  }
  return {processes:processes.map(p=>`${p.id}:${p.start}`).sort(),threads};
}
function delta(before,after){
  assert.deepEqual(after.processes,before.processes,'Inconclusive: browser process tree changed during measurement');
  assert.deepEqual([...after.threads.keys()].sort(),[...before.threads.keys()].sort(),'Inconclusive: browser thread set changed during measurement');
  let elapsed=0n;for(const [key,value] of after.threads){const d=value-before.threads.get(key);assert.ok(d>=0n);elapsed+=d;}return Number(elapsed)/1e9;
}
let browser;
try{
  browser=await chromium.launch({executablePath:process.env.CHROME_BIN||execFileSync('which',['google-chrome'],{encoding:'utf8'}).trim(),headless:true,args:['--no-sandbox','--enable-webgl','--use-angle=swiftshader','--enable-unsafe-swiftshader','--disable-dev-shm-usage','--no-proxy-server']});
  const page=await browser.newPage({viewport:{width:1280,height:900},deviceScaleFactor:1});
  await page.goto(`http://127.0.0.1:${server.address().port}/`);
  await page.waitForFunction(()=>window.odderonDiagnostics?.draws>0,null,{timeout:30000});
  await page.waitForTimeout(1000);
  // Sanity check accounting can see CPU burned inside Chromium, not just zeros.
  const busyBefore=accounting();
  await page.evaluate(()=>{const stop=performance.now()+150;while(performance.now()<stop){};});
  const busyAfter=accounting();
  // The busy loop can cause transient Chrome threads; check overlapping thread
  // deltas for calibration. The strict identity requirement is for idle below.
  let busy=0n;for(const [id,v] of busyAfter.threads)if(busyBefore.threads.has(id))busy+=v-busyBefore.threads.get(id);
  assert.ok(busy>30_000_000n,`Scheduler accounting did not detect the calibration workload: ${busy} ns`);
  const initialDraws=await page.evaluate(()=>odderonDiagnostics.draws);
  const box=await page.locator('#scene').boundingBox();
  await page.mouse.move(box.x+box.width/2,box.y+box.height/2);await page.mouse.down();await page.mouse.move(box.x+box.width*.62,box.y+box.height*.56,{steps:5});await page.mouse.up();
  await page.waitForFunction(()=>odderonDiagnostics.quality.includes('RESTING')&&!odderonDiagnostics.pendingFrame&&!odderonDiagnostics.pendingTimer,null,{timeout:30000});
  // Drain the final GPU pass before the idle window, then let browser settle.
  await page.locator('#world').evaluate(c=>c.getContext('webgl2').finish());await page.waitForTimeout(1000);
  const draws=await page.evaluate(()=>odderonDiagnostics.draws);assert.ok(draws>initialDraws,'camera drag did not render');
  const start=accounting(), startTime=process.hrtime.bigint();
  await new Promise(r=>setTimeout(r,10000));
  const endTime=process.hrtime.bigint(), end=accounting();
  const cpuSeconds=delta(start,end),wallSeconds=Number(endTime-startTime)/1e9,percent=100*cpuSeconds/wallSeconds;
  assert.equal(await page.evaluate(()=>odderonDiagnostics.draws),draws,'frames rendered during idle window');
  assert.equal(await page.evaluate(()=>odderonDiagnostics.pendingFrame||odderonDiagnostics.pendingTimer),false,'scheduled work during idle window');
  console.log(`Idle after camera drag: 0 app frames, ${cpuSeconds.toFixed(6)} s CPU / ${wallSeconds.toFixed(3)} s wall = ${percent.toFixed(4)}% of one core; ${end.processes.length} processes, ${end.threads.size} threads. Calibration ${(Number(busy)/1e6).toFixed(1)} ms.`);
  if(percent>=0.2){
    const top=[...end.threads].map(([id,v])=>({id,ns:Number(v-start.threads.get(id))})).sort((a,b)=>b.ns-a.ns).slice(0,10);
    for(const t of top){const [pid,,tid]=t.id.split(':');let name='unknown',cmd='unknown';try{name=readFileSync(`/proc/${pid}/task/${tid}/comm`,'utf8').trim();cmd=readFileSync(`/proc/${pid}/cmdline`,'utf8').replaceAll('\0',' ').slice(0,160);}catch{}console.log(`  ${(t.ns/1e6).toFixed(3)} ms ${name} (${cmd})`);}
  }
  assert.ok(percent<0.2,`Browser process-tree CPU ${percent.toFixed(4)}% exceeds the <0.2% one-core gate`);
  await page.mouse.move(box.x+box.width*.5,box.y+box.height*.5);await page.mouse.down();await page.mouse.move(box.x+box.width*.55,box.y+box.height*.52);
  await page.waitForFunction(n=>odderonDiagnostics.draws>n,draws,{timeout:30000});await page.mouse.up();
  console.log('Renderer woke successfully on renewed camera interaction.');
}finally{if(browser)await browser.close();await new Promise(r=>server.close(r));}
