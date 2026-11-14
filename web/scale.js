// Physical equatorial radii in meters, without a display enlargement factor.
export const EARTH_RADIUS_M=6_378_137;
export const MOON_RADIUS_M=1_737_400;
export const projectedRadius=(radiusM,spanM)=>2*radiusM/spanM;
export const needsReticle=(radius,height)=>radius*height<8;
