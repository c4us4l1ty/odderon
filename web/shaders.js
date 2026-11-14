export const vertex = `#version 300 es
precision highp float;
out vec2 uv;
void main(){ vec2 p=vec2(float((gl_VertexID<<1)&2),float(gl_VertexID&2)); uv=p; gl_Position=vec4(p*2.0-1.0,0,1); }`;

// Analytic spheres with decorative procedural pigment. Real light and body
// orientation are supplied in J2000; the terrain is explicitly not a map.
export const sceneFragment = `#version 300 es
precision highp float;
in vec2 uv;
layout(location=0) out vec4 colorOut;
layout(location=1) out vec4 normalOut;
uniform vec2 uResolution;
uniform vec4 uEarth; uniform vec4 uMoon; uniform float uVehicle;
uniform vec3 uRight; uniform vec3 uUp; uniform vec3 uView;
uniform vec3 uEarthLight; uniform vec3 uMoonLight;
uniform mat3 uEarthOrientation; uniform mat3 uMoonOrientation;
float hash(vec3 p){return fract(sin(dot(p,vec3(127.1,311.7,74.7)))*43758.5453);}
float noise(vec3 p){vec3 i=floor(p), f=fract(p); f=f*f*(3.0-2.0*f); float a=mix(hash(i),hash(i+vec3(1,0,0)),f.x);float b=mix(hash(i+vec3(0,1,0)),hash(i+vec3(1,1,0)),f.x);float c=mix(hash(i+vec3(0,0,1)),hash(i+vec3(1,0,1)),f.x);float d=mix(hash(i+vec3(0,1,1)),hash(i+vec3(1,1,1)),f.x);return mix(mix(a,b,f.y),mix(c,d,f.y),f.z);}
float fbm(vec3 p){return noise(p)*.55+noise(p*2.03)*.28+noise(p*4.11)*.12;}
void main(){
 vec2 p=(2.0*gl_FragCoord.xy-uResolution)/uResolution.y;
 // Subtle paper grain only: this is not a synthetic star catalog.
 float grain=(hash(vec3(gl_FragCoord.xy,37.0))-.5)*.003;
 vec3 bg=mix(vec3(.018,.039,.041),vec3(.039,.076,.070),clamp(1.0-length(p)*.65,0.0,1.0))+grain;
 float de=length(p-uEarth.xy)/uEarth.z, dm=length(p-uMoon.xy)/uMoon.z;
 float halo=exp(-max(de-1.0,0.0)*8.0)*.11*(1.0-smoothstep(.98,1.0,de));
 // An atmosphere glow extends beyond the silhouette.
 bg+=vec3(.12,.32,.29)*exp(-abs(de-1.0)*13.0)*step(1.0,de)*.55;
 // An optional, screen-space parametric vehicle study. It is a non-physical
 // illustration, not a simulated spacecraft or a reported location.
 if(uVehicle>.5){
   vec2 q=(p-vec2(-uResolution.x/uResolution.y*.76,-.64))/0.075;
   float hull=max(abs(q.x)-.22,max(-q.y-.60,q.y-.45));
   float nose=max(abs(q.x)*1.6+q.y-.82,-q.y-.45);
   float fin=max(abs(q.x)-(.22+max(0.0,-q.y-.2)*.8),max(q.y+.25,-q.y-.65));
   float plume=max(abs(q.x)-(.10+max(0.0,-q.y-.63)*.36),max(q.y+.63,-q.y-1.45));
   if(plume<0.0){float flicker=fbm(vec3(q*5.0,3.0));bg+=vec3(.7,.29,.09)*(1.0-smoothstep(-.35,0.0,plume))*(.6+flicker*.4);}
   if(min(min(hull,nose),fin)<0.0){colorOut=vec4(mix(vec3(.42,.48,.43),vec3(.83,.80,.64),clamp(q.x+0.6,0.0,1.0)),1);normalOut=vec4(.5,.5,.7,1);return;}
 }
 bool earth=de<1.0, moon=dm<1.0; bool isEarth=earth && (!moon || uEarth.w>=uMoon.w);
 if(!earth&&!moon){colorOut=vec4(bg,1);normalOut=vec4(.5,.5,0,1);return;}
 vec4 body=isEarth?uEarth:uMoon; vec2 local=(p-body.xy)/body.z;
 vec3 n=normalize(uRight*local.x+uUp*local.y+uView*sqrt(max(0.0,1.0-dot(local,local))));
 vec3 fixedNormal=transpose(isEarth?uEarthOrientation:uMoonOrientation)*n;
 vec3 light=normalize(isEarth?uEarthLight:uMoonLight);
 float illumination=max(dot(n,light),0.0); float shade=.17+.83*illumination;
 vec3 pigment;
 if(isEarth){
   float land=fbm(fixedNormal*3.4+vec3(8.0,4.0,2.0));
   float coast=smoothstep(.48,.54,land); vec3 sea=vec3(.12,.29,.32); vec3 ground=mix(vec3(.29,.36,.25),vec3(.58,.55,.36),smoothstep(.52,.68,land));
   pigment=mix(sea,ground,coast); float cloud=smoothstep(.60,.75,fbm(fixedNormal*9.0+vec3(24.0,1.0,9.0))); pigment=mix(pigment,vec3(.82,.87,.79),cloud*.55);
   pigment=mix(pigment,vec3(.80,.88,.83),smoothstep(.80,.97,abs(fixedNormal.z))*.8);
 }else{float terrain=fbm(fixedNormal*6.0+vec3(17.0)); pigment=mix(vec3(.28,.31,.28),vec3(.70,.69,.59),terrain);}
 // Tangential, normal-aligned pigment streaks: deliberately illustrative.
 float streak=sin(atan(local.y,local.x)*36.0+length(local)*29.0+fbm(fixedNormal*20.0)*6.0);
 pigment*=1.0+streak*.025;
 colorOut=vec4(pigment*shade+vec3(.03,.045,.04)*halo,1.0);
 normalOut=vec4(local*.5+.5,isEarth?.7:.9,1.0);
}`;

// Instanced, deterministic screen-space strokes read the real G-buffer.
// The analytic surface normal supplies a projected tangential flow direction.
export const strokeVertex = `#version 300 es
precision highp float; precision highp int;
uniform sampler2D uNormal; uniform vec2 uResolution; uniform vec4 uEarth; uniform vec4 uMoon; uniform int uBatch;
out vec2 vBrush; out vec2 vSample; flat out float vSurface;
uint mixbits(uint x){x^=x>>16;x*=0x7feb352du;x^=x>>15;x*=0x846ca68bu;x^=x>>16;return x;}
float rnd(uint x){return float(mixbits(x)&0x00ffffffu)/16777216.0;}
void main(){
 uint id=uint(gl_InstanceID)+uint(uBatch)*100003u;
 // Sample physical projected discs, not random empty sky. Surface clipping
 // in the fragment shader prevents brush quads enlarging the body silhouette.
 float earthWeight=uEarth.z*uEarth.z/(uEarth.z*uEarth.z+uMoon.z*uMoon.z);
 vec4 body=rnd(id*5u+1u)<earthWeight?uEarth:uMoon;
 float theta=6.28318530718*rnd(id*5u+2u),radialDistance=sqrt(rnd(id*5u+3u));
 vec2 point=body.xy+body.z*radialDistance*vec2(cos(theta),sin(theta));
 vec2 center=(uResolution+point*uResolution.y)/(2.0*uResolution);
 vec3 normal=texture(uNormal,center).rgb;
 vec2 radial=normal.xy*2.0-1.0;
 vec2 tangent=length(radial)>.001?normalize(vec2(-radial.y,radial.x)):vec2(1,0);
 vec2 perpendicular=vec2(-tangent.y,tangent.x);
 vec2 corners[6]=vec2[6](vec2(-1,-1),vec2(1,-1),vec2(-1,1),vec2(-1,1),vec2(1,-1),vec2(1,1));
 vec2 corner=corners[gl_VertexID];
 float size=uBatch==0?4.0:mix(3.0,1.1,float(uBatch)/15.0);
 vec2 offset=(tangent*corner.x*size+perpendicular*corner.y*size*.28)*2.0/uResolution;
 gl_Position=vec4(center*2.0-1.0+offset,0,1);
 vBrush=corner;vSample=center;vSurface=normal.b;
}`;
export const strokeFragment = `#version 300 es
precision highp float;
uniform sampler2D uColor; uniform sampler2D uNormal; uniform vec2 uResolution; in vec2 vBrush; in vec2 vSample; flat in float vSurface; out vec4 outColor;
void main(){
 float fragmentSurface=texture(uNormal,gl_FragCoord.xy/uResolution).b;
 if(vSurface<.1 || abs(fragmentSurface-vSurface)>.08)discard;
 float shape=max(0.0,1.0-dot(vBrush,vBrush));
 if(shape<=0.0)discard;
 vec3 pigment=texture(uColor,vSample).rgb;
 outColor=vec4(pigment,shape*.28);
}`;
