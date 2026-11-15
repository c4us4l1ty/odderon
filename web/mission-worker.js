// One-shot flight/targeting worker. Heavy propagation, perigee targeting and
// counterfactuals stay off the UI thread. The main page terminates stale workers.
self.onmessage=async ({data})=>{
  try{
    const response=await fetch('./odd_view.wasm');if(!response.ok)throw Error('Flight physics module unavailable.');
    const {instance:{exports:w}}=await WebAssembly.instantiate(await response.arrayBuffer(),{});
    if(data.action==='verify'){
      w.mission_fdr_reset();
      for(const value of data.fields)if(!w.mission_fdr_field(value))throw Error('recorder field rejected');
      w.mission_input_reset();
      for(const byte of new TextEncoder().encode(data.callsign))w.mission_input_absorb(byte);
      if(w.mission_fdr_parse()!==1)throw Error('The recorder layout is not valid.');
      const started=performance.now();
      const code=w.mission_fdr_verify();
      const read=field=>{const n=w.mission_fdr_string_len(field);return new TextDecoder().decode(Uint8Array.from({length:n},(_,i)=>w.mission_fdr_string_byte(field,i)));};
      const verdict=read(2);
      let dossier=null,dossierError=null;
      if(code===1){
        if(w.mission_fdr_dossier_from_record()===1)dossier=read(1);
        else dossierError='The flight verified, but the dossier could not be rendered: the verification payload is too large for a QR code.';
      }
      self.postMessage({verification:{code,verdict,mismatchIndex:w.mission_fdr_mismatch_index(),mismatchName:read(4),dossier,dossierError,json:read(0),computeMs:performance.now()-started}});
      return;
    }
    if(data.action==='split'){
      if(!w.mission_contract(103))throw Error('Unknown contract.');
      if(!w.mission_split_tug(data.kicks,data.total,data.first))throw Error('Numerical perigee targeting failed.');
      const nodes=Array.from({length:w.mission_node_count()},(_,i)=>({
        time:w.mission_node_time(i),
        dv:[w.mission_node_dv(i,0),w.mission_node_dv(i,1),w.mission_node_dv(i,2)],
        guidance:w.mission_node_guided(i)?Array.from({length:6},(_,j)=>w.mission_node_guidance(i,j)):null
      }));
      self.postMessage({nodes});return;
    }
    if(data.contract){if(!w.mission_contract(data.contract))throw Error('Unknown contract.');}
    else if(!w.mission_setup(...data.setup))throw Error('Invalid spacecraft or mission parameters.');
    if(!w.mission_metric(data.metric))throw Error('Invalid attribution metric.');
    const nodes=[...data.nodes].sort((a,b)=>a.time-b.time);
    let index=0;
    for(const n of nodes){
      if(Math.hypot(...n.dv)<1e-12)continue;
      if(!w.mission_add_node(n.time,...n.dv))throw Error('Invalid maneuver.');
      if(n.guidance&&!w.mission_add_guidance(index,...n.guidance))throw Error('Invalid guidance state.');
      index++;
    }
    if(!w.mission_compile())throw Error('Maneuvers overlap, exceed the fuel budget, or fall outside the mission.');
    if(data.locked&&!w.mission_lock(...data.locked))throw Error('Invalid locked prediction.');
    const computeStart=performance.now();
    if(!w.mission_execute())throw Error('Flight could not be evaluated.');
    const computeMs=performance.now()-computeStart;
    const duration=data.setup[1];
    const stop=Number.isFinite(w.mission_report(29))?w.mission_report(29):duration;
    const points=[];for(let i=0;i<=480;i++){const t=stop*i/480;points.push([w.mission_actual(t,0),w.mission_actual(t,1),w.mission_actual(t,2)]);}
    const hasPatch=w.mission_lunar_info(0)===1;
    const lunar=[];if(data.contract===103&&hasPatch){for(let i=0;i<=480;i++){const t=duration*i/480;const p=[w.mission_lunar_preview(t,0),w.mission_lunar_preview(t,1),w.mission_lunar_preview(t,2)];if(p.every(Number.isFinite))lunar.push(p);}}
    const report=Array.from({length:w.mission_report_len()},(_,i)=>w.mission_report(i));
    const lunarInfo=hasPatch?[w.mission_lunar_info(0),w.mission_lunar_info(1),w.mission_lunar_info(2)]:null;
    if(points.some(p=>p.some(x=>!Number.isFinite(x)))||!Number.isFinite(report[0]))throw Error('Flight returned an invalid state.');
    // Capture the executed flight for signing. The page owns the license and
    // the rating outcome, so the secret stays out of the worker: it stamps the
    // mode and Elo and signs on the main thread.
    let capture=null;
    w.mission_input_reset();
    for(const byte of new TextEncoder().encode(data.callsign??''))w.mission_input_absorb(byte);
    if(w.mission_fdr_capture(1,0,0)===1){
      const count=w.mission_fdr_fields_from_record();
      capture=Array.from({length:count},(_,i)=>w.mission_fdr_field_at(i));
    }
    self.postMessage({points,lunar,report,lunarInfo,computeMs,capture});
  }catch(e){self.postMessage({error:String(e.message||e)});}
};
