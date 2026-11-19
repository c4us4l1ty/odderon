/* Earth: one procedural globe with continents, ice, clouds, city lights and a
 * scattering limb, plus two additive atmosphere shells.
 *
 * The surface is invented geography, not a map. It is generated from a
 * direction hash so it never repeats per frame and costs no texture memory.
 * The body is placed in the vessel-centred render frame; orbital state remains
 * the source of truth. */

import {
  AdditiveBlending,
  Group,
  Mesh,
  ShaderMaterial,
  SphereGeometry,
  Vector3,
  BackSide,
} from "three";
import { EARTH_RADIUS } from "../core/math";

const noise = /* glsl */ `
float hash31(vec3 p){
  p = fract(p * vec3(0.1031, 0.1030, 0.0973));
  p += dot(p, p.yzx + 33.33);
  return fract((p.x + p.y) * p.z);
}
float noise3(vec3 p){
  vec3 i = floor(p);
  vec3 f = fract(p);
  f = f * f * (3.0 - 2.0 * f);
  float a = hash31(i + vec3(0.0, 0.0, 0.0));
  float b = hash31(i + vec3(1.0, 0.0, 0.0));
  float c = hash31(i + vec3(0.0, 1.0, 0.0));
  float d = hash31(i + vec3(1.0, 1.0, 0.0));
  float e = hash31(i + vec3(0.0, 0.0, 1.0));
  float f1 = hash31(i + vec3(1.0, 0.0, 1.0));
  float g = hash31(i + vec3(0.0, 1.0, 1.0));
  float h = hash31(i + vec3(1.0, 1.0, 1.0));
  return mix(mix(mix(a,b,f.x), mix(c,d,f.x), f.y), mix(mix(e,f1,f.x), mix(g,h,f.x), f.y), f.z);
}
float fbm(vec3 p){
  float sum = 0.0;
  float amp = 0.5;
  for(int i = 0; i < 5; i++){
    sum += amp * noise3(p);
    p *= 2.03;
    amp *= 0.5;
  }
  return sum;
}
`;

const surfaceVertex = /* glsl */ `
#include <common>
#include <logdepthbuf_pars_vertex>
varying vec3 vNormalW;
varying vec3 vWorldPos;
void main(){
  vNormalW = normalize(mat3(modelMatrix) * normal);
  vec4 wp = modelMatrix * vec4(position, 1.0);
  vWorldPos = wp.xyz;
  #include <logdepthbuf_vertex>
  gl_Position = projectionMatrix * viewMatrix * wp;
}
`;

const surfaceFragment = /* glsl */ `
#include <common>
#include <logdepthbuf_pars_fragment>
uniform vec3 uSunDir;
uniform float uTime;
varying vec3 vNormalW;
varying vec3 vWorldPos;

${noise}

void main(){
  #include <logdepthbuf_fragment>
  vec3 N = normalize(vNormalW);
  vec3 V = normalize(cameraPosition - vWorldPos);
  vec3 L = normalize(uSunDir);

  float ndl = dot(N, L);
  float day = smoothstep(-0.12, 0.18, ndl);
  float sun = max(ndl, 0.0);

  // ---- terrain: a warped macro field with a finer detail field on top.
  vec3 p = N;
  float warp = fbm(p * 2.1 + vec3(1.7, 0.3, 4.1));
  float macro = fbm(p * 3.3 + warp * 0.65 + vec3(11.0, 3.7, 7.1));
  float detail = fbm(p * 13.0 + vec3(4.0, 9.0, 2.0));
  float height = macro * 0.76 + detail * 0.24;
  // A second, larger-scale mask carves continent-sized basins.
  float plate = fbm(p * 1.2 + vec3(21.0, 2.0, 5.0));
  height += (plate - 0.5) * 0.10;

  float land = smoothstep(0.495, 0.531, height);
  float coast = smoothstep(0.455, 0.520, height);
  float lat = abs(N.y);
  float moisture = fbm(p * 4.6 + vec3(19.0, 3.0, 1.0)) + (fbm(p * 17.0) - 0.5) * 0.12;

  vec3 abyss = vec3(0.008, 0.038, 0.105);
  vec3 shelf = vec3(0.030, 0.160, 0.240);
  vec3 ocean = mix(abyss, shelf, coast);

  vec3 forest = vec3(0.075, 0.125, 0.055);
  vec3 savanna = vec3(0.235, 0.200, 0.095);
  vec3 desert = vec3(0.440, 0.350, 0.190);
  vec3 tundra = vec3(0.200, 0.210, 0.185);
  vec3 rock = vec3(0.320, 0.300, 0.265);
  vec3 ground = mix(forest, savanna, smoothstep(0.46, 0.60, moisture));
  ground = mix(ground, desert, smoothstep(0.63, 0.80, moisture));
  ground = mix(ground, tundra, smoothstep(0.56, 0.82, lat));
  ground = mix(ground, rock, smoothstep(0.60, 0.74, height) * 0.45);

  vec3 albedo = mix(ocean, ground, land);
  // Shorelines stay pale and turquoise before the land fully wins.
  albedo = mix(albedo, vec3(0.045, 0.180, 0.210), smoothstep(0.50, 0.55, height) * (1.0 - land));

  float ice = smoothstep(0.855, 0.945, lat + (fbm(p * 11.0 + vec3(3.0)) - 0.5) * 0.12);
  albedo = mix(albedo, vec3(0.76, 0.83, 0.90), ice);

  // ---- clouds: two layers, drifting, with a soft displaced shadow.
  float cw = fbm(p * 3.0 + vec3(uTime * 0.0025));
  float baseCloud = fbm(p * 5.8 + cw * 1.15 + vec3(0.0, uTime * 0.0012, 0.0));
  float fineCloud = fbm(p * 17.0 + cw * 0.85 + vec3(0.0, uTime * 0.004, 0.0));
  float cloud = smoothstep(0.485, 0.600, baseCloud) * 0.80;
  cloud += smoothstep(0.520, 0.640, fineCloud) * 0.34;
  cloud = clamp(cloud, 0.0, 1.0);
  float cloudShadow = smoothstep(0.49, 0.61, baseCloud * 0.7 + fineCloud * 0.3);

  // ---- lighting
  vec3 lit = albedo * (0.030 + 1.75 * sun);
  lit *= 1.0 - cloudShadow * 0.24 * land * day;

  vec3 H = normalize(L + V);
  float spec = pow(max(dot(N, H), 0.0), 110.0) * (1.0 - land) * (1.0 - cloud) * day;
  lit += vec3(0.62, 0.80, 1.0) * spec * 1.10;

  float cityNoise = noise3(N * 420.0);
  float city = smoothstep(0.80, 0.96, cityNoise) * land * (1.0 - cloud) * (1.0 - smoothstep(-0.20, 0.04, ndl));
  lit += vec3(1.0, 0.66, 0.34) * city * 0.22;

  float term = exp(-abs(ndl) * 9.0);
  lit += vec3(0.52, 0.18, 0.05) * term * 0.05;

  float fres = pow(1.0 - max(dot(N, V), 0.0), 3.6);
  lit += vec3(0.14, 0.34, 0.82) * fres * day * 0.22;

  vec3 cloudCol = vec3(0.95, 0.97, 1.0) * (0.040 + 1.60 * sun);
  cloudCol += vec3(0.55, 0.24, 0.08) * term * 0.14;
  lit = mix(lit, cloudCol, cloud * 0.86);

  #include <tonemapping_fragment>
  #include <colorspace_fragment>
  gl_FragColor = vec4(lit, 1.0);
}
`;

const atmoVertex = /* glsl */ `
#include <common>
#include <logdepthbuf_pars_vertex>
varying vec3 vNormalW;
varying vec3 vWorldPos;
void main(){
  vNormalW = normalize(mat3(modelMatrix) * normal);
  vec4 wp = modelMatrix * vec4(position, 1.0);
  vWorldPos = wp.xyz;
  #include <logdepthbuf_vertex>
  gl_Position = projectionMatrix * viewMatrix * wp;
}
`;

const atmoFragment = /* glsl */ `
#include <common>
#include <logdepthbuf_pars_fragment>
uniform vec3 uSunDir;
uniform float uPower;
uniform float uStrength;
varying vec3 vNormalW;
varying vec3 vWorldPos;
void main(){
  #include <logdepthbuf_fragment>
  vec3 N = normalize(vNormalW);
  vec3 V = normalize(cameraPosition - vWorldPos);
  float rim = pow(1.0 - abs(dot(N, V)), uPower);
  float sun = smoothstep(-0.35, 0.35, dot(N, normalize(uSunDir)));
  float alpha = rim * uStrength * (0.10 + 0.90 * sun);
  vec3 col = mix(vec3(0.05, 0.28, 0.95), vec3(0.30, 0.62, 1.0), sun);
  #include <tonemapping_fragment>
  #include <colorspace_fragment>
  gl_FragColor = vec4(col, alpha);
}
`;

export class EarthSystem {
  readonly group = new Group();
  readonly mesh: Mesh;
  private readonly inner: Mesh;
  private readonly outer: Mesh;
  private readonly surfaceUniforms: { uSunDir: { value: Vector3 }; uTime: { value: number } };
  private readonly innerUniforms: { uSunDir: { value: Vector3 }; uPower: { value: number }; uStrength: { value: number } };
  private readonly outerUniforms: { uSunDir: { value: Vector3 }; uPower: { value: number }; uStrength: { value: number } };

  constructor(sunDir: Vector3) {
    const geometry = new SphereGeometry(1, 160, 96);
    this.surfaceUniforms = {
      uSunDir: { value: sunDir.clone().normalize() },
      uTime: { value: 0 },
    };
    const material = new ShaderMaterial({
      vertexShader: surfaceVertex,
      fragmentShader: surfaceFragment,
      uniforms: this.surfaceUniforms,
    });
    this.mesh = new Mesh(geometry, material);
    this.mesh.scale.setScalar(EARTH_RADIUS);
    this.mesh.frustumCulled = false;

    const innerGeo = new SphereGeometry(1, 96, 64);
    this.innerUniforms = {
      uSunDir: { value: sunDir.clone().normalize() },
      uPower: { value: 2.6 },
      uStrength: { value: 0.30 },
    };
    this.inner = new Mesh(
      innerGeo,
      new ShaderMaterial({
        vertexShader: atmoVertex,
        fragmentShader: atmoFragment,
        uniforms: this.innerUniforms,
        transparent: true,
        depthWrite: false,
        blending: AdditiveBlending,
        side: BackSide,
      }),
    );
    this.inner.scale.setScalar(EARTH_RADIUS * 1.006);
    this.inner.frustumCulled = false;

    this.outerUniforms = {
      uSunDir: { value: sunDir.clone().normalize() },
      uPower: { value: 2.0 },
      uStrength: { value: 0.11 },
    };
    this.outer = new Mesh(
      new SphereGeometry(1, 96, 64),
      new ShaderMaterial({
        vertexShader: atmoVertex,
        fragmentShader: atmoFragment,
        uniforms: this.outerUniforms,
        transparent: true,
        depthWrite: false,
        blending: AdditiveBlending,
        side: BackSide,
      }),
    );
    this.outer.scale.setScalar(EARTH_RADIUS * 1.045);
    this.outer.frustumCulled = false;

    this.group.add(this.mesh, this.inner, this.outer);
  }

  /** Position the planet in the vessel-centred render frame. */
  setVesselPosition(r: Vector3): void {
    this.group.position.copy(r).multiplyScalar(-1);
  }

  setSun(sunDir: Vector3): void {
    this.surfaceUniforms.uSunDir.value.copy(sunDir).normalize();
    this.innerUniforms.uSunDir.value.copy(sunDir).normalize();
    this.outerUniforms.uSunDir.value.copy(sunDir).normalize();
  }

  update(time: number): void {
    this.surfaceUniforms.uTime.value = time;
  }
}
