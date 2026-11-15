// One-shot flight/targeting worker. Heavy propagation, perigee targeting and
// counterfactuals stay off the UI thread. The main page terminates stale workers.
self.onmessage=async ({data})=>{
  try{
    const response=await fetch('./odd_view.wasm');if(!response.ok)throw Error('Flight physics module unavailable.');
    const {instance:{exports:w}}=await WebAssembly.instantiate(await response.arrayBuffer(),{});
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
    self.postMessage({points,lunar,report,lunarInfo,computeMs});
  }catch(e){self.postMessage({error:String(e.message||e)});}
};
