// Experimental mission 101. Scientific calculations are performed by the
// Rust/WASM planner and the one-shot flight worker, not fabricated by canvas.
const $=id=>document.getElementById(id);
const MU=3.986004418e14,R=6378137;
let INITIAL=R+260000,TARGET=R+550000,DURATION=10800,DRY=1020,plane=[1,0],scenario='101';
let setup=[820497600,DURATION,1200,DRY,2400,310,INITIAL,0,0,0,Math.sqrt(MU/INITIAL),0];
const canvas=$('plan-canvas'),ctx=canvas.getContext('2d');
let wasm,nodes=[],selected=-1,preview=[],actual=null,lastReport=null,worker=null,timer=null,locked=null,drag=null,draws=0,messages=0,sequence=0;
const lens={
  t:'A push along the direction of travel changes orbital energy. Near the closest point, a forward push raises the farthest point of the orbit; a backward push lowers it.',
  n:'This sideways push stays in the orbital plane and is perpendicular to the direction of travel. Near an apsis it points roughly inward or outward; away from one it is not exactly radial.',
  w:'This push is perpendicular to the orbital plane. It changes the direction of the orbit’s angular momentum and can tilt the plane. The diagram’s diagonal handle is a symbol, not a 3D projection.',
  time:'Move the maneuver along the predicted path. Timing changes where the engine pushes and how the rest of the orbit responds.'
};
const fields={time:$('node-time'),t:$('node-t'),n:$('node-n'),w:$('node-w')};
const distance=(a,b)=>Math.hypot(a[0]-b[0],a[1]-b[1]);
function readState(w,t){return Array.from({length:7},(_,i)=>w.mission_preview(t,i));}
function configure(w,items){
  if(!w.mission_setup(...setup)||!w.mission_metric(scenario==='102'?1:0))return false;
  for(const item of [...items].sort((a,b)=>a.time-b.time)){
    if(Math.hypot(...item.dv)<1e-12)continue;
    if(!w.mission_add_node(item.time,...item.dv))return false;
  }
  return !!w.mission_compile();
}
function orbitMetrics(state){
  const r=state.slice(0,3),v=state.slice(3,6),radius=Math.hypot(...r),speed2=v.reduce((a,b)=>a+b*b,0);
  const h=[r[1]*v[2]-r[2]*v[1],r[2]*v[0]-r[0]*v[2],r[0]*v[1]-r[1]*v[0]],h2=h.reduce((a,b)=>a+b*b,0);
  const energy=speed2/2-MU/radius;if(!(energy<0))return null;
  const a=-MU/(2*energy),e=Math.sqrt(Math.max(0,1+2*energy*h2/(MU*MU)));
  return {apo:(a*(1+e)-R)/1000,peri:(a*(1-e)-R)/1000};
}
function syncFields(){
  const n=nodes[selected];$('node-select').innerHTML='';
  if(!nodes.length){const o=document.createElement('option');o.textContent='No maneuver';$('node-select').append(o);}
  nodes.forEach((node,i)=>{const o=document.createElement('option');o.value=String(i);o.textContent=`Maneuver ${i+1} · ${Math.round(node.time)} s`;o.selected=i===selected;$('node-select').append(o);});
  for(const control of [...Object.values(fields),$('node-select'),$('remove-node')])control.disabled=!n||!!locked;
  if(n){fields.time.value=String(Math.round(n.time));fields.t.value=String(+n.dv[0].toFixed(2));fields.n.value=String(+n.dv[1].toFixed(2));fields.w.value=String(+n.dv[2].toFixed(2));}
  $('add-node').disabled=!wasm||!!locked||nodes.length>=12;$('example').disabled=!wasm||!!locked;$('mission-select').disabled=!wasm||!!locked;
}
function updatePreview(schedule=true){
  if(!wasm)return;
  cancelFlight();sequence++;syncFields();actual=null;lastReport=null;
  if(!configure(wasm,nodes)){$('plan-status').textContent='Unable to compile: check timing, overlapping burns, or available propellant.';$('plan-status').classList.add('failure');preview=[];$('prefill').disabled=true;$('execute').disabled=true;updateCausal();draw();cancelFlight();return;}
  $('plan-status').classList.remove('failure');$('plan-status').textContent='Ideal orbit ready. The modeled flight calculates in the background.';
  preview=Array.from({length:361},(_,i)=>({time:DURATION*i/360,state:readState(wasm,DURATION*i/360)}));
  if(preview.some(p=>p.state.some(x=>!Number.isFinite(x)))){preview=[];$('plan-status').textContent='The ideal orbit could not be sampled.';cancelFlight();draw();return;}
  $('prefill').disabled=false;$('execute').disabled=nodes.length===0||!!locked;
  syncFields();updateCausal();draw();if(schedule)scheduleFlight();
}
function cancelFlight(){if(timer!==null){clearTimeout(timer);timer=null;}if(worker){worker.terminate();worker=null;}}
function scheduleFlight(){cancelFlight();const version=++sequence;timer=setTimeout(()=>{timer=null;startFlight(false,version);},350);}
function startFlight(isLocked,version=++sequence){
  cancelFlight();worker=new Worker('./mission-worker.js',{type:'module'});const current=worker;
  const snapshot=nodes.map(n=>({time:n.time,dv:[...n.dv]}));
  current.onmessage=({data})=>{messages++;if(worker!==current||version!==sequence)return;current.terminate();worker=null;
    if(data.error){$('plan-status').textContent=data.error;$('plan-status').classList.add('failure');if(isLocked)$('execute-status').textContent=data.error;return;}
    actual=data.points;lastReport=data.report;updateCausal();draw();$('plan-status').textContent='Flight model ready. No simulation runs while idle.';
    if(isLocked)showDebrief(data.report);
  };
  current.onerror=()=>{if(worker!==current)return;worker.terminate();worker=null;$('plan-status').textContent='Flight worker failed.';if(isLocked)$('execute-status').textContent='Flight worker failed.';};
  messages++;current.postMessage({setup,nodes:snapshot,metric:scenario==='102'?1:0});
}
function prefill(){
  const state=readState(wasm,DURATION),metrics=orbitMetrics(state);if(!metrics){$('execute-status').textContent='The ideal orbit has no finite farthest point.';return false;}
  $('predict-dv').value=nodes.reduce((a,n)=>a+Math.hypot(...n.dv),0).toFixed(3);
  $('predict-apo').value=metrics.apo.toFixed(3);$('predict-peri').value=metrics.peri.toFixed(3);
  $('predict-fuel').value=(state[6]-DRY).toFixed(3);$('predict-time').value=String(DURATION);$('execute-status').textContent='';return true;
}
function showDebrief(report){
  const [mass,apo,peri,e,rescueScore,dv]=report,p=locked.prediction;
  const drift=report[28]*180/Math.PI,inclination=report[26]*180/Math.PI;
  const maxError=Math.max(Math.abs(apo/1000-800)/10,Math.abs(peri/1000-800)/10,Math.abs(drift-4.93)/0.15,Math.abs(inclination-98.60)/0.25);
  const score=scenario==='101'?rescueScore:(e<0.005&&mass>=DRY&&maxError<=1?1-0.2*maxError:0);
  const fmt=(x,n=1)=>Number(x).toLocaleString('en-US',{maximumFractionDigits:n,minimumFractionDigits:n});
  const entries=[['Mission score',`${fmt(score*100,1)}%`],['Farthest altitude',`${fmt(apo/1000)} km`],['Closest altitude',`${fmt(peri/1000)} km`],['Remaining propellant',`${fmt(mass-DRY,2)} kg`],['Orbit eccentricity',fmt(e,5)],['Commanded velocity change',`${fmt(dv,2)} m/s`]];
  if(scenario==='102')entries.push(['Orbital-plane tilt',`${fmt(inclination,2)}°`],['Five-day node rotation',`${fmt(drift,3)}°`]);
  const card=document.createElement('div');card.className='debrief-grid';for(const [label,value] of entries){const item=document.createElement('div'),strong=document.createElement('strong'),span=document.createElement('span');strong.textContent=value;span.textContent=label;item.append(strong,span);card.append(item);}
  const compare=document.createElement('p');compare.textContent=`Locked prediction: ${fmt(p.apo)} km farthest, ${fmt(p.peri)} km closest, ${fmt(p.fuel,2)} kg propellant and ${fmt(p.dv,2)} m/s velocity change after ${fmt(p.time,0)} seconds.`;
  const heading=document.createElement('h3');heading.textContent=scenario==='102'?'Orbit-node rotation sensitivity':'Farthest-altitude sensitivity';
  const list=document.createElement('div');list.className='sensitivity-list';const names=['Burn duration','Earth’s equatorial bulge','Moon and Sun gravity','Atmospheric drag','Sunlight pressure'];
  for(let i=0;i<5;i++){const row=document.createElement('div'),share=report[11+i],delta=report[16+i];row.textContent=Number.isFinite(share)?`${names[i]}: ${fmt(share*100)}% of sensitivity · signed change ${scenario==='102'?fmt(delta*180/Math.PI,3)+'°':fmt(delta/1000,2)+' km'}`:`${names[i]}: sensitivity share undefined`;list.append(row);}
  $('debrief-content').replaceChildren(card,compare,heading,list);$('debrief').hidden=false;$('execute-status').textContent='Flight complete. The prediction remains locked.';
}
function updateCausal(){
  const target=$('causal-text');target.replaceChildren();$('split-kick').hidden=true;
  const chain=(symptom,mechanism,cause)=>{const row=document.createElement('div');row.className='cause-chain';const strong=document.createElement('strong');strong.textContent=symptom;row.append(strong,document.createTextNode(` ← ${mechanism} ← ${cause}`));target.append(row);};
  const planned=nodes.reduce((sum,n)=>sum+Math.hypot(...n.dv),0);
  const needed=setup[2]*(-Math.expm1(-planned/(setup[5]*9.80665)));
  if(needed>setup[2]-DRY)chain('Propellant deficit','The requested speed changes exceed the available propellant','The ideal rocket equation estimates '+needed.toFixed(1)+' kg required');
  if(preview.length){
    const peris=preview.filter((_,i)=>i%15===0).map(p=>orbitMetrics(p.state)?.peri).filter(Number.isFinite);
    if(peris.length&&Math.min(...peris)<120)chain('Low closest altitude','Atmospheric drag or impact may dominate','The ideal orbit dips below 120 km; propagation cannot model landing');
  }
  if(scenario==='101'&&lastReport&&preview.length){
    const ideal=orbitMetrics(preview[preview.length-1].state),actualApo=lastReport[1]/1000;
    if(ideal&&ideal.apo>0&&(ideal.apo-actualApo)/ideal.apo>0.02){
      const finite=lastReport[16]/1000;
      chain('Farthest-point shortfall: '+(ideal.apo-actualApo).toFixed(1)+' km','Finite burn comparison changes the farthest altitude by '+finite.toFixed(1)+' km','Thrust is spread over time rather than applied at a single point; other forces also contribute');
      const n=nodes[selected];if(n&&n.dv[0]>0&&Math.abs(n.dv[1])<1e-9&&Math.abs(n.dv[2])<1e-9&&!locked)$('split-kick').hidden=false;
    }
  }
  if(!target.childNodes.length)target.textContent='No rule currently triggered. This is a limited check, not a safety certification.';
}
$('split-kick').addEventListener('click',()=>{
  const original=nodes[selected];if(!original||original.dv[0]<=0)return;
  const old=nodes.map(n=>({time:n.time,dv:[...n.dv]}));
  original.dv[0]*=0.5;
  if(!configure(wasm,nodes)){nodes=old;updatePreview();return;}
  const state=readState(wasm,original.time),radius=Math.hypot(...state.slice(0,3)),speed2=state.slice(3,6).reduce((a,b)=>a+b*b,0),energy=speed2/2-MU/radius;
  if(!(energy<0)){nodes=old;updatePreview();$('plan-status').textContent='A second pass cannot be estimated for an unbound intermediate orbit.';return;}
  const a=-MU/(2*energy),period=2*Math.PI*Math.sqrt(a*a*a/MU),second=original.time+period;
  if(!Number.isFinite(second)||second>=DURATION-1){nodes=old;updatePreview();$('plan-status').textContent='The next ideal periapsis falls outside the current mission duration.';return;}
  nodes.push({time:second,dv:[original.dv[0],0,0]});selected=nodes.length-1;updatePreview();
  $('plan-status').textContent='Second push placed at the next ideal periapsis. The finite-burn trajectory may reach periapsis at a different time; check the gold path.';
});
function toCanvas(point){const rect=canvas.getBoundingClientRect(),span=2.45*TARGET;return [rect.width/2+point[0]*rect.height/span,rect.height/2-(point[1]*plane[0]+point[2]*plane[1])*rect.height/span];}
function draw(){
  const rect=canvas.getBoundingClientRect();if(!rect.width||!rect.height)return;const dpr=Math.min(devicePixelRatio||1,2);canvas.width=Math.round(rect.width*dpr);canvas.height=Math.round(rect.height*dpr);ctx.setTransform(dpr,0,0,dpr,0,0);ctx.clearRect(0,0,rect.width,rect.height);draws++;
  const center=toCanvas([0,0,0]),earth=R*rect.height/(2.45*TARGET);
  const gradient=ctx.createRadialGradient(center[0]-earth*.25,center[1]-earth*.3,earth*.04,center[0],center[1],earth);gradient.addColorStop(0,'#607f71');gradient.addColorStop(.75,'#254b4a');gradient.addColorStop(1,'#102b33');ctx.beginPath();ctx.arc(...center,earth,0,Math.PI*2);ctx.fillStyle=gradient;ctx.fill();ctx.strokeStyle='#6e9e8c';ctx.lineWidth=1;ctx.stroke();
  const line=(pts,color,dashed)=>{ctx.beginPath();ctx.setLineDash(dashed?[4,5]:[]);ctx.strokeStyle=color;ctx.lineWidth=2;pts.forEach((p,i)=>{const xy=toCanvas(p);if(i)ctx.lineTo(...xy);else ctx.moveTo(...xy);});ctx.stroke();ctx.setLineDash([]);};
  if(preview.length)line(preview.map(p=>p.state),'#70d7dc',true);if(actual)line(actual,'#e2bc79',false);
  nodes.forEach((node,i)=>{const state=readState(wasm,node.time);if(state.some(x=>!Number.isFinite(x)))return;const p=toCanvas(state);ctx.fillStyle=i===selected?'#f0ddac':'#a1c2b2';ctx.beginPath();ctx.arc(...p,5,0,Math.PI*2);ctx.fill();ctx.fillStyle='#e4eada';ctx.font='11px system-ui';ctx.fillText(String(i+1),p[0]+9,p[1]-8);if(i===selected){for(const h of handles(p,state)){ctx.strokeStyle=h.color;ctx.beginPath();ctx.moveTo(...p);ctx.lineTo(...h.end);ctx.stroke();ctx.fillStyle=h.color;ctx.beginPath();ctx.arc(...h.end,6,0,Math.PI*2);ctx.fill();ctx.fillText(h.label,h.end[0]+7,h.end[1]-6);}}});
}
function handles(p,state){const v=[state[3],-(state[4]*plane[0]+state[5]*plane[1])],m=Math.hypot(...v)||1,t=v.map(x=>x/m),n=[-t[1],t[0]];return [{key:'t',dir:t,color:'#80d9cb',label:'Along'},{key:'n',dir:n,color:'#e1bc76',label:'Sideways'},{key:'w',dir:[.707,-.707],color:'#bdade0',label:'Plane tilt'}].map(h=>({...h,end:[p[0]+h.dir[0]*42,p[1]+h.dir[1]*42]}));}
function mousePoint(e){const r=canvas.getBoundingClientRect();return [e.clientX-r.left,e.clientY-r.top];}
function nearestPath(p){let best=null;for(const item of preview){const d=distance(p,toCanvas(item.state));if(!best||d<best.d)best={d,time:item.time};}return best;}
function setLens(key){$('lens-text').textContent=lens[key]||lens.time;}
for(const key of Object.keys(fields)){
  fields[key].addEventListener('focus',()=>setLens(key));fields[key].addEventListener('mouseenter',()=>setLens(key));
  fields[key].addEventListener('input',()=>{if(locked||selected<0)return;const value=Number(fields[key].value);if(!Number.isFinite(value))return;if(key==='time')nodes[selected].time=Math.max(1,Math.min(DURATION-1,value));else nodes[selected].dv[{t:0,n:1,w:2}[key]]=value;updatePreview();});
}
$('node-select').addEventListener('change',e=>{selected=Number(e.target.value);syncFields();draw();});
$('add-node').addEventListener('click',()=>{nodes.push({time:200,dv:[10,0,0]});selected=nodes.length-1;updatePreview();});
$('remove-node').addEventListener('click',()=>{if(selected<0)return;nodes.splice(selected,1);selected=Math.min(selected,nodes.length-1);updatePreview();});
$('example').addEventListener('click',()=>{const a=(INITIAL+TARGET)/2,coast=Math.PI*Math.sqrt(a*a*a/MU);
  if(scenario==='102'){const vAtFar=Math.sqrt(MU*(2/TARGET-1/a)),circular=Math.sqrt(MU/TARGET),tilt=2.6*Math.PI/180;nodes=[{time:coast,dv:[circular*Math.cos(tilt)-vAtFar,0,-circular*Math.sin(tilt)]}];}
  else{const v1=Math.sqrt(MU/INITIAL),v2=Math.sqrt(MU/TARGET);nodes=[{time:200,dv:[v1*(Math.sqrt(TARGET/a)-1),0,0]},{time:200+coast,dv:[v2*(1-Math.sqrt(INITIAL/a)),0,0]}];}
  selected=0;updatePreview();prefill();});
$('mission-select').addEventListener('change',()=>{
  if(locked)return;scenario=$('mission-select').value;cancelFlight();sequence++;nodes=[];selected=-1;actual=null;lastReport=null;
  if(scenario==='102'){
    INITIAL=R+250000;TARGET=R+800000;DURATION=432000;plane=[Math.cos(96*Math.PI/180),Math.sin(96*Math.PI/180)];const a=(INITIAL+TARGET)/2,v=Math.sqrt(MU*(2/INITIAL-1/a));setup=[820497600,DURATION,1200,DRY,2400,310,INITIAL,0,0,0,v*plane[0],v*plane[1]];
    $('mission-rank').textContent='CONTRACT 102 / FLIGHT DYNAMICS OFFICER';$('mission-title').textContent='Follow the sunlight';$('mission-description').textContent='Circularize at 800 km and tilt the orbit so its crossing of the equator turns by about 4.93° in five days. Engine and spacecraft mass use the rescue prototype assumptions.';$('example').textContent='Show circularization example';
  }else{
    INITIAL=R+260000;TARGET=R+550000;DURATION=10800;plane=[1,0];setup=[820497600,DURATION,1200,DRY,2400,310,INITIAL,0,0,0,Math.sqrt(MU/INITIAL),0];
    $('mission-rank').textContent='CONTRACT 101 / CADET';$('mission-title').textContent='Orbit rescue';$('mission-description').textContent='Raise a 260 km circular orbit to 550 km, then make it circular within three hours. The dotted cyan path is your instantaneous-push preview; the gold path includes finite burns and modeled forces.';$('example').textContent='Show two-push example';
  }
  fields.time.max=String(DURATION-1);$('debrief').hidden=true;for(const id of ['predict-dv','predict-apo','predict-peri','predict-fuel','predict-time'])$(id).value='';updatePreview();
});
$('prefill').addEventListener('click',prefill);
$('execute').addEventListener('click',()=>{
  const ids=['predict-dv','predict-apo','predict-peri','predict-fuel','predict-time'];
  const vals=ids.map(id=>Number($(id).value));
  if(ids.some(id=>$(id).value.trim()==='')||vals.some(x=>!Number.isFinite(x))||vals[0]<0||vals[3]<0||vals[4]<=0){$('execute-status').textContent='Enter a valid prediction for each field.';return;}
  locked=Object.freeze({prediction:Object.freeze({dv:vals[0],apo:vals[1],peri:vals[2],fuel:vals[3],time:vals[4]})});
  for(const id of ['predict-dv','predict-apo','predict-peri','predict-fuel','predict-time','prefill','execute'])$(id).disabled=true;
  $('unlock').hidden=false;$('debrief').hidden=true;$('execute-status').textContent='Prediction locked. Simulating the flight and its comparisons…';syncFields();startFlight(true);
});
$('unlock').addEventListener('click',()=>{locked=null;cancelFlight();sequence++;$('unlock').hidden=true;$('debrief').hidden=true;$('execute-status').textContent='';for(const id of ['predict-dv','predict-apo','predict-peri','predict-fuel','predict-time'])$(id).disabled=false;updatePreview();});
canvas.addEventListener('pointerdown',e=>{
  if(locked||!preview.length||e.button!==0)return;const p=mousePoint(e),node=nodes[selected];
  if(node){const state=readState(wasm,node.time),center=toCanvas(state);for(const h of handles(center,state))if(distance(p,h.end)<12){drag={pointer:e.pointerId,key:h.key,start:p,initial:node.dv[{t:0,n:1,w:2}[h.key]],dir:h.dir};setLens(h.key);canvas.setPointerCapture(e.pointerId);return;}if(distance(p,center)<12){drag={pointer:e.pointerId,key:'time'};setLens('time');canvas.setPointerCapture(e.pointerId);return;}}
  const nearest=nearestPath(p);if(nearest&&nearest.d<12&&nodes.length<12){nodes.push({time:Math.max(60,Math.min(DURATION-60,nearest.time)),dv:[10,0,0]});selected=nodes.length-1;updatePreview();}
});
canvas.addEventListener('pointermove',e=>{if(!drag||drag.pointer!==e.pointerId||selected<0)return;const p=mousePoint(e),node=nodes[selected];if(drag.key==='time'){const near=nearestPath(p);if(near)node.time=Math.max(60,Math.min(DURATION-60,near.time));}else{const i={t:0,n:1,w:2}[drag.key];node.dv[i]=drag.initial+2*((p[0]-drag.start[0])*drag.dir[0]+(p[1]-drag.start[1])*drag.dir[1]);}updatePreview(false);});
function endDrag(e){if(drag&&drag.pointer===e.pointerId){drag=null;scheduleFlight();}}
canvas.addEventListener('pointerup',endDrag);canvas.addEventListener('pointercancel',endDrag);canvas.addEventListener('lostpointercapture',endDrag);
new ResizeObserver(draw).observe(canvas.parentElement);
window.missionDiagnostics=Object.freeze({get nodes(){return nodes.map(n=>({time:n.time,dv:[...n.dv]}));},get pending(){return !!worker||timer!==null;},get draws(){return draws;},get messages(){return messages;},get locked(){return !!locked;},get handles(){if(selected<0||!wasm)return [];const state=readState(wasm,nodes[selected].time);return handles(toCanvas(state),state).map(h=>({key:h.key,end:h.end}));}});
try{const response=await fetch('./odd_view.wasm');if(!response.ok)throw Error('The physics module is not available. Build it first.');({instance:{exports:wasm}}=await WebAssembly.instantiate(await response.arrayBuffer(),{}));syncFields();updatePreview();}catch(e){$('plan-status').classList.add('failure');$('plan-status').textContent=String(e.message||e);}
