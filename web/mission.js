// Playable mission planning. Scientific calculations are performed by the
// Rust/WASM planner and one-shot flight workers, not fabricated by canvas.
import { Governor } from './governor.js';
import { createLab } from './engine-lab.js';
import { createAssembly, assemblyPayload, applyAssembly } from './assembly.js';
import * as identity from './identity.js';
import { initProfile, openProfile, syncProfile } from './profile.js';
import * as recorder from './recorder.js';
import { initVerify, openVerify, result as verificationResult } from './verify.js';
import * as authority from './authority.js';
import { verifyReceipt } from './authority-crypto.js';
import { initLeaderboard } from './leaderboard.js';
import { readMissionNodes } from './mission-nodes.js';
const $=id=>document.getElementById(id);
const MU=3.986004418e14,R=6378137,G0=9.80665;
let INITIAL=R+260000,TARGET=R+550000,DURATION=10800,DRY=1020,plane=[1,0],scenario='101',contractId=101;
// One boundary for every node-time entry path: typed input, path click and
// pointer drag. Plan::compile rejects epochs <= 0 or beyond the flight end,
// so the legal window is the open interval (0, DURATION) sampled at 1 s.
const NODE_TIME_MIN=1;
const nodeTimeRange=()=>[NODE_TIME_MIN,DURATION-NODE_TIME_MIN];
const clampNodeTime=t=>{const [lo,hi]=nodeTimeRange();return Math.max(lo,Math.min(hi,t));};
let setup=[820497600,DURATION,1200,DRY,2400,310,INITIAL,0,0,0,Math.sqrt(MU/INITIAL),0];
const canvas=$('plan-canvas'),ctx=canvas.getContext('2d');
const lensCanvas=$('lens-canvas'),lensCtx=lensCanvas.getContext('2d');
let wasm,nodes=[],compiledOrder=[],selected=-1,preview=[],lunarPreview=[],actual=null,lastReport=null,lastLunarInfo=null,lastBplaneInfo=null,worker=null,locked=null,drag=null,draws=0,messages=0,sequence=0;
// A local, best-effort guard against using numerical practice as a ranked
// planning preview. The server cannot inspect or trust a user's browser.
const practiceSimulated=new Set();
let activeLens=null,activeTerm=null,lensPhase=0,lensDraws=0,lastComputeMs=null,lastCapture=null,lastRecorder=null,engineLab=null,authorityPending=0;
// The design the stage actually flies. The lab can be mid-edit without this
// changing: only "Fit this engine to the stage" commits it. mission_contract()
// rebuilds the baseline, so every reconfigure must re-apply this afterwards or
// the preview would silently revert while the worker flew the new engine.
let appliedEngine=null;
const CONTRACT_RATINGS={101:800,102:1250,103:1650};
const lensGovernor=new Governor(()=>drawLens());
const SHAPLEY_NAMES=['Finite-burn arc','Earth equatorial bulge','Moon and Sun gravity','Atmospheric drag and sunlight pressure'];
const SHAPLEY_WHY=[
  'Thrust was spread over time instead of applied at one point; steering and gravity losses reduced the useful speed change.',
  'Earth’s equatorial bulge twisted the orbital plane and apsides away from the point-mass prediction.',
  'Moon and Sun gravity pulled the spacecraft off the Earth-only conic.',
  'Atmospheric drag removed energy and sunlight pressure pushed on the spacecraft area.'
];
// The hover registry is loaded from odd-app::lens through the scalar bridge.
// Keeping one registry prevents a new Rust warning term from rendering naked.
let GLOSSARY={},TERM_RE=null,assembly=null,assemblyDesign=null,lastBoilOff=null,lastDelegationMask=0;
// Art keys come from odd_app::lens::Projection::key through the bridge; the
// browser must not restate the Rust declaration order.
function wasmString(len,byte){const bytes=new Uint8Array(len);for(let i=0;i<len;i++)bytes[i]=byte(i);return new TextDecoder().decode(bytes);}
function loadGlossary(w){
  GLOSSARY={};const count=w.mission_acronym_count();
  for(let i=0;i<count;i++){
    const term=wasmString(w.mission_acronym_len(i,0),j=>w.mission_acronym_byte(i,0,j));
    const expansion=wasmString(w.mission_acronym_len(i,1),j=>w.mission_acronym_byte(i,1,j));
    const plain=wasmString(w.mission_acronym_len(i,2),j=>w.mission_acronym_byte(i,2,j));
    const key=wasmString(w.mission_acronym_key_len(i),j=>w.mission_acronym_key_byte(i,j));
    GLOSSARY[term]={expansion,plain,key:key||'t'};
  }
  TERM_RE=new RegExp(`\\b(${Object.keys(GLOSSARY).sort((a,b)=>b.length-a.length).join('|')})\\b`,'g');
}
function lensNodeIndex(){return selected<0?-1:compiledOrder.indexOf(selected);}
const lens={
  tnw:'Three perpendicular push directions: along motion, sideways in the orbital plane, and out of the plane. Hover a colored handle to isolate one direction.',
  leo:'A close Earth parking orbit, here a few hundred kilometers up. The projection compares the parking circle with Earth’s surface.',
  dag:'A cause-and-effect chain with no loops. Each warning points from symptom to mechanism to cause to a one-click demonstration.',
  pdr:'An engineering dossier comparing your locked prediction with modeled flight, signed error contributions and any verified server receipt.',
  mli:'Each film layer re-radiates heat to the next, so the leak falls as layers are added. The projection shows the outer skin running hot while only a small fraction reaches the liquid. Past about twenty layers the support struts, not the blanket, set the floor.',
  cop:'A cryocooler lifts heat from the cold tank and rejects it, plus the electricity it consumed, through a warm radiator. The projection compares heat lifted with power drawn: at 20 K against a 300 K sink the electricity is one to two orders of magnitude larger, and the Carnot ceiling caps the ratio.',
  gnc:'Guidance decides where to point, navigation decides where you are, and control moves the actuators. The projection shows the attitude hardware drawing from the same electrical bus that feeds the cooler and the propulsion.',
  'boil-off':'Heat leaking into a cryogenic tank vaporizes liquid. Below the relief setting the tank self-pressurizes; at relief it vents and the model reports the estimated loss. This version does not feed that loss back into orbital mass or the fuel budget. The projection shows the liquid level falling as heat arrives.',
  t:'A push along the direction of travel changes orbital energy. Near the closest point, a forward push raises the farthest point of the orbit; a backward push lowers it. The projection shows kinetic-energy gain or loss and the opposite side breathing outward or inward.',
  n:'This sideways push stays in the orbital plane and is perpendicular to the direction of travel. Near an apsis it points roughly inward or outward; away from one it is not exactly radial. The projection rotates the eccentricity vector.',
  w:'This push is perpendicular to the orbital plane. It changes the direction of the orbit’s angular momentum and tilts the plane about the line of nodes. The projection shows torque as radius crossed with force.',
  time:'Move the maneuver along the predicted path. Timing changes where the engine pushes and how the rest of the orbit responds. A guided perigee kick uses the numerically propagated pre-burn state to freeze its inertial direction.',
  twr:'Thrust-to-weight ratio compares engine thrust with vehicle weight. Drag the demonstration slider to see why a ratio below one cannot lift the vehicle.',
  isp:'Specific impulse measures propellant efficiency. The projection shows mass flowing out to produce the same push.',
  raan:'The ascending node is where the orbit crosses the equator from south to north. Its right ascension is measured around the equator.',
  aop:'Argument of periapsis locates the closest point inside the orbital plane, measured from the ascending node.',
  soi:'A sphere of influence is a patched-conic preview boundary, not a physical gravity switch.',
  j2:'Earth’s equatorial bulge adds a small twisting acceleration that rotates the orbital plane.',
  srp:'Sunlight carries momentum and pushes gently on the spacecraft area.',
  'gravity-loss':'A finite burn loses useful speed while gravity rotates the velocity vector. Splitting the burn near periapsis reduces the loss.',
  periapsis:'Raise the closest point above the atmosphere to avoid drag and impact.',
  propellant:'The rocket equation ties speed change to propellant mass. Reduce the demand or add tankage.',
  apoapsis:'Concentrate pushes near periapsis to raise the farthest point efficiently.',
  impact:'The trajectory reached the reference surface. Raise periapsis before the burn.',
  'split-kick':'Split one long burn into shorter perigee kicks to reduce steering and gravity loss.',
  impact:'The trajectory crossed Earth’s reference surface. Propagation stops there; no underground flight is drawn.',
  propellant:'The rocket equation ties speed change to propellant mass. The projection compares required propellant with the tank budget.',
  periapsis:'Raise the closest point above the atmosphere to avoid drag and impact. The projection compares periapsis with the 120 km reference.',
  apoapsis:'Concentrate pushes near periapsis to raise the farthest point efficiently. The projection compares ideal and finite-burn apoapsis.'
};
const fields={time:$('node-time'),t:$('node-t'),n:$('node-n'),w:$('node-w')};
const distance=(a,b)=>Math.hypot(a[0]-b[0],a[1]-b[1]);
function readState(w,t){return Array.from({length:7},(_,i)=>w.mission_preview(t,i));}
/// Re-apply the committed design after any call that reloads the contract.
/// Returns false when the design no longer fits, so the caller reports it
/// instead of propagating a different stage than the one on screen.
function reapplyEngine(w){
  if(!appliedEngine||!contractId)return true;
  const e=appliedEngine;
  if(w.mission_engine_solve(e.propellant,e.of,e.pc,e.throat,e.exit,e.length,0)!==1)return false;
  return w.mission_engine_apply()===1;
}
// The bridge refuses a plan with a stable code; say which failure it was
// instead of one sentence naming all of them. Kept in step with
// mission_compile_error() in crates/odd-view/src/mission_bridge.rs.
function refusal(w){
  return {
    1:'Those maneuver numbers are not usable.',
    2:'That maneuver cannot be flown on this orbit.',
    3:'The flight could not be computed from those maneuvers.',
    4:'That needs more fuel than this stage carries.',
    5:'That burn falls outside the mission window. Move it earlier.',
  }[w.mission_compile_error()]||'The flight could not be compiled.';
}
function configure(w,items){
  if(contractId){if(!w.mission_contract(contractId))return false;if(!applyAssembly(w,assemblyDesign))return false;if(!reapplyEngine(w))return false;}
  else if(!w.mission_setup(...setup))return false;
  if(!w.mission_metric(scenario==='102'?1:0))return false;
  const ordered=items.map((n,i)=>({n,i})).filter(x=>Math.hypot(...x.n.dv)>=1e-12).sort((a,b)=>a.n.time-b.n.time);
  compiledOrder=ordered.map(x=>x.i);
  let index=0;
  for(const {n:item} of ordered){
    if(!w.mission_add_node(item.time,...item.dv))return false;
    if(item.guidance&&!w.mission_add_guidance(index,...item.guidance))return false;
    index++;
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
function annotateAcronyms(root){
  if(!root||!TERM_RE)return;
  const walker=document.createTreeWalker(root,NodeFilter.SHOW_TEXT,{acceptNode(node){
    const p=node.parentElement;
    if(!p||p.closest('abbr,select,option,script,style,canvas,input,textarea'))return NodeFilter.FILTER_REJECT;
    TERM_RE.lastIndex=0;return TERM_RE.test(node.nodeValue)?NodeFilter.FILTER_ACCEPT:NodeFilter.FILTER_REJECT;
  }});
  const texts=[];while(walker.nextNode())texts.push(walker.currentNode);
  for(const node of texts){
    const frag=document.createDocumentFragment();let last=0;TERM_RE.lastIndex=0;let m;
    while((m=TERM_RE.exec(node.nodeValue))){
      frag.append(document.createTextNode(node.nodeValue.slice(last,m.index)));
      const abbr=document.createElement('abbr');abbr.className='term';abbr.tabIndex=0;abbr.dataset.term=m[1];abbr.setAttribute('aria-describedby','lens-projection');abbr.textContent=m[1];frag.append(abbr);last=m.index+m[1].length;
    }
    frag.append(document.createTextNode(node.nodeValue.slice(last)));node.replaceWith(frag);
  }
}
function acronymViolations(){
  if(!TERM_RE)return ['glossary-not-loaded'];
  const out=[];const walker=document.createTreeWalker(document.body,NodeFilter.SHOW_TEXT,{acceptNode(node){
    const p=node.parentElement;if(!p||p.closest('abbr,select,option,script,style,canvas,input,textarea'))return NodeFilter.FILTER_REJECT;
    TERM_RE.lastIndex=0;return TERM_RE.test(node.nodeValue)?NodeFilter.FILTER_ACCEPT:NodeFilter.FILTER_REJECT;
  }});while(walker.nextNode()){TERM_RE.lastIndex=0;let m;while((m=TERM_RE.exec(walker.currentNode.nodeValue)))out.push(m[1]);}return out;
}
function syncFields(){
  const n=nodes[selected];$('node-select').innerHTML='';
  if(!nodes.length){const o=document.createElement('option');o.textContent='No maneuver';$('node-select').append(o);}
  nodes.forEach((node,i)=>{const o=document.createElement('option');o.value=String(i);o.textContent=`Maneuver ${i+1} · ${Math.round(node.time)} s${node.guidance?' · guided':''}`;o.selected=i===selected;$('node-select').append(o);});
  for(const control of [...Object.values(fields),$('node-select'),$('remove-node')])control.disabled=!n||!!locked;
  fields.time.min=String(nodeTimeRange()[0]);fields.time.max=String(nodeTimeRange()[1]);
  if(n){fields.time.value=String(Math.round(n.time));fields.t.value=String(+n.dv[0].toFixed(2));fields.n.value=String(+n.dv[1].toFixed(2));fields.w.value=String(+n.dv[2].toFixed(2));}
  $('add-node').disabled=!wasm||!!locked||nodes.length>=12;$('example').disabled=!wasm||!!locked;$('mission-select').disabled=!wasm||!!locked;$('split-kick').disabled=!!locked;$('target-bplane').disabled=!!locked||!lastBplaneInfo;
}
function updatePreview(){
  if(!wasm)return;
  cancelFlight();sequence++;syncFields();actual=null;lastReport=null;lastLunarInfo=null;lastBplaneInfo=null;lunarPreview=[];
  if(!configure(wasm,nodes)){$('plan-status').textContent=refusal(wasm);$('plan-status').classList.add('failure');preview=[];$('prefill').disabled=true;$('execute').disabled=true;
  updateAttemptMode();updateCausal();draw();cancelFlight();return;}
  $('plan-status').classList.remove('failure');$('plan-status').textContent='Ideal orbit ready. The modeled flight runs only after the prediction is locked.';
  preview=Array.from({length:481},(_,i)=>({time:DURATION*i/480,state:readState(wasm,DURATION*i/480)}));
  if(scenario==='103'){
    const valid=wasm.mission_lunar_info(0);
    lastLunarInfo=valid?[wasm.mission_lunar_info(1),wasm.mission_lunar_info(2)]:null;
    if(valid)lunarPreview=Array.from({length:481},(_,i)=>readStatePatched(wasm,DURATION*i/480)).filter(s=>s.every(Number.isFinite));
    const bvalid=wasm.mission_bplane_info(0);
    lastBplaneInfo=bvalid?[wasm.mission_bplane_info(1),wasm.mission_bplane_info(2),wasm.mission_bplane_info(3)]:null;
  } else {
    lastBplaneInfo=null;
  }
  if(preview.some(p=>p.state.some(x=>!Number.isFinite(x)))){preview=[];$('plan-status').textContent='The ideal orbit could not be sampled.';cancelFlight();draw();return;}
  $('prefill').disabled=false;$('execute').disabled=nodes.length===0||!!locked;
  syncFields();updateCausal();draw();annotateAcronyms($('plan-status'));
}
function readStatePatched(w,t){return Array.from({length:7},(_,i)=>w.mission_lunar_preview(t,i));}
function cancelFlight(){if(worker){worker.terminate();worker=null;}}
function workerPayload(){
  const snapshot=nodes.map(n=>({time:n.time,dv:[...n.dv],guidance:n.guidance?[...n.guidance]:null}));
  const payload={contract:contractId,setup,nodes:snapshot,metric:scenario==='102'?1:0,callsign:identity.profile()?.callsign??''};
  // Send the committed physical design, not the performance it produces and
  // not whatever the lab is currently previewing.
  if(appliedEngine)payload.engine={...appliedEngine};
  if(assemblyDesign)payload.assembly={...assemblyDesign};
  const p=locked.prediction;payload.locked=[p.dv,p.apo*1000,p.peri*1000,p.fuel,p.time,Number.isFinite(p.miss)?p.miss:NaN];
  return payload;
}
function startFlight(version,flightContext){
  cancelFlight();worker=new Worker('./mission-worker.js',{type:'module'});const current=worker;
  current.onmessage=({data})=>{messages++;if(worker!==current||version!==sequence)return;current.terminate();worker=null;
    if(data.error){$('plan-status').textContent=data.error;$('plan-status').classList.add('failure');$('execute-status').textContent=data.error;return;}
    actual=data.points;lastReport=data.report;lastCapture=data.capture??null;lastBoilOff=data.boilOff??null;lastDelegationMask=data.delegationMask??0;lastRecorder=null;lastComputeMs=Number.isFinite(data.computeMs)?data.computeMs:null;lastLunarInfo=data.lunarInfo?.[0]?data.lunarInfo:null;if(data.lunar?.length)lunarPreview=data.lunar;updateCausal();draw();$('plan-status').textContent='Flight model ready. No simulation runs while idle.';annotateAcronyms($('plan-status'));
    showDebrief(data.report,flightContext,version);
  };
  current.onerror=()=>{if(worker!==current)return;worker.terminate();worker=null;$('plan-status').textContent='Flight worker failed.';$('execute-status').textContent='Flight worker failed.';};
  const payload=workerPayload();
  if(flightContext){payload.attemptMode=flightContext.ticket?.mode??1;payload.callsign=flightContext.callsign;}
  messages++;current.postMessage(payload);
}
function prefill(){
  const state=readState(wasm,DURATION),metrics=orbitMetrics(state);if(!metrics){$('execute-status').textContent='The ideal orbit has no finite farthest point.';return false;}
  $('predict-dv').value=nodes.reduce((a,n)=>a+Math.hypot(...n.dv),0).toFixed(3);
  $('predict-apo').value=metrics.apo.toFixed(3);$('predict-peri').value=metrics.peri.toFixed(3);
  $('predict-fuel').value=(state[6]-DRY).toFixed(3);$('predict-time').value=String(DURATION);$('predict-miss').value='';$('execute-status').textContent='';return true;
}
function showDebrief(report,flightContext,version){
  const [mass,apo,peri,e,rescueScore,dv]=report,p=locked.prediction;
  const drift=report[28]*180/Math.PI,inclination=report[26]*180/Math.PI;
  const impacted=report[30]===1,entry=report[31],evaluation=report[29],attrEnd=report[18],truncated=report[19]===1;
  const fmt=(x,n=1)=>Number(x).toLocaleString('en-US',{maximumFractionDigits:n,minimumFractionDigits:n});
  const score=Number.isFinite(rescueScore)?rescueScore:0;
  const rows=[
    ['Velocity change',p.dv,dv,'m/s',2],
    ['Farthest altitude',p.apo,apo/1000,'km',3],
    ['Closest altitude',p.peri,peri/1000,'km',3],
    ['Remaining propellant',p.fuel,mass-DRY,'kg',2],
    ['Flight duration',p.time,evaluation,'s',1]
  ];
  if(Number.isFinite(p.miss)||Number.isFinite(report[40]))rows.push(['Target miss distance',Number.isFinite(p.miss)?p.miss/1000:NaN,Number.isFinite(report[40])?report[40]/1000:NaN,'km',3]);
  const table=document.createElement('table');table.className='prediction-table';
  table.innerHTML='<thead><tr><th>Metric</th><th>Predicted</th><th>Actual</th><th>Error</th></tr></thead>';
  const tbody=document.createElement('tbody');
  for(const [label,pred,act,unit,dec] of rows){
    const tr=document.createElement('tr');
    const err=Number.isFinite(pred)&&Number.isFinite(act)?act-pred:NaN;
    tr.innerHTML=`<th scope="row">${label}</th><td>${Number.isFinite(pred)?fmt(pred,dec):'—'} ${unit}</td><td>${Number.isFinite(act)?fmt(act,dec):'—'} ${unit}</td><td>${Number.isFinite(err)?(err>=0?'+':'')+fmt(err,dec):'—'} ${unit}</td>`;
    tbody.append(tr);
  }
  // Boil-off is integrated along the flown trajectory and signed; report it
  // from the executed thermal balance rather than claiming it is unmodeled.
  const cryo=lastBoilOff;
  const boil=document.createElement('tr');
  if(!cryo)boil.innerHTML='<th scope="row">Boil-off loss</th><td>—</td><td>Not available</td><td>—</td>';
  else if(cryo.inert)boil.innerHTML='<th scope="row">Boil-off loss</th><td>—</td><td>0.000 kg (storable propellants)</td><td>—</td>';
  else boil.innerHTML=`<th scope="row">Boil-off loss</th><td>—</td><td>${fmt(cryo.lost,3)} kg vented</td><td>—</td>`;
  tbody.append(boil);table.append(tbody);
  const cards=document.createElement('div');cards.className='debrief-grid';
  const entries=[['Mission score',`${fmt(score*100,1)}%`],['Orbit eccentricity',fmt(e,5)],['Orbital-plane tilt',`${fmt(inclination,2)}°`]];
  if(scenario==='102')entries.push(['Five-day node rotation',`${fmt(drift,3)}°`]);
  if(scenario==='103')entries.push(['Apoapsis radius',`${fmt((apo+R)/1000)} km`],['Target radius error',`${fmt(report[40]/1000)} km`]);
  if(Number.isFinite(report[32]))entries.push(['Lunar sphere-of-influence entry',`${fmt(report[32]/86400,3)} days`]);
  if(cryo&&!cryo.inert){
    entries.push(['Propellant boiled off',`${fmt(cryo.lost,3)} kg`],['Relief-valve openings',`${fmt(cryo.vents,0)}`]);
    if(cryo.coolerJoules>0)entries.push(['Cryocooler electricity',`${fmt(cryo.coolerJoules/1e6,2)} MJ`]);
  }
  const mask=lastDelegationMask;
  if(mask)entries.push(['Disciplines delegated',`${[0,1,2,3].filter(b=>mask&(1<<b)).length} of 4 · efficiency capped`]);
  if(impacted)entries.unshift(['Terminal event',`Impact at ${fmt(evaluation,1)} s`]);
  for(const [label,value] of entries){const item=document.createElement('div'),strong=document.createElement('strong'),span=document.createElement('span');strong.textContent=value;span.textContent=label;item.append(strong,span);cards.append(item);}
  const loss=document.createElement('p');loss.className='finite-loss';
  if(Number.isFinite(report[20]))loss.textContent=`Finite-burn loss isolated in the two-body model: commanded ${fmt(report[20],2)} m/s, steering loss ${fmt(report[21],2)} m/s, gravity loss ${fmt(report[22],2)} m/s, speed deficit ${fmt(report[23],2)} m/s over ${fmt(report[25],1)} s. Identity residual ${fmt(report[24],6)} m/s.`;
  const heading=document.createElement('h3');heading.textContent=scenario==='102'?'Exact Shapley attribution of node-rotation error':'Exact Shapley attribution of farthest-altitude error';
  const list=document.createElement('div');list.className='sensitivity-list';
  if(impacted||truncated||!Number.isFinite(report[6])){
    const row=document.createElement('div');row.textContent=impacted?`Flight ended at the reference surface${Number.isFinite(entry)?` after atmospheric entry at ${fmt(entry,1)} s`:''}. Late-epoch attribution shares are undefined.`:`Attribution shares are undefined${Number.isFinite(attrEnd)?` before the common epoch ${fmt(attrEnd,1)} s`:''}.`;list.append(row);
  }else{
    for(let i=0;i<4;i++){const row=document.createElement('div'),share=report[6+i],delta=report[10+i];row.textContent=`${SHAPLEY_NAMES[i]}: ${fmt(share*100,1)}% · signed ${scenario==='102'?fmt(delta*180/Math.PI,3)+'°':fmt(delta/1000,2)+' km'} · ${SHAPLEY_WHY[i]}`;list.append(row);}
    const note=document.createElement('div');note.className='attribution-note';note.textContent=`Signed contributions sum to the total error ${scenario==='102'?fmt(report[14]*180/Math.PI,3)+'°':fmt(report[14]/1000,2)+' km'}; efficiency residual ${fmt(report[17],9)}. Attribution epoch ${fmt(attrEnd,1)} s. Exact four-player Shapley uses 16 coalition propagations${lastComputeMs!==null?`; flight, loss isolation and attribution took ${fmt(lastComputeMs,1)} ms in this worker`:''}.`;list.append(note);
  }
  const elo=document.createElement('p');elo.className='elo-update';elo.id='elo-result';
  const signed=document.createElement('p');signed.className='profile-note';signed.id='recorder-status';
  const cert=document.createElement('p');cert.className='verify-verdict';cert.id='certification-status';
  cert.textContent='UNVERIFIED LOCAL / PRACTICE';
  const active=identity.profile();
  const activeKey=active.publicKey.map(b=>b.toString(16).padStart(2,'0')).join('');
  if(activeKey===flightContext.pubkey){
    const outcome=identity.recordFlight({contractId,score});
    elo.textContent=`Practice / pending verification — no local Astro-Elo was awarded. Client-recorded value ${fmt(outcome.after,1)} is not authoritative.`;
    if(lastCapture){
      try{
        const fields=recorder.stampOutcome(lastCapture,{ranked:flightContext.ticket?.mode===0,before:outcome.before,after:outcome.before});
        lastRecorder=recorder.sign(wasm,fields,flightContext.callsign);
        signed.textContent=`Flight recorder signed by ${lastRecorder.document.public_id}; digest ${lastRecorder.document.canonical_digest.slice(0,16)}…`;
      }catch(error){signed.textContent=`Recorder could not be signed: ${String(error.message||error)}`;}
    }else signed.textContent='No recorder was captured for this flight.';
  }else{
    lastRecorder=null;
    elo.textContent='The active license changed during flight; no record was signed or submitted.';
  }
  const actions=document.createElement('div');actions.className='button-row';
  const print=document.createElement('button');print.id='export-pdr';print.textContent='Print debrief (PDF)';print.title='Opens the browser print dialog; choose Save as PDF.';print.addEventListener('click',()=>window.print());actions.append(print);
  const exportButton=document.createElement('button');exportButton.id='export-fdr';exportButton.textContent='Export signed recorder (.odderon-fdr.json)';exportButton.disabled=!lastRecorder;
  exportButton.addEventListener('click',()=>{if(lastRecorder)recorder.download(`contract-${contractId}.odderon-fdr.json`,`${lastRecorder.json}\n`,'application/json');});
  const dossierButton=document.createElement('button');dossierButton.id='verify-flight';dossierButton.textContent='Verify / Replay Dossier';dossierButton.disabled=!lastCapture;
  dossierButton.addEventListener('click',()=>{
    // Show the very document being replayed beside its verdict, rather than
    // leaving an older pasted recorder in the verification input.
    if(lastRecorder)openVerify(lastRecorder.json);
  });
  actions.append(exportButton,dossierButton);
  $('debrief-content').replaceChildren(table,cards,loss,heading,list,cert,elo,signed,actions);
  annotateAcronyms($('debrief-content'));$('debrief').hidden=false;$('execute-status').textContent=impacted?'Flight ended in impact. The prediction remains locked.':'Flight complete. The prediction remains locked.';annotateAcronyms($('execute-status'));
  if(flightContext.ticket&&lastRecorder)void settleReceipt(lastRecorder,flightContext,version,cert,elo,actions);
}

async function settleReceipt(record,context,version,cert,elo,actions,receiptRevision=identity.profileRevision()){
  authorityPending++;
  cert.textContent='Submitting the signed flight for server replay…';
  try{
    const response=await authority.submit(record.payload,context.ticket);
    const r=response?.receipt;
    if(response?.accepted!==true||r?.attempt_num!==context.ticket.attempt_num||r?.mode!==context.ticket.mode
      ||response.fdr_hash!==r.fdr_hash||!verifyReceipt(wasm,r,record.document))throw Error('The authority returned an invalid or mismatched receipt.');
    const accepted=recorder.withReceipt(record,r);
    // Keep the receipt even if the debrief was closed while submission ran,
    // but never attach it to a different active identity.
    const saved=identity.recordReceipt(accepted.json,{expectedRevision:receiptRevision});
    // Receipts are immutable historical snapshots, and concurrent submissions
    // can settle out of order. Refresh the current rating after accepting one.
    if(saved)void syncProfile();
    if(version!==sequence)return;
    lastRecorder=accepted;
    cert.textContent=r.mode===0?'★ CERTIFIED SERVER-RANKED RECEIPT':'SERVER-VERIFIED PRACTICE';
    cert.className=`verify-verdict ${r.mode===0?'is-verified':''}`;
    elo.textContent=r.mode===0
      ? `Server-ranked Astro-Elo ${r.elo_before.toFixed(1)} → ${r.elo_after.toFixed(1)} (${(r.elo_after-r.elo_before)>=0?'+':''}${(r.elo_after-r.elo_before).toFixed(1)}); mission rating ${r.mission_elo_after.toFixed(1)}.`
      : `Server-verified practice — Astro-Elo unchanged at ${r.elo_after.toFixed(1)}.`;
    if(!saved)elo.textContent+=' Switch back to the signing license to sync this result.';
    actions.querySelector('#retry-submit')?.remove();
  }catch(error){
    if(version!==sequence)return;
    cert.textContent='UNVERIFIED LOCAL / PRACTICE';
    elo.textContent=`Authority did not certify this flight: ${String(error.message||error)}. No local rating was awarded.`;
    if(!actions.querySelector('#retry-submit')){
      const retry=document.createElement('button');retry.id='retry-submit';retry.textContent='Retry server submission';
      // The retry refers to the original submission, so retain its original
      // profile revision rather than making an old receipt look newly ordered.
      retry.addEventListener('click',()=>{retry.remove();void settleReceipt(record,context,version,cert,elo,actions,receiptRevision);});actions.append(retry);
    }
  }finally{authorityPending--;}
}
// Warning discriminants follow odd_app::causal::Warning declaration order.
const CAUSAL_EVIDENCE=[
  i=>`Thrust ${i.thrust.toFixed(0)} N versus standard weight ${(i.mass*G0).toFixed(0)} N gives ratio ${i.twr.toFixed(3)}.`,
  i=>`The closest sampled altitude is ${i.minPeri.toFixed(1)} km; the atmospheric reference is 120 km.`,
  i=>`Isolated gravity loss is ${(i.lossFraction*100).toFixed(1)}% of commanded speed change.`,
  i=>`The ideal rocket equation estimates ${i.needed.toFixed(1)} kg required.`,
  i=>`The ideal farthest altitude exceeds the modeled result by ${i.shortfallKm.toFixed(1)} km.`,
  ()=>'No underground flight or late-epoch attribution is reported.',
  i=>`The commanded burns sweep ${i.sweep.toFixed(1)}° in the isolated two-body model.`
];
function causalInputs(){
  const planned=nodes.reduce((sum,n)=>sum+Math.hypot(...n.dv),0);
  const needed=setup[2]*(-Math.expm1(-planned/(setup[5]*G0)));
  const minPeri=preview.length?Math.min(...preview.filter((_,i)=>i%15===0).map(p=>orbitMetrics(p.state)?.peri).filter(Number.isFinite)):Infinity;
  let sweep=0;const n=nodes[selected];
  if(n&&n.dv[0]>0&&Math.abs(n.dv[1])<1e-9&&Math.abs(n.dv[2])<1e-9&&preview.length){
    const pre=readState(wasm,Math.max(0,n.time-1e-6)),om=orbitMetrics(pre);
    if(om){const a=(om.apo+om.peri+2*R/1000)*500,ve=setup[5]*G0,duration=pre[6]*ve/setup[4]*(-Math.expm1(-n.dv[0]/ve));sweep=duration*Math.sqrt(MU/(a*a*a))*180/Math.PI;}
  }
  let lossFraction=0,shortfall=0,shortfallKm=0;
  if(lastReport&&Number.isFinite(lastReport[57]))sweep=lastReport[57]*180/Math.PI;
  if(lastReport&&Number.isFinite(lastReport[20])&&lastReport[20]>0)lossFraction=Math.max(0,lastReport[22]/lastReport[20]);
  if(lastReport&&preview.length){const ideal=orbitMetrics(preview[preview.length-1].state);if(ideal&&ideal.apo>0){shortfallKm=ideal.apo-lastReport[1]/1000;shortfall=shortfallKm/ideal.apo;}}
  return {twr:setup[4]/(setup[2]*G0),thrust:setup[4],mass:setup[2],needed,propellantDeficit:needed>setup[2]-DRY,minPeri,sweep,lossFraction,shortfall,shortfallKm,impacted:lastReport?.[30]===1};
}
// Engine warnings carry their own evidence, read from the solved design so
// the browser never restates a threshold Rust owns.
const ENGINE_EVIDENCE=[
  ()=>{const d=engineLab?.diagnostics();if(!d?.solution)return '';const s=d.solution;return ` Flow detaches at x/L ${s.separationX.toFixed(3)} with an estimated ${s.sideLoad.toFixed(3)} kN side load at ${(s.ambient/1000).toFixed(2)} kPa ambient.`;},
  ()=>{const d=engineLab?.diagnostics();if(!d?.solution)return '';const s=d.solution;return ` Wall ${s.wallTemperature.toFixed(0)} K against a 1350 K limit, at ${(s.heatFlux/1e6).toFixed(1)} MW/m² throat heat flux.`;},
  ()=>{const d=engineLab?.diagnostics();if(!d?.solution)return '';const s=d.solution;return ` O/F ${s.of.toFixed(2)} gives ${s.tc.toFixed(0)} K and ${(s.molar*1000).toFixed(2)} g/mol, for ${s.vacuumIsp.toFixed(1)} s vacuum specific impulse.`;}
];
// The engine registry is separate from the flight one: redrawing the flight
// panel would resize the content above the lab canvas and move it under the
// pointer mid-drag.
function renderEngineCausal(){
  const target=$('engine-causal');if(!target||!wasm)return;target.replaceChildren();
  const count=wasm.mission_engine_diagnose();
  for(let c=0;c<count;c++){
    const warning=wasm.mission_engine_causal_warning(c);
    const row=document.createElement('div');row.className='cause-chain';row.dataset.warning=String(warning);
    const n=wasm.mission_engine_causal_node_count(c);
    for(let j=0;j<n;j++){
      const kind=wasm.mission_engine_causal_node_kind(c,j),text=wasmString(wasm.mission_engine_causal_node_len(c,j),k=>wasm.mission_engine_causal_node_byte(c,j,k));
      if(j)row.append(document.createTextNode(' ← '));
      const span=document.createElement(kind===0?'strong':'span');span.textContent=text;row.append(span);
    }
    const evidence=document.createElement('span');evidence.className='cause-evidence';evidence.textContent=ENGINE_EVIDENCE[warning-7]?.()??'';row.append(evidence);target.append(row);
  }
  if(!target.childNodes.length)target.textContent='No engine rule triggered. The bell flows full, the throat stays inside its material limit, and the mixture is away from the temperature peak.';
  annotateAcronyms(target);
}
function updateCausal(){
  const target=$('causal-text');target.replaceChildren();$('split-kick').hidden=true;
  if(!wasm)return;
  const inputs=causalInputs();
  const count=wasm.mission_diagnose(inputs.twr,inputs.minPeri*1000,120000,inputs.lossFraction,inputs.propellantDeficit?1:0,inputs.shortfall,inputs.impacted?1:0,inputs.sweep);
  for(let c=0;c<count;c++){
    const warning=wasm.mission_causal_warning(c),demo=wasmString(wasm.mission_causal_demo_len(c),j=>wasm.mission_causal_demo_byte(c,j));
    const row=document.createElement('div');row.className='cause-chain';row.dataset.demo=demo;row.dataset.warning=String(warning);
    const n=wasm.mission_causal_node_count(c);
    for(let j=0;j<n;j++){
      const kind=wasm.mission_causal_node_kind(c,j),text=wasmString(wasm.mission_causal_node_len(c,j),k=>wasm.mission_causal_node_byte(c,j,k));
      if(kind===3){const btn=document.createElement('button');btn.className='demo quiet';btn.dataset.demo=demo;btn.textContent='Show me';btn.setAttribute('aria-label',text);row.append(btn);continue;}
      if(j)row.append(document.createTextNode(' ← '));
      const span=document.createElement(kind===0?'strong':'span');span.textContent=text;row.append(span);
    }
    const evidence=document.createElement('span');evidence.className='cause-evidence';evidence.textContent=' '+CAUSAL_EVIDENCE[warning](inputs);row.append(evidence);target.append(row);
    if((warning===4||warning===6)&&nodes[selected]?.dv[0]>0&&!locked)$('split-kick').hidden=false;
  }
  if(scenario==='103'&&nodes.length===1&&nodes[0].dv[0]>2500&&!locked)$('split-kick').hidden=false;
  if(scenario==='103'){$('split-kick').disabled=!!locked||$('attempt-mode').value==='ranked';$('split-kick').title=$('attempt-mode').value==='ranked'?'Numerical targeting runs the flight model and is available only in practice.':'';}
  if(scenario==='103'&&lastBplaneInfo){$('target-bplane').hidden=false;$('target-bplane').disabled=!!locked;}else{$('target-bplane').hidden=true;}
  if(scenario==='103'&&lastLunarInfo){const row=document.createElement('div');row.className='cause-chain';row.textContent=`Lunar sphere-of-influence entry: ideal patched preview enters at ${Math.round(lastLunarInfo[0])} s. The ${Math.round(lastLunarInfo[1]/1000)} km boundary is a preview aid, not a gravity discontinuity; the gold path keeps continuous third-body gravity.`;target.append(row);}
  if(scenario==='103'&&lastBplaneInfo){const row=document.createElement('div');row.className='cause-chain';row.textContent=`B-plane at entry: miss ${lastBplaneInfo[0].toFixed(0)} m, B·T ${lastBplaneInfo[1].toFixed(0)} m (along-track), B·R ${lastBplaneInfo[2].toFixed(0)} m (cross-track). Target a B-plane point to adjust the maneuver nodes.`;target.append(row);}
  if(!target.childNodes.length)target.textContent='No rule currently triggered. This is a limited check, not a safety certification.';
  annotateAcronyms(target);
}
$('split-kick').addEventListener('click',()=>{
  if(scenario==='103'){
    if($('attempt-mode').value==='ranked'){$('plan-status').textContent='Numerical targeting is available in practice only.';return;}
    practiceSimulated.add(contractId);updateAttemptMode();
    const total=nodes.reduce((a,n)=>a+Math.hypot(...n.dv),0)||3240;
    const first=nodes[0]?.time||1600;
    cancelFlight();const version=++sequence;worker=new Worker('./mission-worker.js',{type:'module'});const current=worker;
    $('plan-status').textContent='Targeting numerical periapses in the background…';$('split-kick').disabled=true;
    current.onmessage=({data})=>{messages++;if(worker!==current||version!==sequence)return;current.terminate();worker=null;
      if(data.error){$('plan-status').textContent=data.error;$('split-kick').disabled=!!locked;return;}
      nodes=data.nodes;selected=nodes.length-1;updatePreview();$('plan-status').textContent='Three equal kicks placed at numerically propagated periapses. Lock a prediction and fly to see the modeled path.';annotateAcronyms($('plan-status'));
    };
    current.onerror=()=>{if(worker!==current)return;worker.terminate();worker=null;$('plan-status').textContent='Targeting worker failed.';$('split-kick').disabled=!!locked;};
    messages++;current.postMessage({action:'split',kicks:3,total,first,engine:appliedEngine?{...appliedEngine}:null,assembly:assemblyDesign?{...assemblyDesign}:null});return;
  }
  const original=nodes[selected];if(!original||original.dv[0]<=0)return;
  const old=nodes.map(n=>({time:n.time,dv:[...n.dv]}));
  original.dv[0]*=0.5;
  if(!configure(wasm,nodes)){nodes=old;updatePreview();$('plan-status').textContent=refusal(wasm);$('plan-status').classList.add('failure');return;}
  const state=readState(wasm,original.time),radius=Math.hypot(...state.slice(0,3)),speed2=state.slice(3,6).reduce((a,b)=>a+b*b,0),energy=speed2/2-MU/radius;
  if(!(energy<0)){nodes=old;updatePreview();$('plan-status').textContent='A second pass cannot be estimated for an unbound intermediate orbit.';return;}
  const a=-MU/(2*energy),period=2*Math.PI*Math.sqrt(a*a*a/MU),second=original.time+period;
  if(!Number.isFinite(second)||second>=DURATION-1){nodes=old;updatePreview();$('plan-status').textContent='The next ideal periapsis falls outside the current mission duration.';return;}
  nodes.push({time:second,dv:[original.dv[0],0,0]});selected=nodes.length-1;updatePreview();
  $('plan-status').textContent='Second push placed at the next ideal periapsis. The finite-burn trajectory may reach periapsis at a different time; check the gold path.';
});
$('target-bplane').addEventListener('click',()=>{
  if(scenario!=='103'||!lastBplaneInfo||locked)return;
  const bt=prompt('Target B·T (along-track, m). Current: '+lastBplaneInfo[1].toFixed(0),lastBplaneInfo[1].toFixed(0));
  if(bt===null)return;
  const br=prompt('Target B·R (cross-track, m). Current: '+lastBplaneInfo[2].toFixed(0),lastBplaneInfo[2].toFixed(0));
  if(br===null)return;
  const btN=Number(bt),brN=Number(br);
  if(!Number.isFinite(btN)||!Number.isFinite(brN)){$('plan-status').textContent='Invalid B-plane target.';return;}
  practiceSimulated.add(contractId);updateAttemptMode();
  const ok=wasm.mission_target_bplane(btN,brN);
  if(!ok){$('plan-status').textContent='B-plane target could not be reached (insufficient sensitivity).';return;}
  // Rust owns the targeting solve. Pull its adjusted commands back before
  // updatePreview reconfigures the bridge; otherwise stale page nodes erase
  // the solution immediately.
  try{
    nodes=readMissionNodes(wasm);
    selected=Math.min(Math.max(selected,0),nodes.length-1);
  }catch(error){
    $('plan-status').textContent=String(error.message||error);
    return;
  }
  updatePreview();
  $('plan-status').textContent='Maneuver nodes adjusted for B-plane target. Recompile and fly to verify.';
});
function toCanvas(point){const rect=canvas.getBoundingClientRect(),span=2.45*TARGET;return [rect.width/2+point[0]*rect.height/span,rect.height/2-(point[1]*plane[0]+point[2]*plane[1])*rect.height/span];}
function draw(){
  const rect=canvas.getBoundingClientRect();if(!rect.width||!rect.height)return;const dpr=Math.min(devicePixelRatio||1,2);canvas.width=Math.round(rect.width*dpr);canvas.height=Math.round(rect.height*dpr);ctx.setTransform(dpr,0,0,dpr,0,0);ctx.clearRect(0,0,rect.width,rect.height);draws++;
  const center=toCanvas([0,0,0]),earth=R*rect.height/(2.45*TARGET);
  const gradient=ctx.createRadialGradient(center[0]-earth*.25,center[1]-earth*.3,earth*.04,center[0],center[1],earth);gradient.addColorStop(0,'#607f71');gradient.addColorStop(.75,'#254b4a');gradient.addColorStop(1,'#102b33');ctx.beginPath();ctx.arc(...center,earth,0,Math.PI*2);ctx.fillStyle=gradient;ctx.fill();ctx.strokeStyle='#6e9e8c';ctx.lineWidth=1;ctx.stroke();
  const line=(pts,color,dashed)=>{if(!pts.length)return;ctx.beginPath();ctx.setLineDash(dashed?[4,5]:[]);ctx.strokeStyle=color;ctx.lineWidth=2;pts.forEach((p,i)=>{const xy=toCanvas(p);if(i)ctx.lineTo(...xy);else ctx.moveTo(...xy);});ctx.stroke();ctx.setLineDash([]);};
  if(preview.length)line(preview.map(p=>p.state),'#70d7dc',true);
  if(scenario==='103'&&lunarPreview.length)line(lunarPreview,'#b9a7e8',true);
  if(actual)line(actual,'#e2bc79',false);
  nodes.forEach((node,i)=>{const state=readState(wasm,node.time);if(state.some(x=>!Number.isFinite(x)))return;const p=toCanvas(state);ctx.fillStyle=i===selected?'#f0ddac':'#a1c2b2';ctx.beginPath();ctx.arc(...p,5,0,Math.PI*2);ctx.fill();ctx.fillStyle='#e4eada';ctx.font='11px system-ui';ctx.fillText(String(i+1),p[0]+9,p[1]-8);if(i===selected){for(const h of handles(p,state)){ctx.strokeStyle=h.color;ctx.beginPath();ctx.moveTo(...p);ctx.lineTo(...h.end);ctx.stroke();ctx.fillStyle=h.color;ctx.beginPath();ctx.arc(...h.end,6,0,Math.PI*2);ctx.fill();ctx.fillText(h.label,h.end[0]+7,h.end[1]-6);}}});
}
function handles(p,state){const v=[state[3],-(state[4]*plane[0]+state[5]*plane[1])],m=Math.hypot(...v)||1,t=v.map(x=>x/m),n=[-t[1],t[0]];return [{key:'t',dir:t,color:'#80d9cb',label:'Along'},{key:'n',dir:n,color:'#e1bc76',label:'Sideways'},{key:'w',dir:[.707,-.707],color:'#bdade0',label:'Plane tilt'}].map(h=>({...h,end:[p[0]+h.dir[0]*42,p[1]+h.dir[1]*42]}));}
function mousePoint(e){const r=canvas.getBoundingClientRect();return [e.clientX-r.left,e.clientY-r.top];}
function nearestPath(p){let best=null;for(const item of preview){const d=distance(p,toCanvas(item.state));if(!best||d<best.d)best={d,time:item.time};}return best;}
function setLens(key){$('lens-text').textContent=lens[key]||lens.time;annotateAcronyms($('lens-text'));}
function startLens(key,term){
  const wanted=term&&GLOSSARY[term]?term:null;
  // Re-entering the projection that is already showing must be a no-op.
  // The lens writes its own text and projection nodes, so without this guard
  // hovering a term rendered inside #lens-projection destroys the very node
  // under the pointer, which re-fires mouseover forever and steals focus.
  // It also stops pointermove from resetting lensPhase on every mouse sample,
  // which froze the handle animation while the cursor was moving.
  if(activeLens===key&&activeTerm===wanted)return;
  activeLens=key;activeTerm=wanted;lensPhase=0;setLens(key);$('lens-close').hidden=false;
  const proj=$('lens-projection');
  if(wanted){proj.hidden=false;proj.textContent=`${wanted} — ${GLOSSARY[wanted].expansion}. ${GLOSSARY[wanted].plain}`;annotateAcronyms(proj);}else proj.hidden=true;
  lensGovernor.resume();lensGovernor.invalidate(false);
}
function stopLens(){activeLens=null;activeTerm=null;$('lens-projection').hidden=true;$('lens-close').hidden=true;lensGovernor.suspend();}
// Moving the pointer or focus into the lens panel must not close it: the
// projection text and the Close button live inside that panel, so stopping on
// every leave hid the button while the cursor was travelling toward it and
// made it unclickable. A null relatedTarget (leaving the window) still closes.
const intoLens=e=>!!e.relatedTarget?.closest?.('.lens');
const leaveLens=e=>{if(!intoLens(e))stopLens();};
$('lens-close').addEventListener('click',stopLens);
function drawLens(){
  const w=lensCanvas.width,h=lensCanvas.height;lensCtx.clearRect(0,0,w,h);lensPhase+=0.05;lensDraws++;
  const cx=w/2,cy=h/2,phase=Math.sin(lensPhase),key=activeLens||'t';
  lensCtx.strokeStyle='#3d5a4e';lensCtx.lineWidth=1;lensCtx.strokeRect(0.5,0.5,w-1,h-1);
  const arrow=(x1,y1,x2,y2,color)=>{lensCtx.strokeStyle=color;lensCtx.fillStyle=color;lensCtx.lineWidth=2;lensCtx.beginPath();lensCtx.moveTo(x1,y1);lensCtx.lineTo(x2,y2);lensCtx.stroke();const a=Math.atan2(y2-y1,x2-x1);lensCtx.beginPath();lensCtx.moveTo(x2,y2);lensCtx.lineTo(x2-8*Math.cos(a-0.4),y2-8*Math.sin(a-0.4));lensCtx.lineTo(x2-8*Math.cos(a+0.4),y2-8*Math.sin(a+0.4));lensCtx.closePath();lensCtx.fill();};
  lensCtx.fillStyle='#c7d3c4';lensCtx.font='10px system-ui';
  if(key==='t'||key==='time'){
    const breathe=1+0.12*phase;lensCtx.strokeStyle='#70d7dc';lensCtx.setLineDash([4,4]);lensCtx.beginPath();lensCtx.ellipse(cx-20,cy,70,40,0,0,Math.PI*2);lensCtx.stroke();lensCtx.setLineDash([]);
    lensCtx.strokeStyle='#e2bc79';lensCtx.beginPath();lensCtx.ellipse(cx-20,cy,70*breathe,40,0,0,Math.PI*2);lensCtx.stroke();
    arrow(cx+50,cy,cx+50+30*phase,cy,'#80d9cb');lensCtx.fillText('Kinetic energy gain / loss',10,14);lensCtx.fillText('Opposite apsis breathing',10,h-8);
    const li=lensNodeIndex();if(li>=0&&wasm){const ke=wasm.mission_lens(li,0,0);if(Number.isFinite(ke))lensCtx.fillText(`Δ kinetic energy ${(ke/1000).toFixed(1)} kJ`,10,28);}
  }else if(key==='w'||key==='raan'||key==='j2'){
    const tilt=0.35*phase;lensCtx.save();lensCtx.translate(cx,cy);lensCtx.rotate(tilt);lensCtx.strokeStyle='#bdade0';lensCtx.beginPath();lensCtx.ellipse(0,0,80,30,0,0,Math.PI*2);lensCtx.stroke();lensCtx.restore();
    lensCtx.strokeStyle='#3d5a4e';lensCtx.beginPath();lensCtx.moveTo(cx-90,cy);lensCtx.lineTo(cx+90,cy);lensCtx.stroke();
    arrow(cx,cy,cx+40*phase,cy-30,'#e1bc76');lensCtx.fillText('Torque = radius × force',10,14);lensCtx.fillText('Plane tilts about the line of nodes',10,h-8);
    const li=lensNodeIndex();if(li>=0&&wasm){const tiltRad=wasm.mission_lens(li,1,3);if(Number.isFinite(tiltRad))lensCtx.fillText(`Plane tilt ${(tiltRad*180/Math.PI).toFixed(2)}°`,10,28);}
  }else if(key==='n'||key==='aop'){
    const rot=0.6*phase;lensCtx.strokeStyle='#3d5a4e';lensCtx.beginPath();lensCtx.ellipse(cx,cy,80,40,0,0,Math.PI*2);lensCtx.stroke();
    arrow(cx,cy,cx+60*Math.cos(rot),cy+30*Math.sin(rot),'#e1bc76');lensCtx.fillText('Eccentricity vector rotates',10,14);
    const li=lensNodeIndex();if(li>=0&&wasm){const rotRad=wasm.mission_lens(li,2,6);if(Number.isFinite(rotRad))lensCtx.fillText(`Rotation ${(rotRad*180/Math.PI).toFixed(2)}°`,10,28);}
  }else if(key==='twr'){
    const ratio=setup[4]/(setup[2]*G0);const thrustH=50*Math.min(ratio,2),weightH=50;
    arrow(cx-40,cy+30,cx-40,cy+30-thrustH,'#80d9cb');arrow(cx+40,cy-30,cx+40,cy-30+weightH,'#edbc98');
    lensCtx.fillText(`Thrust ${setup[4].toFixed(0)} N`,cx-80,cy+44);lensCtx.fillText(`Weight ${(setup[2]*G0).toFixed(0)} N`,cx+10,cy-36);lensCtx.fillText(ratio<1?'Cannot lift':'Can lift',10,14);
  }else if(key==='isp'){
    for(let i=0;i<6;i++){const x=cx-60+((lensPhase*20+i*20)%120);lensCtx.fillStyle='#80d9cb';lensCtx.beginPath();lensCtx.arc(x,cy,3,0,Math.PI*2);lensCtx.fill();}
    arrow(cx+60,cy,cx+90,cy,'#e1bc76');lensCtx.fillText('Propellant mass flow produces thrust',10,14);
  }else if(key==='soi'){
    lensCtx.strokeStyle='#b9a7e8';lensCtx.setLineDash([4,4]);lensCtx.beginPath();lensCtx.arc(cx,cy,50+5*phase,0,Math.PI*2);lensCtx.stroke();lensCtx.setLineDash([]);lensCtx.fillStyle='#607f71';lensCtx.beginPath();lensCtx.arc(cx,cy,10,0,Math.PI*2);lensCtx.fill();lensCtx.fillText('Patched-conic preview boundary',10,14);
  }else if(key==='srp'){
    for(let i=0;i<5;i++){const x=20+((lensPhase*30+i*30)%120);arrow(x,30+i*12,x+20,30+i*12,'#e2bc79');}lensCtx.fillText('Sunlight momentum pushes the spacecraft',10,14);
  }else if(key==='tnw'){
    arrow(cx,cy,cx+60,cy,'#80d9cb');arrow(cx,cy,cx,cy-45,'#e1bc76');arrow(cx,cy,cx-35,cy+35,'#bdade0');lensCtx.fillText('Along / sideways / out of plane',10,14);
  }else if(key==='leo'){
    lensCtx.fillStyle='#607f71';lensCtx.beginPath();lensCtx.arc(cx,cy,30,0,Math.PI*2);lensCtx.fill();lensCtx.strokeStyle='#70d7dc';lensCtx.beginPath();lensCtx.arc(cx,cy,42+2*phase,0,Math.PI*2);lensCtx.stroke();lensCtx.fillText('Low Earth parking orbit',10,14);
  }else if(key==='mli'){
    const layers=6;for(let i=0;i<layers;i++){const x=cx-50+i*14;lensCtx.strokeStyle=i===0?'#edbc98':'#3d5a4e';lensCtx.beginPath();lensCtx.moveTo(x,cy-40);lensCtx.lineTo(x,cy+40);lensCtx.stroke();}
    arrow(cx-80,cy,cx-54,cy,'#e1bc76');arrow(cx+34,cy,cx+44,cy,'#80d9cb');
    lensCtx.fillStyle='#607f71';lensCtx.fillRect(cx+48,cy-30,26,60);
    lensCtx.fillText('Outer skin runs hot; only a fraction reaches the liquid',10,14);
    lensCtx.fillText('More layers help less once the struts dominate',10,h-8);
  }else if(key==='cop'){
    const lift=26,draw=70;
    lensCtx.fillStyle='#80d9cb';lensCtx.fillRect(cx-60,cy-lift/2,24,lift);
    lensCtx.fillStyle='#edbc98';lensCtx.fillRect(cx+36,cy-draw/2,24,draw);
    arrow(cx-30,cy,cx+30,cy,'#e1bc76');
    lensCtx.fillText('Heat lifted at the cold tank',cx-96,cy+lift/2+16);
    lensCtx.fillText('Electricity drawn',cx+22,cy+draw/2+16);
    lensCtx.fillText('Carnot caps the ratio: cold / (hot − cold)',10,14);
  }else if(key==='gnc'){
    lensCtx.strokeStyle='#3d5a4e';lensCtx.strokeRect(cx-24,cy-18,48,36);
    arrow(cx,cy,cx+52*Math.cos(phase),cy+34*Math.sin(phase),'#bdade0');
    lensCtx.fillText('Sensors, computer and actuators hold attitude',10,14);
    lensCtx.fillText('They draw from the same bus as the cooler',10,h-8);
  }else if(key==='boil-off'){
    const level=34+10*Math.sin(phase);
    lensCtx.strokeStyle='#3d5a4e';lensCtx.strokeRect(cx-30,cy-34,60,68);
    lensCtx.fillStyle='#70d7dc';lensCtx.fillRect(cx-29,cy+34-level,58,level);
    arrow(cx-70,cy-20,cx-34,cy-20,'#e1bc76');
    for(let i=0;i<3;i++){const y=cy-40-((lensPhase*18+i*12)%30);lensCtx.fillStyle='#edbc98';lensCtx.beginPath();lensCtx.arc(cx,y,2.5,0,Math.PI*2);lensCtx.fill();}
    lensCtx.fillText('Heat in vaporizes liquid; the relief valve vents it',10,14);
    lensCtx.fillText('Loss is reported, not fed back into orbital mass',10,h-8);
  }else if(key==='dag'){
    const labels=['Symptom','Mechanism','Cause','Demo'];for(let i=0;i<4;i++){const x=15+i*65;lensCtx.strokeStyle='#8ea98d';lensCtx.strokeRect(x,cy-12,52,24);lensCtx.fillStyle='#c7d3c4';lensCtx.fillText(labels[i],x+4,cy+4);if(i<3)arrow(x+52,cy,x+65,cy,'#e2bc79');}lensCtx.fillText('Deterministic cause chain',10,14);
  }else if(key==='pdr'){
    lensCtx.strokeStyle='#8ea98d';lensCtx.strokeRect(cx-70,cy-35,140,70);lensCtx.beginPath();lensCtx.moveTo(cx-70,cy-10);lensCtx.lineTo(cx+70,cy-10);lensCtx.moveTo(cx,cy-35);lensCtx.lineTo(cx,cy+35);lensCtx.stroke();lensCtx.fillStyle='#c7d3c4';lensCtx.fillText('Predicted',cx-60,cy-18);lensCtx.fillText('Actual',cx+10,cy-18);lensCtx.fillText('Locked baseline vs flight',10,14);
  }else if(key==='impact'){
    lensCtx.fillStyle='#607f71';lensCtx.beginPath();lensCtx.arc(cx,cy+20,35,0,Math.PI*2);lensCtx.fill();
    lensCtx.strokeStyle='#edbc98';lensCtx.beginPath();lensCtx.moveTo(cx-80,cy-40);lensCtx.quadraticCurveTo(cx,cy+10,cx+20,cy+30);lensCtx.stroke();lensCtx.fillStyle='#edbc98';lensCtx.beginPath();lensCtx.arc(cx+20,cy+30,4,0,Math.PI*2);lensCtx.fill();lensCtx.fillText('Propagation stops at the reference surface',10,14);
  }else if(key==='propellant'){
    const i=causalInputs(),available=Math.max(0,i.mass-DRY),needed=Number.isFinite(i.needed)?i.needed:0,scale=140/Math.max(needed,available,1);
    lensCtx.strokeStyle='#8ea98d';lensCtx.strokeRect(cx-70,cy-30,140,18);lensCtx.fillStyle='#80d9cb';lensCtx.fillRect(cx-70,cy-30,available*scale,18);
    lensCtx.strokeStyle='#edbc98';lensCtx.strokeRect(cx-70,cy,140,18);lensCtx.fillStyle='#edbc98';lensCtx.fillRect(cx-70,cy,Math.min(140,needed*scale),18);
    lensCtx.fillStyle='#c7d3c4';lensCtx.fillText(`Tank ${available.toFixed(0)} kg`,cx-66,cy-34);lensCtx.fillText(`Required ${needed.toFixed(0)} kg`,cx-66,cy+32);lensCtx.fillText('Rocket-equation propellant budget',10,14);
  }else if(key==='periapsis'){
    const i=causalInputs(),peri=Number.isFinite(i.minPeri)?i.minPeri:200;
    lensCtx.fillStyle='#607f71';lensCtx.beginPath();lensCtx.arc(cx-30,cy,25,0,Math.PI*2);lensCtx.fill();lensCtx.strokeStyle='#edbc98';lensCtx.setLineDash([3,3]);lensCtx.beginPath();lensCtx.arc(cx-30,cy,32,0,Math.PI*2);lensCtx.stroke();lensCtx.setLineDash([]);
    lensCtx.strokeStyle=peri<120?'#edbc98':'#80d9cb';lensCtx.beginPath();lensCtx.ellipse(cx+10,cy,70,35,0,0,Math.PI*2);lensCtx.stroke();lensCtx.fillStyle='#c7d3c4';lensCtx.fillText(`Periapsis ${peri.toFixed(0)} km vs 120 km atmosphere`,10,14);
  }else if(key==='apoapsis'){
    const breathe=1+0.1*phase;lensCtx.strokeStyle='#70d7dc';lensCtx.setLineDash([4,4]);lensCtx.beginPath();lensCtx.ellipse(cx-20,cy,70,35,0,0,Math.PI*2);lensCtx.stroke();lensCtx.setLineDash([]);
    lensCtx.strokeStyle='#e2bc79';lensCtx.beginPath();lensCtx.ellipse(cx-20,cy,70*0.8*breathe,35,0,0,Math.PI*2);lensCtx.stroke();lensCtx.fillStyle='#c7d3c4';lensCtx.fillText('Ideal apoapsis vs finite-burn apoapsis',10,14);
  }else{
    const arc=0.5+0.3*phase;lensCtx.strokeStyle='#e2bc79';lensCtx.beginPath();lensCtx.arc(cx,cy+30,60,Math.PI+arc,Math.PI*2-arc);lensCtx.stroke();lensCtx.strokeStyle='#70d7dc';lensCtx.setLineDash([4,4]);lensCtx.beginPath();lensCtx.arc(cx,cy+30,60,Math.PI,Math.PI*2);lensCtx.stroke();lensCtx.setLineDash([]);lensCtx.fillText('Finite burn spreads the push; split near periapsis',10,14);
  }
  if(activeLens)lensGovernor.invalidate(false);
}
for(const key of Object.keys(fields)){
  fields[key].addEventListener('focus',()=>startLens(key));fields[key].addEventListener('mouseenter',()=>startLens(key));fields[key].addEventListener('blur',leaveLens);fields[key].addEventListener('mouseleave',leaveLens);
  fields[key].addEventListener('input',()=>{if(locked||selected<0)return;const value=Number(fields[key].value);if(!Number.isFinite(value))return;if(key==='time')nodes[selected].time=clampNodeTime(value);else nodes[selected].dv[{t:0,n:1,w:2}[key]]=value;delete nodes[selected].guidance;updatePreview();});
}
document.addEventListener('mouseover',e=>{const t=e.target.closest?.('.term');if(t)startLens(GLOSSARY[t.dataset.term]?.key||'t',t.dataset.term);});
document.addEventListener('mouseout',e=>{const t=e.target.closest?.('.term,button.demo');if(t&&!t.contains(e.relatedTarget)&&!intoLens(e))stopLens();});
document.addEventListener('focusin',e=>{const t=e.target.closest?.('.term');if(t)startLens(GLOSSARY[t.dataset.term]?.key||'t',t.dataset.term);});
document.addEventListener('focusout',e=>{const t=e.target.closest?.('.term,button.demo');if(t&&!t.contains(e.relatedTarget)&&!intoLens(e))stopLens();});
document.addEventListener('click',e=>{const b=e.target.closest?.('button.demo');if(b)startLens(b.dataset.demo);});
document.addEventListener('keydown',e=>{if(e.key==='Escape')stopLens();});
$('lens-canvas').addEventListener('pointerleave',e=>{if(!drag&&!intoLens(e))stopLens();});
// Selection changes which node the sweep estimate and the split-kick offer
// describe, so the causal panel must be rebuilt even though the physics is
// unchanged. No re-propagation and no animation frame is scheduled here.
$('node-select').addEventListener('change',e=>{selected=Number(e.target.value);syncFields();updateCausal();draw();});
$('add-node').addEventListener('click',()=>{nodes.push({time:200,dv:[10,0,0]});selected=nodes.length-1;updatePreview();});
$('remove-node').addEventListener('click',()=>{if(selected<0)return;nodes.splice(selected,1);selected=Math.min(selected,nodes.length-1);updatePreview();});
$('example').addEventListener('click',()=>{
  if(scenario==='103'){nodes=[{time:1600,dv:[3240,0,0]}];selected=0;updatePreview();prefill();return;}
  const a=(INITIAL+TARGET)/2,coast=Math.PI*Math.sqrt(a*a*a/MU);
  if(scenario==='102'){const vAtFar=Math.sqrt(MU*(2/TARGET-1/a)),circular=Math.sqrt(MU/TARGET),tilt=2.6*Math.PI/180;nodes=[{time:coast,dv:[circular*Math.cos(tilt)-vAtFar,0,-circular*Math.sin(tilt)]}];}
  else{const v1=Math.sqrt(MU/INITIAL),v2=Math.sqrt(MU/TARGET);nodes=[{time:200,dv:[v1*(Math.sqrt(TARGET/a)-1),0,0]},{time:200+coast,dv:[v2*(1-Math.sqrt(INITIAL/a)),0,0]}];}
  selected=0;updatePreview();prefill();
});
// Only the authority decides whether a commit is attempt 1. We also avoid
// presenting a ranked launch in this page session after numerical practice
// on the same contract; this UX guard is not a security boundary.
function updateAttemptMode(reset=false){
  const select=$('attempt-mode');if(!select)return;
  const available=authority.configured(), practiced=practiceSimulated.has(contractId);
  select.querySelector('option[value="ranked"]').disabled=!available||practiced;
  if(!available||practiced)select.value='practice';else if(reset&&!locked)select.value='ranked';
  select.disabled=!!locked;
  const note=$('attempt-mode-note');
  if(note)note.textContent=!available?'No trusted server key is configured. Local practice and replay are available, but no rating or certification is awarded.'
    :practiced?'Numerical practice was used on this contract in this page session. Ranked is disabled here; offline practice does not consume a server attempt.'
    :'Ranked launch commits the vehicle, maneuver plan and prediction before the modeled flight. The server forces later attempts to verified practice. Numerical tug targeting is practice-only.';
  const split=$('split-kick');
  if(split){split.disabled=!!locked||(scenario==='103'&&select.value==='ranked');split.title=scenario==='103'&&select.value==='ranked'?'Numerical targeting runs the flight model and is available only in practice.':'';}
}
identity.onChange(()=>updateAttemptMode());
$('attempt-mode').addEventListener('change',()=>{updateAttemptMode();});
$('mission-select').addEventListener('change',()=>{
  if(locked)return;scenario=$('mission-select').value;contractId=Number(scenario);updateAttemptMode(true);cancelFlight();sequence++;nodes=[];selected=-1;actual=null;lastReport=null;lastLunarInfo=null;lunarPreview=[];
  if(scenario==='102'){
    INITIAL=R+250000;TARGET=R+800000;DURATION=432000;plane=[Math.cos(96*Math.PI/180),Math.sin(96*Math.PI/180)];const a=(INITIAL+TARGET)/2,v=Math.sqrt(MU*(2/INITIAL-1/a));setup=[820497600,DURATION,1200,DRY,2400,310,INITIAL,0,0,0,v*plane[0],v*plane[1]];
    $('mission-rank').textContent='CONTRACT 102 / FLIGHT DYNAMICS OFFICER · 1250';$('mission-title').textContent='Follow the sunlight';$('mission-description').textContent='Circularize at 800 km and tilt the orbit so its crossing of the equator turns by about 4.93° in five days. Engine and spacecraft mass use the rescue assumptions.';$('example').textContent='Show circularization example';
  }else if(scenario==='103'){
    INITIAL=R+200000;TARGET=380000000;DURATION=600000;DRY=5200;plane=[1,0];setup=[820497600,DURATION,18000,DRY,18000,450,INITIAL,0,0,0,Math.sqrt(MU/INITIAL),0];
    $('mission-rank').textContent='CONTRACT 103 / SENIOR FLIGHT DYNAMICS OFFICER · 1650';$('mission-title').textContent='The perigee-kick trap';$('mission-description').textContent='Raise a 200 km tug orbit toward a 380,000 km apoapsis radius (±15,000 km) using two or three perigee kicks. The single-burn example demonstrates the trap; the splitter targets actual numerical periapses. The violet dashed path is a patched lunar preview, not the continuous-gravity flight.';$('example').textContent='Show single-burn trap';
  }else{
    INITIAL=R+260000;TARGET=R+550000;DURATION=10800;DRY=1020;plane=[1,0];setup=[820497600,DURATION,1200,DRY,2400,310,INITIAL,0,0,0,Math.sqrt(MU/INITIAL),0];
    $('mission-rank').textContent='CONTRACT 101 / CADET · 800';$('mission-title').textContent='Orbit rescue';$('mission-description').textContent='Raise a 260 km circular orbit to 550 km, then make it circular within three hours. The dotted cyan path is your instantaneous-push preview; the gold path includes finite burns and modeled forces.';$('example').textContent='Show two-push example';
  }
  if(engineLab){appliedEngine=null;assemblyDesign=null;lastBoilOff=null;lastDelegationMask=0;wasm.mission_contract(contractId);engineLab.loadBaseline();engineLab.setLocked();if(assembly){assembly.sync();assembly.setLocked();}DRY=wasm.mission_parameter(3);setup=Array.from({length:12},(_,i)=>wasm.mission_parameter(i));}
  fields.time.min=String(nodeTimeRange()[0]);fields.time.max=String(nodeTimeRange()[1]);$('debrief').hidden=true;for(const id of ['predict-dv','predict-apo','predict-peri','predict-fuel','predict-time','predict-miss'])$(id).value='';annotateAcronyms($('mission-description'));annotateAcronyms($('mission-rank'));updatePreview();
});
$('prefill').addEventListener('click',prefill);
$('execute').addEventListener('click',()=>{
  const ids=['predict-dv','predict-apo','predict-peri','predict-fuel','predict-time'];
  const vals=ids.map(id=>Number($(id).value));
  if(ids.some(id=>$(id).value.trim()==='')||vals.some(x=>!Number.isFinite(x))||vals[0]<0||vals[3]<0||vals[4]<=0){$('execute-status').textContent='Enter a valid prediction for each field.';return;}
  const miss=$('predict-miss').value.trim()===''?NaN:Number($('predict-miss').value)*1000;
  if($('predict-miss').value.trim()!==''&&(!Number.isFinite(miss)||miss<0)){$('execute-status').textContent='Target miss distance must be non-negative.';return;}
  locked=Object.freeze({prediction:Object.freeze({dv:vals[0],apo:vals[1],peri:vals[2],fuel:vals[3],time:vals[4],miss})});
  for(const id of ['predict-dv','predict-apo','predict-peri','predict-fuel','predict-time','predict-miss','prefill','execute'])$(id).disabled=true;
  $('unlock').hidden=false;$('debrief').hidden=true;engineLab?.setLocked();assembly?.setLocked();syncFields();updateAttemptMode();
  void launchLocked();
});
async function launchLocked(){
  cancelFlight();const version=++sequence;
  lastReport=null;lastCapture=null;lastRecorder=null;
  const profile=identity.profile(),pubkey=profile.publicKey.map(b=>b.toString(16).padStart(2,'0')).join('');
  const context={pubkey,callsign:profile.callsign,ticket:null};
  if($('attempt-mode').value==='ranked'){
    authorityPending++;$('execute-status').textContent='Committing the locked vehicle, plan and prediction before flight…';
    try{
      if(!configure(wasm,nodes))throw Error('The locked design could not be compiled.');
      const p=locked.prediction;
      if(!wasm.mission_lock(p.dv,p.apo*1000,p.peri*1000,p.fuel,p.time,Number.isFinite(p.miss)?p.miss:NaN))throw Error('The locked prediction is invalid.');
      context.ticket=await authority.commit(wasm,contractId);
      if(version!==sequence||!locked)return;
      const current=identity.profile().publicKey.map(b=>b.toString(16).padStart(2,'0')).join('');
      if(current!==pubkey)throw Error('The active license changed; the committed attempt was not flown.');
      $('execute-status').textContent=context.ticket.mode===0?'Commit accepted. Simulating the ranked flight…':'This is a later attempt; the server requires practice. Simulating…';
      startFlight(version,context);
    }catch(error){
      if(version===sequence)$('execute-status').textContent=`Ranked launch stopped: ${String(error.message||error)}. No locked flight ran. If a response was lost, the attempt may have been consumed; retry may be practice.`;
    }finally{authorityPending--;}
  }else{
    practiceSimulated.add(contractId);updateAttemptMode();
    $('execute-status').textContent='Local practice: simulating the locked flight without a server rating…';
    startFlight(version,context);
  }
}
$('unlock').addEventListener('click',()=>{locked=null;cancelFlight();sequence++;engineLab?.setLocked();assembly?.setLocked();updateAttemptMode();$('unlock').hidden=true;$('debrief').hidden=true;$('execute-status').textContent='';for(const id of ['predict-dv','predict-apo','predict-peri','predict-fuel','predict-time','predict-miss'])$(id).disabled=false;updatePreview();});
canvas.addEventListener('pointerdown',e=>{
  if(locked||!preview.length||e.button!==0)return;const p=mousePoint(e),node=nodes[selected];
  if(node){const state=readState(wasm,node.time),center=toCanvas(state);for(const h of handles(center,state))if(distance(p,h.end)<12){drag={pointer:e.pointerId,key:h.key,start:p,initial:node.dv[{t:0,n:1,w:2}[h.key]],dir:h.dir};startLens(h.key);canvas.setPointerCapture(e.pointerId);return;}if(distance(p,center)<12){drag={pointer:e.pointerId,key:'time'};startLens('time');canvas.setPointerCapture(e.pointerId);return;}}
  const nearest=nearestPath(p);if(nearest&&nearest.d<12&&nodes.length<12){nodes.push({time:clampNodeTime(nearest.time),dv:[10,0,0]});selected=nodes.length-1;updatePreview();}
});
canvas.addEventListener('pointermove',e=>{
  if(!drag||drag.pointer!==e.pointerId||selected<0){
    if(!locked&&preview.length&&nodes[selected]){const p=mousePoint(e),state=readState(wasm,nodes[selected].time),center=toCanvas(state);const h=handles(center,state).find(h=>distance(p,h.end)<12);if(h)startLens(h.key);else if(activeLens&&!e.target.closest?.('.term'))stopLens();}
    return;
  }
  const p=mousePoint(e),node=nodes[selected];if(drag.key==='time'){const near=nearestPath(p);if(near)node.time=clampNodeTime(near.time);}else{const i={t:0,n:1,w:2}[drag.key];node.dv[i]=drag.initial+2*((p[0]-drag.start[0])*drag.dir[0]+(p[1]-drag.start[1])*drag.dir[1]);}delete node.guidance;updatePreview();
});
function endDrag(e){if(drag&&drag.pointer===e.pointerId){drag=null;stopLens();}}
canvas.addEventListener('pointerup',endDrag);canvas.addEventListener('pointercancel',endDrag);canvas.addEventListener('lostpointercapture',endDrag);canvas.addEventListener('pointerleave',e=>{if(!drag&&!intoLens(e))stopLens();});
new ResizeObserver(draw).observe(canvas.parentElement);
window.missionDiagnostics=Object.freeze({wasm:()=>wasm,recorder:()=>lastRecorder,verification:()=>verificationResult(),openVerify,profile:()=>identity.profile(),isRanked:id=>identity.isRanked(id),openProfile,get nodeTimeRange(){return nodeTimeRange();},clampNodeTime,get nodeMarker(){const n=nodes[selected];if(!n||!wasm)return null;const s=readState(wasm,n.time);return s.some(x=>!Number.isFinite(x))?null:toCanvas(s);},get pathEnds(){return preview.length?[toCanvas(preview[0].state),toCanvas(preview[preview.length-1].state)]:null;},get nodes(){return nodes.map(n=>({time:n.time,dv:[...n.dv],guidance:n.guidance?[...n.guidance]:null}));},get pending(){return !!worker||authorityPending>0;},get draws(){return draws;},get messages(){return messages;},get locked(){return !!locked;},get report(){return lastReport?[...lastReport]:null;},get computeMs(){return lastComputeMs;},get lunarInfo(){return lastLunarInfo?[...lastLunarInfo]:null;},get lens(){return activeLens;},get lensDraws(){return lensDraws;},get lensPending(){return lensGovernor.frame!==null;},get engine(){return engineLab?engineLab.diagnostics():null;},get appliedEngine(){return appliedEngine?{...appliedEngine}:null;},get assembly(){return assembly?assembly.diagnostics():null;},get boilOff(){return lastBoilOff;},get flownDelegationMask(){return lastDelegationMask;},get causalValid(){return wasm?wasm.mission_causal_valid()===1&&wasm.mission_engine_causal_valid()===1:false;},get acronymViolations(){return acronymViolations();},get handles(){if(selected<0||!wasm)return [];const state=readState(wasm,nodes[selected].time);return handles(toCanvas(state),state).map(h=>({key:h.key,end:h.end}));},get refusal(){return wasm?refusal(wasm):null;}});
try{const response=await fetch('./odd_view.wasm');if(!response.ok)throw Error('The physics module is not available. Build it first.');({instance:{exports:wasm}}=await WebAssembly.instantiate(await response.arrayBuffer(),{}));identity.init(wasm);initProfile();initVerify(wasm);initLeaderboard();updateAttemptMode(true);loadGlossary(wasm);
// The lab reads canonical stage numbers back after a redesign so the planner,
// the worker payload and the signed recorder cannot disagree about the engine.
engineLab=createLab({wasm,isLocked:()=>!!locked,annotate:annotateAcronyms,onDiagnose:()=>renderEngineCausal(),onApply:baselineOnly=>{
  // The bridge already rebuilt the stage; record the committed design so every
  // later reconfigure and every worker payload uses exactly this one.
  appliedEngine=baselineOnly?null:{...engineLab.diagnostics().spec};
  if(assemblyDesign&&!applyAssembly(wasm,assemblyDesign))assemblyDesign=null;
  assembly?.sync();engineLab?.setLocked();
  if(!baselineOnly){nodes=[];selected=-1;}
  if(!reapplyEngine(wasm))appliedEngine=null;
  DURATION=wasm.mission_parameter(1);DRY=wasm.mission_parameter(3);
  setup=Array.from({length:12},(_,i)=>wasm.mission_parameter(i));
  INITIAL=Math.hypot(setup[6],setup[7],setup[8]);
  $('debrief').hidden=true;assembly?.sync();engineLab?.setLocked();updatePreview();
}});
assembly=createAssembly({wasm,isLocked:()=>!!locked,annotate:annotateAcronyms,onChange:()=>{
  assemblyDesign=assemblyPayload(wasm);
  // A chief's margin is dry mass and a thicker blanket changes the hardware,
  // so any plan sized for the previous stage is stale.
  nodes=[];selected=-1;
  DURATION=wasm.mission_parameter(1);DRY=wasm.mission_parameter(3);
  setup=Array.from({length:12},(_,i)=>wasm.mission_parameter(i));
  INITIAL=Math.hypot(setup[6],setup[7],setup[8]);
  // Delegating Propulsion freezes the engine lab; handing it back unfreezes
  // it. The lab must be told whenever the assembly changes.
  $('debrief').hidden=true;engineLab?.setLocked();updatePreview();
}});
if(wasm.mission_contract(contractId)){engineLab.loadBaseline();engineLab.setLocked();assembly.sync();assembly.setLocked();}
annotateAcronyms(document.querySelector('.mission-main'));syncFields();updatePreview();}catch(e){$('plan-status').classList.add('failure');$('plan-status').textContent=String(e.message||e);annotateAcronyms($('plan-status'));}
