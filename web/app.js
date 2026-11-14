import { Governor } from './governor.js';
import { Renderer } from './renderer.js';
import { polyline, vector, reticle, apsis } from './overlay.js';
import { EARTH_RADIUS_M, MOON_RADIUS_M, projectedRadius, needsReticle } from './scale.js';

const $=id=>document.getElementById(id);
const scene=$('scene'), canvas=$('world'), overlay=$('overlay'), date=$('date'), error=$('error');
const J2000_UNIX_MS=Date.UTC(2000,0,1,12), DAY=86400;
let renderer, wasm, snapshot, activeOrbit=null, contextLost=false, camera={yaw:-1.30,pitch:-.35,zoom:1}, pointer=null, draws=0, overlayDraws=0, worldDirty=true, overlayDirty=true, lastQuality=null;
const showError=message=>{error.hidden=false;error.textContent=message;};
const governor=new Governor((quality,batch)=>{try{return draw(quality,batch);}catch(e){governor.suspend();showError(`The view could not be rendered: ${e.message}`);return false;}});
const invalidateWorld=()=>{worldDirty=true;overlayDirty=true;governor.invalidate();};
const invalidateOverlay=()=>{overlayDirty=true;governor.invalidate(false);};
const dot=(a,b)=>a.reduce((s,x,i)=>s+x*b[i],0);
const norm=a=>{const m=Math.hypot(...a);return a.map(v=>v/m);};
const subtract=(a,b)=>a.map((v,i)=>v-b[i]);
const columns=matrix=>[matrix[0],matrix[3],matrix[6],matrix[1],matrix[4],matrix[7],matrix[2],matrix[5],matrix[8]];
function loadDate(){
  // Calendar arithmetic only: selected midnight is explicitly TDB, not UTC.
  const time=Date.parse(`${date.value}T00:00:00Z`), seconds=(time-J2000_UNIX_MS)/1000;
  if(!Number.isFinite(time)||!wasm.view_load(seconds)){snapshot=null;canvas.style.visibility='hidden';overlay.style.visibility='hidden';$('distance').textContent='—';$('speed').textContent='—';$('prev').disabled=true;$('next').disabled=true;$('render-status').textContent='NO VALID DATE';if(!contextLost)showError('Date outside the available ephemeris (2020–2035 TDB).');return false;}
  const value=i=>wasm.view_value(i), count=value(27), samples=[];
  for(let i=0;i<count;i++)samples.push({time:value(28+4*i),pos:[value(29+4*i),value(30+4*i),value(31+4*i)]});
  snapshot={seconds,moon:[value(0),value(1),value(2)],velocity:[value(3),value(4),value(5)],sun:[value(6),value(7),value(8)],earthOrientation:columns(Array.from({length:9},(_,i)=>value(9+i))),moonOrientation:columns(Array.from({length:9},(_,i)=>value(18+i))),samples};
  $('distance').textContent=Math.round(Math.hypot(...snapshot.moon)/1000).toLocaleString('en-US');
  $('speed').textContent=(Math.hypot(...snapshot.velocity)/1000).toFixed(3);
  $('prev').disabled=date.value===date.min;$('next').disabled=date.value===date.max;
  if(!contextLost)error.hidden=true;invalidateWorld();return true;
}
// Accept caller-supplied real inertial state in SI units; this is a conic, not
// numerical propagation. A failed update clears stale orbit geometry.
function setOrbit(state,mu){
  activeOrbit=null;invalidateWorld();
  if(!wasm||!state||!Array.isArray(state.position)||!Array.isArray(state.velocity)||state.position.length!==3||state.velocity.length!==3)return false;
  if(!wasm.orbit_load(mu,...state.position,...state.velocity))return false;
  const count=wasm.orbit_value(0),points=[];
  for(let i=0;i<Math.abs(count);i++)points.push([wasm.orbit_value(1+3*i),wasm.orbit_value(2+3*i),wasm.orbit_value(3+3*i)]);
  const markers=[];for(let i=0;i<4;i++){const index=541+4*i;if(wasm.orbit_value(index))markers.push({name:['PE','AP','AN','DN'][i],pos:[wasm.orbit_value(index+1),wasm.orbit_value(index+2),wasm.orbit_value(index+3)]});}
  activeOrbit={points,markers,closed:count>0,position:[...state.position]};invalidateWorld();return true;
}
window.odderonSetOrbit=setOrbit;
function geometry(){
  const {yaw,pitch,zoom}=camera, c=Math.cos(pitch),s=Math.sin(pitch), cy=Math.cos(yaw),sy=Math.sin(yaw);
  const right=[-sy,cy,0],up=[-s*cy,-s*sy,c],view=[c*cy,c*sy,s];
  const orbitMode=$('show-orbit').checked&&activeOrbit;
  const half=orbitMode?[0,0,0]:snapshot.moon.map(x=>x/2), span=(orbitMode?30e6:520e6)/zoom;
  const project=v=>[2*dot(subtract(v,half),right)/span,2*dot(subtract(v,half),up)/span,dot(subtract(v,half),view)/span];
  const e=project([0,0,0]),m=project(snapshot.moon);
  return {right,up,view,project,earth:[e[0],e[1],projectedRadius(EARTH_RADIUS_M,span),e[2]],moon:[m[0],m[1],projectedRadius(MOON_RADIUS_M,span),m[2]],earthLight:norm(snapshot.sun),moonLight:norm(subtract(snapshot.sun,snapshot.moon)),earthOrientation:snapshot.earthOrientation,moonOrientation:snapshot.moonOrientation,vehicle:$('show-vehicle').checked};
}
function draw(quality,batch){
  if(!snapshot||!renderer)return false;
  const rect=scene.getBoundingClientRect();if(rect.width<1||rect.height<1)return false;
  const model=geometry();
  if(quality==='interactive'||(quality==='final'&&batch===0)){
    if(!renderer.start(model,quality))return false;
    draws++;lastQuality=quality;worldDirty=false;overlayDirty=true;
  } else if(quality==='final'&&batch>0){renderer.batch(batch);draws++;}
  if(overlayDirty){drawOverlay(model,rect);overlayDraws++;overlayDirty=false;}
  if(quality==='interactive')$('render-status').textContent='INTERACTIVE QUALITY';
  else if(quality==='final')$('render-status').textContent=batch===15?'RENDERED · RESTING':`PAINTING · ${batch}/15`;
  canvas.style.visibility='';overlay.style.visibility='';return true;
}
function drawOverlay(model,rect){
  const dpr=Math.min(window.devicePixelRatio||1,2), w=Math.round(rect.width*dpr),h=Math.round(rect.height*dpr);
  if(overlay.width!==w||overlay.height!==h){overlay.width=w;overlay.height=h;}
  const ctx=overlay.getContext('2d');ctx.setTransform(dpr,0,0,dpr,0,0);ctx.clearRect(0,0,rect.width,rect.height);
  const pixel=p=>[rect.width/2+p[0]*rect.height/2,rect.height/2-p[1]*rect.height/2];
  if($('show-orbit').checked&&activeOrbit){
    const points=activeOrbit.points.map(p=>pixel(model.project(p)));if(activeOrbit.closed)points.push(points[0]);
    polyline(ctx,points,{stroke:'#e1bd78',width:1.5});
    for(const marker of activeOrbit.markers)apsis(ctx,pixel(model.project(marker.pos)),marker.name);
    const craft=pixel(model.project(activeOrbit.position));reticle(ctx,craft,{color:'#e1bd78'});ctx.fillStyle='#e1bd78';ctx.font='10px system-ui';ctx.fillText('TEST CRAFT',craft[0]+9,craft[1]+12);
    ctx.fillStyle='#e1bd78';ctx.font='10px system-ui';ctx.fillText(activeOrbit.closed?'IDEAL TWO-BODY TEST ORBIT':'IDEAL TWO-BODY · OPEN SEGMENT',20,rect.height-26);
  }
  if($('show-path').checked){
    for(const future of [false,true]){
      const points=snapshot.samples.filter(sample=>future ? sample.time>=snapshot.seconds : sample.time<=snapshot.seconds).map(sample=>pixel(model.project(sample.pos)));
      polyline(ctx,points,{stroke:future?'rgba(194,211,189,.25)':'rgba(194,211,189,.55)',dashed:future});
    }
  }
  if($('show-velocity').checked){
    const from=pixel(model.project(snapshot.moon));
    // A 1-day tangent scaled from instantaneous geometric velocity, not an orbit prediction.
    const to=pixel(model.project(snapshot.moon.map((x,i)=>x+snapshot.velocity[i]*86400)));
    const visible=vector(ctx,from,to);
    ctx.fillStyle='#e1bd78';
    ctx.font='10px system-ui';ctx.fillText(visible?'VELOCITY · 1 DAY AT CURRENT SPEED':'VELOCITY · ALMOST ALONG VIEW',20,rect.height-(model.vehicle?62:45));
  }
  if(model.vehicle){ctx.fillStyle='#d5bb80';ctx.font='10px system-ui';ctx.fillText('VEHICLE STUDY · NOT A SIMULATED MISSION',20,rect.height-45);}
  function label(body,name,detail){
    const [x,y]=pixel(body),radius=body[2]*rect.height/2;
    if(x < -radius||x>rect.width+radius||y < -radius||y>rect.height+radius)return;
    if(needsReticle(body[2],rect.height))reticle(ctx,[x,y]);
    const side=x+radius+140>rect.width?-1:1, tx=x+side*(Math.max(radius,6)+16),ty=Math.max(42,Math.min(rect.height-40,y-radius*.22));
    ctx.textAlign=side===1?'left':'right';ctx.strokeStyle='rgba(205,223,203,.55)';ctx.lineWidth=1;ctx.beginPath();ctx.moveTo(x+side*radius*.76,y-radius*.3);ctx.lineTo(tx-side*5,ty);ctx.stroke();
    ctx.font='700 10px system-ui';ctx.letterSpacing='1.2px';ctx.fillStyle='#dce8d5';ctx.fillText(name,tx,ty-3);ctx.font='10px system-ui';ctx.letterSpacing='0px';ctx.fillStyle='#91a89a';ctx.fillText(detail,tx,ty+11);ctx.textAlign='left';
  }
  label(model.earth,'EARTH','Our home');label(model.moon,'MOON','Natural satellite');
}
function zoom(factor){const value=Math.max(.55,Math.min(4,camera.zoom*factor));if(value!==camera.zoom){camera.zoom=value;invalidateWorld();}}
function changeDay(amount){if(!date.value)return;const d=new Date(Date.parse(`${date.value}T00:00:00Z`)+amount*DAY*1000);const next=d.toISOString().slice(0,10);if(next>=date.min&&next<=date.max){date.value=next;loadDate();}}
scene.addEventListener('pointerdown',e=>{if(e.button!==0||pointer)return;pointer={id:e.pointerId,x:e.clientX,y:e.clientY};scene.setPointerCapture(e.pointerId);governor.begin();});
scene.addEventListener('pointermove',e=>{if(!pointer||e.pointerId!==pointer.id)return;camera.yaw+=(e.clientX-pointer.x)*.006;camera.pitch=Math.max(-1.52,Math.min(1.52,camera.pitch+(e.clientY-pointer.y)*.006));pointer.x=e.clientX;pointer.y=e.clientY;worldDirty=true;overlayDirty=true;governor.move();});
function endPointer(e){if(pointer&&e.pointerId===pointer.id){pointer=null;governor.end();}}
scene.addEventListener('pointerup',endPointer);scene.addEventListener('pointercancel',endPointer);scene.addEventListener('lostpointercapture',endPointer);
scene.addEventListener('wheel',e=>{e.preventDefault();zoom(Math.exp(-Math.max(-100,Math.min(100,e.deltaY))*.002));},{passive:false});
scene.addEventListener('keydown',e=>{let used=true;switch(e.key){case 'ArrowLeft':camera.yaw-=.12;break;case 'ArrowRight':camera.yaw+=.12;break;case 'ArrowUp':camera.pitch=Math.max(-1.52,camera.pitch-.12);break;case 'ArrowDown':camera.pitch=Math.min(1.52,camera.pitch+.12);break;case '+':case '=':zoom(1.2);break;case '-':zoom(1/1.2);break;default:used=false;}if(used){e.preventDefault();invalidateWorld();}});
$('zoom-in').addEventListener('click',()=>zoom(1.2));$('zoom-out').addEventListener('click',()=>zoom(1/1.2));$('reset').addEventListener('click',()=>{camera={yaw:-1.30,pitch:-.35,zoom:1};invalidateWorld();});
for(const id of ['show-path','show-velocity'])$(id).addEventListener('change',invalidateOverlay);$('show-vehicle').addEventListener('change',invalidateWorld);$('show-orbit').addEventListener('change',invalidateWorld);date.addEventListener('change',loadDate);$('prev').addEventListener('click',()=>changeDay(-1));$('next').addEventListener('click',()=>changeDay(1));$('today-reset').addEventListener('click',()=>{date.value='2026-01-01';loadDate();});
new ResizeObserver(invalidateWorld).observe(scene);
// A monitor or browser zoom change can alter pixel density without resizing the
// CSS box. Re-arm a one-shot media query rather than polling while idle.
function watchPixelDensity(){
  const query=matchMedia(`(resolution: ${window.devicePixelRatio || 1}dppx)`);
  query.addEventListener('change',()=>{watchPixelDensity();invalidateWorld();},{once:true});
}
watchPixelDensity();
function syncVisibility(){if(document.hidden||contextLost){pointer=null;governor.suspend();}else governor.resume();}
document.addEventListener('visibilitychange',syncVisibility);
canvas.addEventListener('webglcontextlost',event=>{event.preventDefault();contextLost=true;syncVisibility();showError('Graphics context lost. Waiting for the browser to restore it.');});
canvas.addEventListener('webglcontextrestored',()=>{try{renderer.init();worldDirty=true;overlayDirty=true;lastQuality=null;contextLost=false;if(wasm&&!snapshot)showError('Date outside the available ephemeris (2020–2035 TDB).');else error.hidden=true;syncVisibility();}catch(e){showError(`Could not restore graphics: ${e.message}`);}});
if(document.hidden)governor.suspend();
// Read-only diagnostics make the idle gate observable without creating a poller.
window.odderonDiagnostics=Object.freeze({get draws(){return draws;},get frameCount(){return governor.frameCount;},get orbit(){return activeOrbit?{samples:activeOrbit.points.length,closed:activeOrbit.closed,markers:activeOrbit.markers.map(m=>m.name)}:null;},get batchIndex(){return renderer?.batchIndex??-1;},get strokeCount(){return renderer?.strokeCount??0;},get overlayDraws(){return overlayDraws;},get pendingFrame(){return governor.frame!==null;},get pendingTimer(){return governor.timer!==null;},get quality(){return $('render-status').textContent;},get circles(){if(!snapshot)return null;const g=geometry();return {earth:[...g.earth],moon:[...g.moon]};}});
try {
  renderer=new Renderer(canvas);
  const response=await fetch('./odd_view.wasm');if(!response.ok)throw Error(`Ephemeris module not found (${response.status}). Build the browser app first.`);
  ({instance:{exports:wasm}}=await WebAssembly.instantiate(await response.arrayBuffer(),{}));
  date.disabled=false;$('today-reset').disabled=false;setOrbit({position:[7e6,0,0],velocity:[0,7600,2000]},3.986004418e14);loadDate();
} catch(e){showError(`Unable to start the observatory: ${e.message}`);}
