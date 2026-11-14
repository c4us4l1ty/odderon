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
uniform vec2 uResolution;
uniform vec4 uEarth; uniform vec4 uMoon; uniform float uVehicle;
uniform vec3 uRight; uniform vec3 uUp; uniform vec3 uView;
uniform vec3 uEarthLight; uniform vec3 uMoonLight;
uniform vec3 uCraft; uniform vec3 uCraftLight; uniform vec3 uEarthShine; uniform vec3 uMoonShine; uniform vec2 uBounce;
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
 // Small screen-space spacecraft glyph anchored at the supplied orbit state.
 // Its size and surface are illustrative, but light vectors come from the
 // craft/body/Sun geometry. Depth test against the visible spherical bodies.
 if(uVehicle>.5){
   vec2 q=(p-uCraft.xy)/0.035;
   float hull=max(abs(q.x)-.22,max(-q.y-.60,q.y-.45));
   float nose=max(abs(q.x)*1.6+q.y-.82,-q.y-.45);
   float fin=max(abs(q.x)-(.22+max(0.0,-q.y-.2)*.8),max(q.y+.25,-q.y-.65));
   bool hidden=(de<1.0&&uCraft.z<uEarth.w+uEarth.z*.5*sqrt(max(0.0,1.0-de*de)))||(dm<1.0&&uCraft.z<uMoon.w+uMoon.z*.5*sqrt(max(0.0,1.0-dm*dm)));
   if(min(min(hull,nose),fin)<0.0&&!hidden){
     vec3 n=normalize(uRight*q.x+uUp*(q.y*.5)+uView*.7);
     float sun=dot(n,normalize(uCraftLight));
     vec3 key=vec3(1.0,.957,.863), shadow=vec3(.118,.227,.431), amber=vec3(.88,.42,.24);
     vec3 light=sun>.12?key:(sun>=-.06?amber:shadow);
     float bounce=uBounce.x*max(dot(n,uEarthShine),0.0)+uBounce.y*max(dot(n,uMoonShine),0.0);
     float rim=pow(1.0-max(dot(n,uView),0.0),4.0)*smoothstep(-.06,.12,sun);
     colorOut=vec4(vec3(.64,.68,.65)*(light+vec3(.45,.58,.7)*bounce)+key*rim*.45,1);return;
   }
 }
 bool earth=de<1.0, moon=dm<1.0; bool isEarth=earth && (!moon || uEarth.w>=uMoon.w);
 if(!earth&&!moon){colorOut=vec4(bg,1);return;}
 vec4 body=isEarth?uEarth:uMoon; vec2 local=(p-body.xy)/body.z;
 vec3 n=normalize(uRight*local.x+uUp*local.y+uView*sqrt(max(0.0,1.0-dot(local,local))));
 vec3 fixedNormal=transpose(isEarth?uEarthOrientation:uMoonOrientation)*n;
 vec3 light=normalize(isEarth?uEarthLight:uMoonLight);
 float ndl=dot(n,light);
 // A deliberately stylized three-band key, terminator, and cool shadow.
 vec3 shadow=vec3(.118,.227,.431), rimBand=vec3(.88,.42,.24), key=vec3(1.0,.957,.863);
 vec3 lighting=ndl > .12 ? key : (ndl >= -.06 ? rimBand : shadow);
 float sunGate=smoothstep(-.06,.12,ndl);
 float fresnel=pow(1.0-max(dot(n,uView),0.0),4.0)*sunGate;
 vec3 pigment;
 if(isEarth){
   float land=fbm(fixedNormal*3.4+vec3(8.0,4.0,2.0));
   float coast=smoothstep(.48,.54,land); vec3 sea=vec3(.12,.29,.32); vec3 ground=mix(vec3(.29,.36,.25),vec3(.58,.55,.36),smoothstep(.52,.68,land));
   pigment=mix(sea,ground,coast);
   // Decorative cloud bands and a displaced, directional ground shadow.
   vec3 bodyLight=transpose(uEarthOrientation)*light;
   float cloudNoise=fbm(fixedNormal*9.0+vec3(24.0,1.0,9.0)+vec3(0.0,0.0,sin(fixedNormal.z*18.0)*.35));
   float shadowNoise=fbm((fixedNormal+bodyLight*.025)*9.0+vec3(24.0,1.0,9.0)+vec3(0.0,0.0,sin(fixedNormal.z*18.0)*.35));
   pigment*=1.0-step(.64,shadowNoise)*sunGate*.22;
   float cloud=step(.64,cloudNoise);
   vec3 cloudColor=ndl>.12?vec3(.95,.94,.84):vec3(.39,.49,.62);
   pigment=mix(pigment,cloudColor,cloud*.82);
   pigment=mix(pigment,vec3(.80,.88,.83),smoothstep(.80,.97,abs(fixedNormal.z))*.8);
 }else{float terrain=fbm(fixedNormal*6.0+vec3(17.0)); pigment=mix(vec3(.28,.31,.28),vec3(.70,.69,.59),terrain);}
 // Tangential, normal-aligned pigment streaks: deliberately illustrative.
 float streak=sin(atan(local.y,local.x)*36.0+length(local)*29.0+fbm(fixedNormal*20.0)*6.0);
 pigment*=1.0+streak*.025;
 colorOut=vec4(pigment*lighting+vec3(1.0,.957,.863)*fresnel*.24+vec3(.03,.045,.04)*halo,1.0);
}`;
