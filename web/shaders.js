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

// Eight overlapping polynomial sectors, with ellipse orientation and eccentricity
// derived from the locally smoothed image structure tensor. Formulation follows
// Kyprianidis et al. (2010), with a small fixed radius for modest GPU budgets.
export const paintFragment = `#version 300 es
precision highp float;
in vec2 uv; out vec4 outColor;
uniform sampler2D uColor; uniform sampler2D uNormal; uniform vec2 uResolution; uniform float uQuality;
vec3 colorAt(vec2 pixelOffset){return texture(uColor,uv+pixelOffset/uResolution).rgb;}
void main(){
 vec3 base=colorAt(vec2(0)); vec3 normal=texture(uNormal,uv).rgb;
 if(normal.b<.1 || uQuality<.5){outColor=vec4(base,1);return;}
 // Gaussian-smoothed structure tensor of RGB gradients. Its major eigenvector
 // points across an edge; the minor eigenvector is the direction of the brush.
 float jxx=0.0, jxy=0.0, jyy=0.0, total=0.0;
 for(int x=-1;x<=1;x++)for(int y=-1;y<=1;y++){
   vec2 p=vec2(float(x),float(y));
   vec3 gx=(colorAt(p+vec2(1,0))-colorAt(p-vec2(1,0)))*.5;
   vec3 gy=(colorAt(p+vec2(0,1))-colorAt(p-vec2(0,1)))*.5;
   float w=(x==0?2.0:1.0)*(y==0?2.0:1.0);
   jxx+=w*dot(gx,gx);jxy+=w*dot(gx,gy);jyy+=w*dot(gy,gy);total+=w;
 }
 jxx/=total;jxy/=total;jyy/=total;
 float diff=sqrt(max(0.0,(jxx-jyy)*(jxx-jyy)+4.0*jxy*jxy));
 float anisotropy=clamp(diff/max(jxx+jyy,0.000001),0.0,1.0);
 float angle=diff>0.0000001?.5*atan(2.0*jxy,jxx-jyy):0.0; vec2 across=vec2(cos(angle),sin(angle));
 vec2 tangent=vec2(-across.y,across.x);
 // In flat regions use the analytic surface normal's tangent for a stable
 // normal-aligned brush direction; the isotropic kernel has no preferred axis.
 if(jxx+jyy<0.00001){vec2 radial=normal.xy*2.0-1.0;if(length(radial)>.05){tangent=normalize(vec2(-radial.y,radial.x));across=vec2(-tangent.y,tangent.x);}}
 float a=2.0*(1.0+anisotropy), b=2.0/(1.0+anisotropy);
 vec3 sums[8]; vec3 squares[8]; float weights[8];
 for(int k=0;k<8;k++){sums[k]=vec3(0);squares[k]=vec3(0);weights[k]=0.0;}
 for(int x=-2;x<=2;x++)for(int y=-2;y<=2;y++){
   vec2 v=vec2(float(x),float(y))*.5; float r2=dot(v,v); if(r2>1.0)continue;
   vec2 offset=tangent*v.x*a+across*v.y*b; vec3 c=colorAt(offset);
   float sectors[8];float sectorSum=0.0;
   for(int k=0;k<8;k++){
     float theta=6.28318530718*float(k)/8.0; vec2 dir=vec2(cos(theta),sin(theta));
     float longitudinal=dot(v,dir), lateral=dot(v,vec2(-dir.y,dir.x));
     float z=max(0.0,longitudinal+0.3333333333-3.77*lateral*lateral);
     sectors[k]=z*z;sectorSum+=sectors[k];
   }
   float radialWeight=exp(-3.125*r2)/max(sectorSum,0.000001);
   for(int k=0;k<8;k++){float wk=sectors[k]*radialWeight;sums[k]+=c*wk;squares[k]+=c*c*wk;weights[k]+=wk;}
 }
 vec3 result=vec3(0);float all=0.0;
 for(int k=0;k<8;k++){
   vec3 mean=sums[k]/max(weights[k],0.000001);
   vec3 variance=max(squares[k]/max(weights[k],0.000001)-mean*mean,vec3(0));
   float sigma=10.0*sqrt(dot(variance,vec3(1)));
   float alpha=1.0/(1.0+pow(sigma,8.0)); result+=mean*alpha; all+=alpha;
 }
 outColor=vec4(mix(base,result/max(all,0.000001),.7),1.0);
}`;

// Instanced, deterministic screen-space strokes read the real G-buffer.
// The analytic surface normal supplies a projected tangential flow direction.
export const strokeVertex = `#version 300 es
precision highp float; precision highp int;
uniform sampler2D uNormal; uniform vec2 uResolution; uniform int uBatch;
out vec2 vBrush; out vec2 vSample; flat out float vSurface;
uint mixbits(uint x){x^=x>>16;x*=0x7feb352du;x^=x>>15;x*=0x846ca68bu;x^=x>>16;return x;}
float rnd(uint x){return float(mixbits(x)&0x00ffffffu)/16777216.0;}
void main(){
 uint id=uint(gl_InstanceID)+uint(uBatch)*100003u;
 vec2 center=vec2(rnd(id*2u+31u),rnd(id*2u+32u));
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
uniform sampler2D uColor; in vec2 vBrush; in vec2 vSample; flat in float vSurface; out vec4 outColor;
void main(){
 if(vSurface<.1)discard;
 float shape=max(0.0,1.0-dot(vBrush,vBrush));
 if(shape<=0.0)discard;
 vec3 pigment=texture(uColor,vSample).rgb;
 outColor=vec4(pigment,shape*.28);
}`;
