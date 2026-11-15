// Playable mission planning. Scientific calculations are performed by the
// Rust/WASM planner and one-shot flight workers, not fabricated by canvas.
import { Governor } from './governor.js';
import * as identity from './identity.js';
import { initProfile, openProfile } from './profile.js';
import * as recorder from './recorder.js';
import { initVerify, openVerify, verifyFields, result as verificationResult } from './verify.js';
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
let wasm,nodes=[],compiledOrder=[],selected=-1,preview=[],lunarPreview=[],actual=null,lastReport=null,lastLunarInfo=null,worker=null,timer=null,locked=null,drag=null,draws=0,messages=0,sequence=0;
let activeLens=null,activeTerm=null,lensPhase=0,lensDraws=0,lastComputeMs=null,lastCapture=null,lastRecorder=null;
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
let GLOSSARY={},TERM_RE=null;
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
  pdr:'An engineering dossier comparing your locked prediction with modeled flight, signed error contributions and practice rating.',
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
function configure(w,items){
  if(contractId){if(!w.mission_contract(contractId))return false;}
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
  $('add-node').disabled=!wasm||!!locked||nodes.length>=12;$('example').disabled=!wasm||!!locked;$('mission-select').disabled=!wasm||!!locked;$('split-kick').disabled=!!locked;
}
function updatePreview(schedule=true){
  if(!wasm)return;
  cancelFlight();sequence++;syncFields();actual=null;lastReport=null;lastLunarInfo=null;lunarPreview=[];
  if(!configure(wasm,nodes)){$('plan-status').textContent='Unable to compile: check timing, overlapping burns, or available propellant.';$('plan-status').classList.add('failure');preview=[];$('prefill').disabled=true;$('execute').disabled=true;
  updateAttemptMode();updateCausal();draw();cancelFlight();return;}
  $('plan-status').classList.remove('failure');$('plan-status').textContent='Ideal orbit ready. The modeled flight calculates in the background.';
  preview=Array.from({length:481},(_,i)=>({time:DURATION*i/480,state:readState(wasm,DURATION*i/480)}));
  if(scenario==='103'){
    const valid=wasm.mission_lunar_info(0);
    lastLunarInfo=valid?[wasm.mission_lunar_info(1),wasm.mission_lunar_info(2)]:null;
    if(valid)lunarPreview=Array.from({length:481},(_,i)=>readStatePatched(wasm,DURATION*i/480)).filter(s=>s.every(Number.isFinite));
  }
  if(preview.some(p=>p.state.some(x=>!Number.isFinite(x)))){preview=[];$('plan-status').textContent='The ideal orbit could not be sampled.';cancelFlight();draw();return;}
  $('prefill').disabled=false;$('execute').disabled=nodes.length===0||!!locked;
  syncFields();updateCausal();draw();annotateAcronyms($('plan-status'));if(schedule)scheduleFlight();
}
function readStatePatched(w,t){return Array.from({length:7},(_,i)=>w.mission_lunar_preview(t,i));}
function cancelFlight(){if(timer!==null){clearTimeout(timer);timer=null;}if(worker){worker.terminate();worker=null;}}
function scheduleFlight(){cancelFlight();const version=++sequence;timer=setTimeout(()=>{timer=null;startFlight(false,version);},350);}
function workerPayload(isLocked){
  const snapshot=nodes.map(n=>({time:n.time,dv:[...n.dv],guidance:n.guidance?[...n.guidance]:null}));
  const payload={contract:contractId,setup,nodes:snapshot,metric:scenario==='102'?1:0,callsign:identity.profile()?.callsign??''};
  if(isLocked){const p=locked.prediction;payload.locked=[p.dv,p.apo*1000,p.peri*1000,p.fuel,p.time,Number.isFinite(p.miss)?p.miss:NaN];}
  return payload;
}
function startFlight(isLocked,version=++sequence){
  cancelFlight();worker=new Worker('./mission-worker.js',{type:'module'});const current=worker;
  current.onmessage=({data})=>{messages++;if(worker!==current||version!==sequence)return;current.terminate();worker=null;
    if(data.error){$('plan-status').textContent=data.error;$('plan-status').classList.add('failure');if(isLocked)$('execute-status').textContent=data.error;return;}
    actual=data.points;lastReport=data.report;lastCapture=data.capture??null;lastRecorder=null;lastComputeMs=Number.isFinite(data.computeMs)?data.computeMs:null;lastLunarInfo=data.lunarInfo?.[0]?data.lunarInfo:null;if(data.lunar?.length)lunarPreview=data.lunar;updateCausal();draw();$('plan-status').textContent='Flight model ready. No simulation runs while idle.';annotateAcronyms($('plan-status'));
    if(isLocked)showDebrief(data.report);
  };
  current.onerror=()=>{if(worker!==current)return;worker.terminate();worker=null;$('plan-status').textContent='Flight worker failed.';if(isLocked)$('execute-status').textContent='Flight worker failed.';};
  messages++;current.postMessage(workerPayload(isLocked));
}
function prefill(){
  const state=readState(wasm,DURATION),metrics=orbitMetrics(state);if(!metrics){$('execute-status').textContent='The ideal orbit has no finite farthest point.';return false;}
  $('predict-dv').value=nodes.reduce((a,n)=>a+Math.hypot(...n.dv),0).toFixed(3);
  $('predict-apo').value=metrics.apo.toFixed(3);$('predict-peri').value=metrics.peri.toFixed(3);
  $('predict-fuel').value=(state[6]-DRY).toFixed(3);$('predict-time').value=String(DURATION);$('predict-miss').value='';$('execute-status').textContent='';return true;
}
function showDebrief(report){
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
  const boil=document.createElement('tr');boil.innerHTML='<th scope="row">Boil-off loss</th><td>—</td><td>Not modeled</td><td>—</td>';tbody.append(boil);table.append(tbody);
  const cards=document.createElement('div');cards.className='debrief-grid';
  const entries=[['Mission score',`${fmt(score*100,1)}%`],['Orbit eccentricity',fmt(e,5)],['Orbital-plane tilt',`${fmt(inclination,2)}°`]];
  if(scenario==='102')entries.push(['Five-day node rotation',`${fmt(drift,3)}°`]);
  if(scenario==='103')entries.push(['Apoapsis radius',`${fmt((apo+R)/1000)} km`],['Target radius error',`${fmt(report[40]/1000)} km`]);
  if(Number.isFinite(report[32]))entries.push(['Lunar sphere-of-influence entry',`${fmt(report[32]/86400,3)} days`]);
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
  const requested=$('attempt-mode')?.value??'ranked';
  const missionRating=identity.profile()?.missionElo[String(contractId)]??CONTRACT_RATINGS[contractId]??1200;
  const outcome=identity.recordFlight({contractId,score,mode:requested,missionRating});
  if(outcome.ranked){
    elo.textContent=`Ranked flight certified. Astro-Elo ${fmt(outcome.before,0)} → ${fmt(outcome.after,1)} (${outcome.delta>=0?'+':''}${fmt(outcome.delta,1)}, K=${outcome.k}); contract rating ${fmt(outcome.missionBefore,0)} → ${fmt(outcome.missionAfter,1)}. Contract ${contractId} now has a locked official certificate; further attempts are practice.`;
  }else{
    elo.textContent=`Practice flight — Astro-Elo unchanged at ${fmt(outcome.after,1)}. ${outcome.alreadyRanked?`Contract ${contractId} already has a certified ranked certificate, so this run cannot change the rating.`:'Practice mode was selected.'}`;
  }
  const actions=document.createElement('div');actions.className='button-row';
  const print=document.createElement('button');print.id='export-pdr';print.textContent='Print debrief (PDF)';print.title='Opens the browser print dialog; choose Save as PDF.';print.addEventListener('click',()=>window.print());actions.append(print);
  // Sign the recorder now that the rating outcome is settled. Signing is cheap
  // and stays on this thread so the license secret never enters a worker.
  const signed=document.createElement('p');signed.className='profile-note';signed.id='recorder-status';
  if(lastCapture){
    try{
      const fields=recorder.stampOutcome(lastCapture,{ranked:outcome.ranked,before:outcome.before,after:outcome.after});
      lastRecorder=recorder.sign(wasm,fields,identity.profile().callsign);
      // A ranked certificate is issued once, so keep the signed document with
      // the certificate; a later reload must still be able to export it.
      if(outcome.ranked)identity.attachCertificate(contractId,lastRecorder.json);
      signed.textContent=`Flight recorder signed by ${lastRecorder.document.public_id}. ${outcome.ranked?'Ranked certificate':'Practice record'}; canonical digest ${lastRecorder.document.canonical_digest.slice(0,16)}…`;
    }catch(error){signed.textContent=`Recorder could not be signed: ${String(error.message||error)}`;}
  }else{signed.textContent='No recorder was captured for this flight.';}
  const exportButton=document.createElement('button');exportButton.id='export-fdr';exportButton.textContent='Export signed recorder (.odderon-fdr.json)';exportButton.disabled=!lastRecorder;
  exportButton.addEventListener('click',()=>{if(lastRecorder)recorder.download(`contract-${contractId}.odderon-fdr.json`,`${lastRecorder.json}\n`,'application/json');});
  const dossierButton=document.createElement('button');dossierButton.id='verify-flight';dossierButton.textContent='Verify / Replay Dossier';dossierButton.disabled=!lastCapture;
  dossierButton.addEventListener('click',()=>{
    if(!lastRecorder)return;
    const {fields,callsign}=recorder.fieldsFromJson(lastRecorder.document);
    openVerify();verifyFields(fields,callsign);
  });
  actions.append(exportButton,dossierButton);
  $('debrief-content').replaceChildren(table,cards,loss,heading,list,elo,signed,actions);
  annotateAcronyms($('debrief-content'));$('debrief').hidden=false;$('execute-status').textContent=impacted?'Flight ended in impact. The prediction remains locked.':'Flight complete. The prediction remains locked.';annotateAcronyms($('execute-status'));
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
  if(scenario==='103'&&lastLunarInfo){const row=document.createElement('div');row.className='cause-chain';row.textContent=`Lunar sphere-of-influence entry: ideal patched preview enters at ${Math.round(lastLunarInfo[0])} s. The ${Math.round(lastLunarInfo[1]/1000)} km boundary is a preview aid, not a gravity discontinuity; the gold path keeps continuous third-body gravity.`;target.append(row);}
  if(!target.childNodes.length)target.textContent='No rule currently triggered. This is a limited check, not a safety certification.';
  annotateAcronyms(target);
}
$('split-kick').addEventListener('click',()=>{
  if(scenario==='103'){
    const total=nodes.reduce((a,n)=>a+Math.hypot(...n.dv),0)||3240;
    const first=nodes[0]?.time||1600;
    cancelFlight();const version=++sequence;worker=new Worker('./mission-worker.js',{type:'module'});const current=worker;
    $('plan-status').textContent='Targeting numerical periapses in the background…';$('split-kick').disabled=true;
    current.onmessage=({data})=>{messages++;if(worker!==current||version!==sequence)return;current.terminate();worker=null;
      if(data.error){$('plan-status').textContent=data.error;$('split-kick').disabled=!!locked;return;}
      nodes=data.nodes;selected=nodes.length-1;updatePreview();$('plan-status').textContent='Three equal kicks placed at numerically propagated periapses. Check the gold path and the apoapsis radius.';annotateAcronyms($('plan-status'));
    };
    current.onerror=()=>{if(worker!==current)return;worker.terminate();worker=null;$('plan-status').textContent='Targeting worker failed.';$('split-kick').disabled=!!locked;};
    messages++;current.postMessage({action:'split',kicks:3,total,first});return;
  }
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
// Ranked eligibility belongs to the persistent license, so the control
// reflects it instead of letting a player farm the same contract.
function updateAttemptMode(reset=false){
  const select=$('attempt-mode');if(!select)return;
  const certified=identity.isRanked($('mission-select').value);
  select.querySelector('option[value="ranked"]').disabled=certified;
  // Switching contracts returns to the eligible default; a certification
  // forces practice. Mid-planning refreshes leave a deliberate choice alone.
  if(certified)select.value='practice';else if(reset)select.value='ranked';
  const note=$('attempt-mode-note');
  if(note)note.textContent=certified?'This contract already has a certified ranked certificate. Further runs are practice and cannot change Astro-Elo.':'Your first execution of this contract is ranked and locks its official certificate.';
}
identity.onChange(()=>updateAttemptMode());
$('mission-select').addEventListener('change',()=>{
  updateAttemptMode(true);
  if(locked)return;scenario=$('mission-select').value;contractId=Number(scenario);cancelFlight();sequence++;nodes=[];selected=-1;actual=null;lastReport=null;lastLunarInfo=null;lunarPreview=[];
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
  $('unlock').hidden=false;$('debrief').hidden=true;$('execute-status').textContent='Prediction locked. Simulating the flight and its comparisons…';syncFields();startFlight(true);
});
$('unlock').addEventListener('click',()=>{locked=null;cancelFlight();sequence++;$('unlock').hidden=true;$('debrief').hidden=true;$('execute-status').textContent='';for(const id of ['predict-dv','predict-apo','predict-peri','predict-fuel','predict-time','predict-miss'])$(id).disabled=false;updatePreview();});
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
  const p=mousePoint(e),node=nodes[selected];if(drag.key==='time'){const near=nearestPath(p);if(near)node.time=clampNodeTime(near.time);}else{const i={t:0,n:1,w:2}[drag.key];node.dv[i]=drag.initial+2*((p[0]-drag.start[0])*drag.dir[0]+(p[1]-drag.start[1])*drag.dir[1]);}delete node.guidance;updatePreview(false);
});
function endDrag(e){if(drag&&drag.pointer===e.pointerId){drag=null;stopLens();scheduleFlight();}}
canvas.addEventListener('pointerup',endDrag);canvas.addEventListener('pointercancel',endDrag);canvas.addEventListener('lostpointercapture',endDrag);canvas.addEventListener('pointerleave',e=>{if(!drag&&!intoLens(e))stopLens();});
new ResizeObserver(draw).observe(canvas.parentElement);
window.missionDiagnostics=Object.freeze({wasm:()=>wasm,recorder:()=>lastRecorder,verification:()=>verificationResult(),openVerify,profile:()=>identity.profile(),isRanked:id=>identity.isRanked(id),openProfile,get nodeTimeRange(){return nodeTimeRange();},clampNodeTime,get nodeMarker(){const n=nodes[selected];if(!n||!wasm)return null;const s=readState(wasm,n.time);return s.some(x=>!Number.isFinite(x))?null:toCanvas(s);},get pathEnds(){return preview.length?[toCanvas(preview[0].state),toCanvas(preview[preview.length-1].state)]:null;},get nodes(){return nodes.map(n=>({time:n.time,dv:[...n.dv],guidance:n.guidance?[...n.guidance]:null}));},get pending(){return !!worker||timer!==null;},get draws(){return draws;},get messages(){return messages;},get locked(){return !!locked;},get report(){return lastReport?[...lastReport]:null;},get computeMs(){return lastComputeMs;},get lunarInfo(){return lastLunarInfo?[...lastLunarInfo]:null;},get lens(){return activeLens;},get lensDraws(){return lensDraws;},get lensPending(){return lensGovernor.frame!==null;},get causalValid(){return wasm?wasm.mission_causal_valid()===1:false;},get acronymViolations(){return acronymViolations();},get handles(){if(selected<0||!wasm)return [];const state=readState(wasm,nodes[selected].time);return handles(toCanvas(state),state).map(h=>({key:h.key,end:h.end}));}});
try{const response=await fetch('./odd_view.wasm');if(!response.ok)throw Error('The physics module is not available. Build it first.');({instance:{exports:wasm}}=await WebAssembly.instantiate(await response.arrayBuffer(),{}));identity.init(wasm);initProfile();initVerify(wasm);loadGlossary(wasm);annotateAcronyms(document.querySelector('.mission-main'));syncFields();updatePreview();}catch(e){$('plan-status').classList.add('failure');$('plan-status').textContent=String(e.message||e);annotateAcronyms($('plan-status'));}
