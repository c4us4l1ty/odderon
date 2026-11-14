// One-shot flight; all heavy propagation and counterfactuals stay off the UI.
self.onmessage=async ({data})=>{
  try{
    const response=await fetch('./odd_view.wasm');if(!response.ok)throw Error('Flight physics module unavailable.');
    const {instance:{exports:w}}=await WebAssembly.instantiate(await response.arrayBuffer(),{});
    if(!w.mission_setup(...data.setup))throw Error('Invalid spacecraft or mission parameters.');
    for(const n of data.nodes)if(!w.mission_add_node(n.time,...n.dv))throw Error('Invalid maneuver.');
    if(!w.mission_compile())throw Error('Maneuvers overlap, exceed the fuel budget, or fall outside the mission.');
    if(!w.mission_execute())throw Error('Flight could not be evaluated. The trajectory may be unbound or cross Earth.');
    const points=[];for(let i=0;i<=360;i++){const t=data.setup[1]*i/360;points.push([w.mission_actual(t,0),w.mission_actual(t,1),w.mission_actual(t,2)]);}
    const report=Array.from({length:26},(_,i)=>w.mission_report(i));
    if(points.some(p=>p.some(x=>!Number.isFinite(x)))||report.slice(0,6).some(x=>!Number.isFinite(x)))throw Error('Flight returned an invalid state.');
    self.postMessage({points,report});
  }catch(e){self.postMessage({error:String(e.message||e)});}
};
