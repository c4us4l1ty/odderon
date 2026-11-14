// Physical equatorial radii in meters, without a display enlargement factor.
export const EARTH_RADIUS_M=6_378_137;
export const MOON_RADIUS_M=1_737_400;
export const projectedRadius=(radiusM,spanM)=>2*radiusM/spanM;
export const needsReticle=(radius,height)=>radius*height<8;
// Fit a central-body-relative orbit into the shorter viewport dimension.
// Radius includes the supplied vehicle and all sampled trajectory points.
export function orbitViewSpan(radiusM,width,height){
  if(!Number.isFinite(radiusM)||radiusM<0||!Number.isFinite(width)||!Number.isFinite(height)||width<=0||height<=0)return null;
  const span=Math.max(30e6,2.2*radiusM/Math.min(1,width/height));
  return Number.isFinite(span)?span:null;
}
