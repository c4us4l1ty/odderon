import { Governor } from './governor.js';
import { Renderer } from './renderer.js';

const $=id=>document.getElementById(id);
const scene=$('scene'), canvas=$('world'), overlay=$('overlay'), date=$('date'), error=$('error');
const J2000_UNIX_MS=Date.UTC(2000,0,1,12), DAY=86400;
let renderer, wasm, snapshot, contextLost=false, camera={yaw:-1.30,pitch:-.35,zoom:1}, pointer=null, draws=0;
const showError=message=>{error.hidden=false;error.textContent=message;};
const governor=new Governor(quality=>{try{draw(quality);}catch(e){governor.suspend();showError(`The view could not be rendered: ${e.message}`);}});
const dot=(a,b)=>a.reduce((s,x,i)=>s+x*b[i],0);
const norm=a=>{const m=Math.hypot(...a);return a.map(v=>v/m);};
const subtract=(a,b)=>a.map((v,i)=>v-b[i]);
const columns=matrix=>[matrix[0],matrix[3],matrix[6],matrix[1],matrix[4],matrix[7],matrix[2],matrix[5],matrix[8]];
function loadDate(){
  // Calendar arithmetic only: selected midnight is explicitly TDB, not UTC.
  const time=Date.parse(`${date.value}T00:00:00Z`), seconds=(time-J2000_UNIX_MS)/1000;
  if(!Number.isFinite(time)||!wasm.view_load(seconds)){snapshot=null;$('distance').textContent='—';$('speed').textContent='—';showError('Date outside the available ephemeris (2020–2035 TDB).');return false;}
  const value=i=>wasm.view_value(i), count=value(27), samples=[];
  for(let i=0;i<count;i++)samples.push({time:value(28+4*i),pos:[value(29+4*i),value(30+4*i),value(31+4*i)]});
  snapshot={seconds,moon:[value(0),value(1),value(2)],velocity:[value(3),value(4),value(5)],sun:[value(6),value(7),value(8)],earthOrientation:columns(Array.from({length:9},(_,i)=>value(9+i))),moonOrientation:columns(Array.from({length:9},(_,i)=>value(18+i))),samples};
  $('distance').textContent=Math.round(Math.hypot(...snapshot.moon)/1000).toLocaleString('en-US');
  $('speed').textContent=(Math.hypot(...snapshot.velocity)/1000).toFixed(3);
  $('prev').disabled=date.value===date.min;$('next').disabled=date.value===date.max;
  if(!contextLost)error.hidden=true;governor.invalidate();return true;
}
function geometry(){
  const {yaw,pitch,zoom}=camera, c=Math.cos(pitch),s=Math.sin(pitch), cy=Math.cos(yaw),sy=Math.sin(yaw);
  const right=[-sy,cy,0],up=[-s*cy,-s*sy,c],view=[c*cy,c*sy,s];
  const half=snapshot.moon.map(x=>x/2), span=520e6/zoom;
  const project=v=>[2*dot(subtract(v,half),right)/span,2*dot(subtract(v,half),up)/span,dot(subtract(v,half),view)/span];
  const e=project([0,0,0]),m=project(snapshot.moon);
  return {right,up,view,project,earth:[e[0],e[1],2*6378136.6*7.5/span,e[2]],moon:[m[0],m[1],2*1737400*7.5/span,m[2]],earthLight:norm(snapshot.sun),moonLight:norm(subtract(snapshot.sun,snapshot.moon)),earthOrientation:snapshot.earthOrientation,moonOrientation:snapshot.moonOrientation,vehicle:$('show-vehicle').checked};
}
function draw(quality){
  if(!snapshot||!renderer)return;
  const rect=scene.getBoundingClientRect();if(rect.width<1||rect.height<1)return;
  const model=geometry();renderer.draw(model,quality);drawOverlay(model,rect);draws++;
  $('render-status').textContent=quality==='interactive'?'INTERACTIVE QUALITY':'RENDERED · RESTING';
}
function drawOverlay(model,rect){
  const dpr=Math.min(window.devicePixelRatio||1,2), w=Math.round(rect.width*dpr),h=Math.round(rect.height*dpr);
  if(overlay.width!==w||overlay.height!==h){overlay.width=w;overlay.height=h;}
  const ctx=overlay.getContext('2d');ctx.setTransform(dpr,0,0,dpr,0,0);ctx.clearRect(0,0,rect.width,rect.height);
  const pixel=p=>[rect.width/2+p[0]*rect.height/2,rect.height/2-p[1]*rect.height/2];
  if($('show-path').checked){
    for(const future of [false,true]){
      ctx.beginPath();let started=false;for(const sample of snapshot.samples){if(future ? sample.time<snapshot.seconds : sample.time>snapshot.seconds)continue;const [x,y]=pixel(model.project(sample.pos));if(!started){ctx.moveTo(x,y);started=true;}else ctx.lineTo(x,y);}
      ctx.strokeStyle=future?'rgba(194,211,189,.25)':'rgba(194,211,189,.55)';ctx.lineWidth=1;ctx.setLineDash(future?[3,5]:[]);ctx.stroke();
    } ctx.setLineDash([]);
  }
  if($('show-velocity').checked){
    const from=pixel(model.project(snapshot.moon));
    // A 1-day tangent scaled from instantaneous geometric velocity, not an orbit prediction.
    const to=pixel(model.project(snapshot.moon.map((x,i)=>x+snapshot.velocity[i]*86400)));
    const angle=Math.atan2(to[1]-from[1],to[0]-from[0]);
    ctx.strokeStyle='#e1bd78';ctx.fillStyle='#e1bd78';ctx.lineWidth=1.5;ctx.beginPath();ctx.moveTo(...from);ctx.lineTo(...to);ctx.stroke();
    ctx.beginPath();ctx.moveTo(...to);ctx.lineTo(to[0]-8*Math.cos(angle-.45),to[1]-8*Math.sin(angle-.45));ctx.lineTo(to[0]-8*Math.cos(angle+.45),to[1]-8*Math.sin(angle+.45));ctx.closePath();ctx.fill();
    ctx.font='10px system-ui';ctx.fillText('VELOCITY · 1 DAY AT CURRENT SPEED',20,rect.height-(model.vehicle?62:45));
  }
  if(model.vehicle){ctx.fillStyle='#d5bb80';ctx.font='10px system-ui';ctx.fillText('VEHICLE STUDY · NOT A SIMULATED MISSION',20,rect.height-45);}
  function label(body,name,detail){
    const [x,y]=pixel(body),radius=body[2]*rect.height/2;
    if(x < -radius||x>rect.width+radius||y < -radius||y>rect.height+radius)return;
    const side=x+radius+140>rect.width?-1:1, tx=x+side*(radius+16),ty=Math.max(42,Math.min(rect.height-40,y-radius*.22));
    ctx.textAlign=side===1?'left':'right';ctx.strokeStyle='rgba(205,223,203,.55)';ctx.lineWidth=1;ctx.beginPath();ctx.moveTo(x+side*radius*.76,y-radius*.3);ctx.lineTo(tx-side*5,ty);ctx.stroke();
    ctx.font='700 10px system-ui';ctx.letterSpacing='1.2px';ctx.fillStyle='#dce8d5';ctx.fillText(name,tx,ty-3);ctx.font='10px system-ui';ctx.letterSpacing='0px';ctx.fillStyle='#91a89a';ctx.fillText(detail,tx,ty+11);ctx.textAlign='left';
  }
  label(model.earth,'EARTH','Our home');label(model.moon,'MOON','Natural satellite');
}
function zoom(factor){camera.zoom=Math.max(.55,Math.min(4,camera.zoom*factor));governor.invalidate();}
function changeDay(amount){if(!date.value)return;const d=new Date(Date.parse(`${date.value}T00:00:00Z`)+amount*DAY*1000);const next=d.toISOString().slice(0,10);if(next>=date.min&&next<=date.max){date.value=next;loadDate();}}
scene.addEventListener('pointerdown',e=>{if(e.button!==0||pointer)return;pointer={id:e.pointerId,x:e.clientX,y:e.clientY};scene.setPointerCapture(e.pointerId);governor.begin();});
scene.addEventListener('pointermove',e=>{if(!pointer||e.pointerId!==pointer.id)return;camera.yaw+=(e.clientX-pointer.x)*.006;camera.pitch=Math.max(-1.52,Math.min(1.52,camera.pitch+(e.clientY-pointer.y)*.006));pointer.x=e.clientX;pointer.y=e.clientY;governor.move();});
function endPointer(e){if(pointer&&e.pointerId===pointer.id){pointer=null;governor.end();}}
scene.addEventListener('pointerup',endPointer);scene.addEventListener('pointercancel',endPointer);scene.addEventListener('lostpointercapture',endPointer);
scene.addEventListener('wheel',e=>{e.preventDefault();zoom(Math.exp(-Math.max(-100,Math.min(100,e.deltaY))*.002));},{passive:false});
scene.addEventListener('keydown',e=>{let used=true;switch(e.key){case 'ArrowLeft':camera.yaw-=.12;break;case 'ArrowRight':camera.yaw+=.12;break;case 'ArrowUp':camera.pitch=Math.max(-1.52,camera.pitch-.12);break;case 'ArrowDown':camera.pitch=Math.min(1.52,camera.pitch+.12);break;case '+':case '=':zoom(1.2);break;case '-':zoom(1/1.2);break;default:used=false;}if(used){e.preventDefault();governor.invalidate();}});
$('zoom-in').addEventListener('click',()=>zoom(1.2));$('zoom-out').addEventListener('click',()=>zoom(1/1.2));$('reset').addEventListener('click',()=>{camera={yaw:-1.30,pitch:-.35,zoom:1};governor.invalidate();});
for(const id of ['show-path','show-velocity','show-vehicle'])$(id).addEventListener('change',()=>governor.invalidate());date.addEventListener('change',loadDate);$('prev').addEventListener('click',()=>changeDay(-1));$('next').addEventListener('click',()=>changeDay(1));$('today-reset').addEventListener('click',()=>{date.value='2026-01-01';loadDate();});
new ResizeObserver(()=>governor.invalidate()).observe(scene);
function syncVisibility(){if(document.hidden||contextLost){pointer=null;governor.suspend();}else governor.resume();}
document.addEventListener('visibilitychange',syncVisibility);
canvas.addEventListener('webglcontextlost',event=>{event.preventDefault();contextLost=true;syncVisibility();showError('Graphics context lost. Waiting for the browser to restore it.');});
canvas.addEventListener('webglcontextrestored',()=>{try{renderer.init();contextLost=false;if(wasm&&!snapshot)showError('Date outside the available ephemeris (2020–2035 TDB).');else error.hidden=true;syncVisibility();}catch(e){showError(`Could not restore graphics: ${e.message}`);}});
if(document.hidden)governor.suspend();
// Read-only diagnostics make the idle gate observable without creating a poller.
window.odderonDiagnostics=Object.freeze({get draws(){return draws;},get pendingFrame(){return governor.frame!==null;},get pendingTimer(){return governor.timer!==null;},get quality(){return $('render-status').textContent;},get circles(){if(!snapshot)return null;const g=geometry();return {earth:[...g.earth],moon:[...g.moon]};}});
try {
  renderer=new Renderer(canvas);
  const response=await fetch('./odd_view.wasm');if(!response.ok)throw Error(`Ephemeris module not found (${response.status}). Build the browser app first.`);
  ({instance:{exports:wasm}}=await WebAssembly.instantiate(await response.arrayBuffer(),{}));
  loadDate();
} catch(e){showError(`Unable to start the observatory: ${e.message}`);}
