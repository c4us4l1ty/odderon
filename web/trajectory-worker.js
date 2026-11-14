// One-shot worker: propagation is off the render thread; it terminates after
// responding. All positions remain Earth-centered J2000 SI coordinates.
self.onmessage=async ({data})=>{
  try{
    const response=await fetch('./odd_view.wasm');if(!response.ok)throw Error('Propagation module unavailable');
    const {instance}=await WebAssembly.instantiate(await response.arrayBuffer(),{}), w=instance.exports;
    const {epoch,duration,mu,position,velocity,mass}=data;
    if(!w.trajectory_load(epoch,duration,mu,mass,...position,...velocity))throw Error('Propagation failed: orbit may intersect Earth or leave model range.');
    const points=[];for(let i=0;i<=240;i++){const t=duration*i/240;points.push([w.trajectory_value(t,0),w.trajectory_value(t,1),w.trajectory_value(t,2)]);}
    if(points.some(p=>p.some(x=>!Number.isFinite(x))))throw Error('Invalid trajectory sample');
    self.postMessage({points});
  }catch(e){self.postMessage({error:String(e.message||e)});}
};
