// Lambert-sphere approximate reflected solar irradiance, relative to direct
// sunlight. All positions are in the same inertial frame. This is deliberately
// a broad-body approximation, not a photometric surface map.
export function planetShine(body, sun, craft, radius, albedo){
  const toSun=sun.map((x,i)=>x-body[i]),toCraft=craft.map((x,i)=>x-body[i]);
  const ds=Math.hypot(...toSun),d=Math.hypot(...toCraft);
  if(!Number.isFinite(ds)||!Number.isFinite(d)||ds===0||d<=radius||!Number.isFinite(albedo)||albedo<0)return {direction:[0,0,1],strength:0};
  const c=Math.max(-1,Math.min(1,toSun.reduce((v,x,i)=>v+x*toCraft[i],0)/(ds*d)));
  const phase=Math.acos(c),lambert=(Math.sin(phase)+(Math.PI-phase)*c)/Math.PI;
  return {direction:toCraft.map(x=>-x/d),strength:albedo*(radius/d)**2*lambert};
}
