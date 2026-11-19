// Procedural art adapted from the Odderon 2 render library.
import {
  AdditiveBlending,
  Color,
  CylinderGeometry,
  DoubleSide,
  Group,
  LatheGeometry,
  Mesh,
  MeshBasicMaterial,
  MeshStandardMaterial,
  PointLight,
  SphereGeometry,
  Sprite,
  SpriteMaterial,
  TorusGeometry,
  Vector2,
  ShaderMaterial,
  BoxGeometry,
  ConeGeometry,
} from "three";
import { clamp } from "./math.js";
import {
  glowTexture,
  hullTexture,
  metalTexture,
  solarTexture,
} from "./textures.js";
const plumeVertex =
  /* glsl */
  `
varying vec2 vUv;
#include <common>
#include <logdepthbuf_pars_vertex>
varying vec3 vNormalW;
varying vec3 vWorldPos;
void main(){
  vUv = uv;
  vNormalW = normalize(mat3(modelMatrix) * normal);
  vec4 wp = modelMatrix * vec4(position, 1.0);
  vWorldPos = wp.xyz;
  #include <logdepthbuf_vertex>
  gl_Position = projectionMatrix * viewMatrix * wp;
}
`;
const plumeFragment =
  /* glsl */
  `
#include <common>
#include <logdepthbuf_pars_fragment>
uniform float uTime;
uniform float uThrottle;
varying vec2 vUv;
varying vec3 vNormalW;
varying vec3 vWorldPos;

float hash(vec2 p){ return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }
float noise(vec2 p){
  vec2 i = floor(p), f = fract(p);
  f = f * f * (3.0 - 2.0 * f);
  return mix(mix(hash(i), hash(i + vec2(1,0)), f.x), mix(hash(i + vec2(0,1)), hash(i + vec2(1,1)), f.x), f.y);
}

void main(){
  #include <logdepthbuf_fragment>
  vec3 N = normalize(vNormalW);
  vec3 V = normalize(cameraPosition - vWorldPos);
  float facing = abs(dot(N, V));
  float along = 1.0 - vUv.y;                 // 0 at the nozzle, 1 at the tail
  float taper = pow(max(1.0 - along, 0.0), 1.55); // fade out down the plume
  float flicker = 0.84 + 0.16 * noise(vec2(uTime * 22.0, along * 9.0));
  float turbulence = noise(vec2(along * 16.0 - uTime * 11.0, vUv.x * 9.0));

  // The bright column is the part of the cone facing the camera.
  float dense = pow(facing, 1.05);
  float core = pow(facing, 2.8) * smoothstep(0.72, 0.0, along);
  float diamonds = 0.74 + 0.26 * sin(along * 54.0 - uTime * 6.0) * exp(-along * 1.9);

  vec3 hot = mix(vec3(0.72, 0.88, 1.0), vec3(1.0, 0.99, 0.98), core);
  vec3 cool = mix(vec3(0.20, 0.34, 0.98), vec3(0.52, 0.52, 1.0), turbulence * 0.5);
  vec3 col = mix(cool, hot, clamp(core * 1.5 + 0.06, 0.0, 1.0));

  float alpha = (0.10 + dense * 0.42 + core * 0.72) * taper * uThrottle * flicker;
  alpha *= 0.78 + 0.42 * turbulence;
  alpha *= diamonds;

  #include <tonemapping_fragment>
  #include <colorspace_fragment>
  gl_FragColor = vec4(col, clamp(alpha, 0.0, 1.0));
}
`;
export class Ship {
  root = new Group();
  engineLight;
  plumeGroup = new Group();
  plumeInner;
  plumeOuter;
  plumeCoreMat;
  plumeUniformsInner;
  plumeUniformsOuter;
  nozzleGlow;
  engineAssembly = new Group();
  strobe;
  strobeMat;
  hull;
  time = 0;
  constructor() {
    const hullTex = hullTexture();
    const metalTex = metalTexture();
    this.hull = new MeshStandardMaterial({
      map: hullTex,
      color: 16777215,
      roughness: 0.52,
      metalness: 0.18,
    });
    const dark = new MeshStandardMaterial({
      map: metalTex,
      color: 12567492,
      roughness: 0.46,
      metalness: 0.72,
    });
    const metal = new MeshStandardMaterial({
      color: 10134182,
      roughness: 0.27,
      metalness: 0.95,
    });
    const black = new MeshStandardMaterial({
      color: 1053462,
      roughness: 0.6,
      metalness: 0.35,
    });
    const gold = new MeshStandardMaterial({
      color: 13081151,
      roughness: 0.36,
      metalness: 0.55,
    });
    const ceramic = new MeshStandardMaterial({
      color: 14605264,
      roughness: 0.7,
      metalness: 0.08,
    });
    const body = new Group();
    this.root.add(body);
    const tank = new Mesh(
      new CylinderGeometry(1.78, 1.78, 5, 64, 1, false),
      this.hull,
    );
    tank.position.y = 1.1;
    body.add(tank);
    for (const [y, h, mat] of [
      [2.34, 0.34, black],
      [2.84, 0.16, black],
      [0.62, 0.1, gold],
    ]) {
      const ring2 = new Mesh(
        new CylinderGeometry(1.795, 1.795, h, 64, 1, true),
        mat,
      );
      ring2.position.y = y;
      body.add(ring2);
    }
    const skirt = new Mesh(
      new CylinderGeometry(1.92, 1.58, 0.9, 48, 1, true),
      dark,
    );
    skirt.position.y = -1.9;
    body.add(skirt);
    const ringAft = new Mesh(new TorusGeometry(1.9, 0.07, 10, 56), metal);
    ringAft.rotation.x = Math.PI / 2;
    ringAft.position.y = -1.52;
    body.add(ringAft);
    const bay = new Mesh(new CylinderGeometry(1.2, 1.5, 0.7, 40), black);
    bay.position.y = -2.55;
    body.add(bay);
    const taper = new Mesh(
      new CylinderGeometry(1.36, 1.78, 1.1, 48),
      this.hull,
    );
    taper.position.y = 4.15;
    body.add(taper);
    const service = new Mesh(new CylinderGeometry(1.1, 1.34, 1.4, 48), gold);
    service.position.y = 5.1;
    body.add(service);
    const serviceBand = new Mesh(
      new CylinderGeometry(1.115, 1.115, 0.16, 48),
      gold,
    );
    serviceBand.position.y = 4.62;
    body.add(serviceBand);
    const nosePts = [];
    for (let i = 0; i <= 22; i++) {
      const t = i / 22;
      const r = 1.1 * (1 - t) ** 1.35 + 8e-3;
      nosePts.push(new Vector2(Math.max(r, 0.03), t * 1.62));
    }
    const nose = new Mesh(new LatheGeometry(nosePts, 56), ceramic);
    nose.position.y = 5.78;
    body.add(nose);
    const noseCollar = new Mesh(
      new CylinderGeometry(1.115, 1.115, 0.1, 48),
      black,
    );
    noseCollar.position.y = 5.8;
    body.add(noseCollar);
    const ring = new Mesh(new TorusGeometry(0.19, 0.032, 10, 40), metal);
    ring.rotation.x = Math.PI / 2;
    ring.position.y = 7.44;
    body.add(ring);
    const hatch = new Mesh(new CylinderGeometry(0.175, 0.175, 0.05, 32), dark);
    hatch.position.y = 7.43;
    body.add(hatch);
    const probe = new Mesh(new CylinderGeometry(0.022, 0.022, 0.5, 8), metal);
    probe.position.y = 7.72;
    body.add(probe);
    for (let k = 0; k < 4; k++) {
      const a = (k / 4) * Math.PI * 2 + Math.PI / 4;
      const quad = new Group();
      quad.position.set(Math.cos(a) * 1.72, 3.9, Math.sin(a) * 1.72);
      quad.rotation.y = -a;
      const pod = new Mesh(new BoxGeometry(0.34, 0.5, 0.26), metal);
      quad.add(pod);
      for (const [dx, dy] of [
        [0.19, 0.14],
        [-0.19, 0.14],
        [0.19, -0.14],
        [-0.19, -0.14],
      ]) {
        const nozzle = new Mesh(new ConeGeometry(0.055, 0.12, 8), black);
        nozzle.position.set(dx, dy, 0.19);
        nozzle.rotation.x = Math.PI / 2;
        quad.add(nozzle);
      }
      body.add(quad);
    }
    for (const side of [-1, 1]) {
      const rad = new Group();
      rad.position.set(side * 1.86, 1.9, 0);
      rad.rotation.z = side * -0.12;
      rad.rotation.y = side * 0.55;
      const panel = new Mesh(new BoxGeometry(0.07, 2.1, 1.15), ceramic);
      rad.add(panel);
      for (let i = -2; i <= 2; i++) {
        const rib = new Mesh(new BoxGeometry(0.09, 2.16, 0.05), metal);
        rib.position.z = i * 0.26;
        rad.add(rib);
      }
      body.add(rad);
    }
    const solar = new MeshStandardMaterial({
      map: solarTexture(),
      color: 16777215,
      roughness: 0.42,
      metalness: 0.35,
      emissive: new Color(662067),
      emissiveIntensity: 0.35,
    });
    for (const side of [-1, 1]) {
      const wing = new Group();
      wing.position.set(side * 2.2, 2.3, 0.55);
      const spar = new Mesh(new CylinderGeometry(0.045, 0.045, 2.3, 10), metal);
      spar.rotation.z = Math.PI / 2;
      wing.add(spar);
      for (let k = 0; k < 3; k++) {
        const panel = new Mesh(new BoxGeometry(1.35, 1.55, 0.035), solar);
        panel.position.x = side * (0.75 + k * 1.38);
        wing.add(panel);
        const frame = new Mesh(new BoxGeometry(1.37, 1.57, 0.05), gold);
        frame.position.copy(panel.position);
        frame.position.z = -0.025;
        frame.scale.set(1, 1, 0.35);
        wing.add(frame);
      }
      wing.rotation.x = 0.62;
      wing.rotation.y = side * -0.42;
      wing.rotation.z = side * 0.06;
      body.add(wing);
    }
    for (const [ang, y0, len] of [
      [1.05, 0.4, 3],
      [-1.4, 1.1, 2.2],
      [2.5, 0.9, 2.4],
    ]) {
      const rail = new Group();
      const x = Math.cos(ang) * 1.755;
      const z = Math.sin(ang) * 1.755;
      rail.position.set(x, y0, z);
      rail.rotation.y = -ang;
      const bar = new Mesh(new CylinderGeometry(0.026, 0.026, len, 8), metal);
      bar.position.y = len / 2;
      rail.add(bar);
      for (let i = 0; i <= 4; i++) {
        const stand = new Mesh(
          new CylinderGeometry(0.018, 0.018, 0.12, 6),
          metal,
        );
        stand.rotation.x = Math.PI / 2;
        stand.position.set(0, (len * i) / 4, -0.05);
        rail.add(stand);
      }
      body.add(rail);
    }
    const blanket = new Mesh(
      new CylinderGeometry(1.795, 1.795, 0.7, 64, 1, true),
      gold,
    );
    blanket.position.y = -0.85;
    body.add(blanket);
    const boom = new Group();
    boom.position.set(1.42, 4.85, 0.55);
    boom.rotation.z = -0.62;
    boom.rotation.x = 0.42;
    const arm = new Mesh(new CylinderGeometry(0.045, 0.045, 1.5, 10), metal);
    arm.position.y = 0.65;
    boom.add(arm);
    const dish = new Mesh(
      new SphereGeometry(0.8, 28, 14, 0, Math.PI * 2, 0, Math.PI / 3),
      metal,
    );
    dish.rotation.x = Math.PI;
    dish.position.y = 1.35;
    dish.scale.y = 0.55;
    boom.add(dish);
    body.add(boom);
    this.engineAssembly.position.y = -2.9;
    body.add(this.engineAssembly);
    const bellPts = [
      new Vector2(0.36, 0.55),
      new Vector2(0.42, 0.3),
      new Vector2(0.5, 0.02),
      new Vector2(0.62, -0.32),
      new Vector2(0.8, -0.78),
      new Vector2(0.99, -1.24),
      new Vector2(1.06, -1.36),
      new Vector2(1.08, -1.4),
    ];
    const bell = new Mesh(new LatheGeometry(bellPts, 48), metal);
    this.engineAssembly.add(bell);
    const bellInner = new Mesh(
      new LatheGeometry(
        bellPts.map((p) => new Vector2(p.x * 0.94, p.y)),
        48,
      ),
      black,
    );
    bellInner.scale.set(1, 1, 1);
    this.engineAssembly.add(bellInner);
    const throat = new Mesh(new CylinderGeometry(0.34, 0.2, 0.5, 24), metal);
    throat.position.y = 0.78;
    this.engineAssembly.add(throat);
    const manifold = new Mesh(new TorusGeometry(0.56, 0.075, 8, 32), dark);
    manifold.rotation.x = Math.PI / 2;
    manifold.position.y = -0.25;
    this.engineAssembly.add(manifold);
    const exitRim = new Mesh(new TorusGeometry(1.075, 0.028, 8, 48), metal);
    exitRim.rotation.x = Math.PI / 2;
    exitRim.position.y = -1.4;
    this.engineAssembly.add(exitRim);
    for (const y of [-0.55, -0.9, -1.2]) {
      const rib = new Mesh(
        new TorusGeometry(0.68 + (y + 0.55) * -0.55, 0.022, 6, 36),
        metal,
      );
      rib.rotation.x = Math.PI / 2;
      rib.position.y = y;
      this.engineAssembly.add(rib);
    }
    const glowDisc = new Mesh(
      new ConeGeometry(0.98, 0.5, 32, 1, true),
      new MeshBasicMaterial({
        color: new Color(9422591),
        transparent: true,
        opacity: 0,
        blending: AdditiveBlending,
        side: DoubleSide,
        depthWrite: false,
      }),
    );
    glowDisc.rotation.x = Math.PI;
    glowDisc.position.y = -1.28;
    this.engineAssembly.add(glowDisc);
    this.nozzleGlow = new Sprite(
      new SpriteMaterial({
        map: glowTexture("rgba(255,255,255,1)", "rgba(80,150,255,0)"),
        blending: AdditiveBlending,
        depthWrite: false,
        transparent: true,
        opacity: 0,
      }),
    );
    this.nozzleGlow.scale.setScalar(4.6);
    this.nozzleGlow.position.y = -1.55;
    this.engineAssembly.add(this.nozzleGlow);
    this.plumeUniformsInner = { uTime: { value: 0 }, uThrottle: { value: 0 } };
    this.plumeUniformsOuter = { uTime: { value: 0 }, uThrottle: { value: 0 } };
    const innerGeo = new CylinderGeometry(0.5, 1.05, 5, 28, 1, true);
    innerGeo.translate(0, -2.5, 0);
    this.plumeInner = new Mesh(
      innerGeo,
      new ShaderMaterial({
        vertexShader: plumeVertex,
        fragmentShader: plumeFragment,
        uniforms: this.plumeUniformsInner,
        transparent: true,
        depthWrite: false,
        blending: AdditiveBlending,
        side: DoubleSide,
      }),
    );
    this.plumeInner.frustumCulled = false;
    this.plumeInner.position.y = -1.42;
    const outerGeo = new CylinderGeometry(0.62, 1.75, 9, 32, 1, true);
    outerGeo.translate(0, -4.5, 0);
    this.plumeOuter = new Mesh(
      outerGeo,
      new ShaderMaterial({
        vertexShader: plumeVertex,
        fragmentShader: plumeFragment,
        uniforms: this.plumeUniformsOuter,
        transparent: true,
        depthWrite: false,
        blending: AdditiveBlending,
        side: DoubleSide,
      }),
    );
    this.plumeOuter.frustumCulled = false;
    this.plumeOuter.position.y = -1.42;
    const coreGeo = new CylinderGeometry(0.1, 0.3, 4, 20, 1, true);
    coreGeo.translate(0, -2, 0);
    this.plumeCoreMat = new MeshBasicMaterial({
      color: 15267071,
      transparent: true,
      opacity: 0,
      blending: AdditiveBlending,
      side: DoubleSide,
      depthWrite: false,
    });
    const coreMesh = new Mesh(coreGeo, this.plumeCoreMat);
    coreMesh.position.y = -1.42;
    coreMesh.frustumCulled = false;
    this.plumeGroup.add(this.plumeOuter, this.plumeInner, coreMesh);
    this.engineAssembly.add(this.plumeGroup);
    const port = new Mesh(
      new SphereGeometry(0.07, 8, 8),
      new MeshBasicMaterial({ color: 16726831 }),
    );
    port.position.set(-1.8, 2.45, 0);
    body.add(port);
    const starboard = new Mesh(
      new SphereGeometry(0.07, 8, 8),
      new MeshBasicMaterial({ color: 3800970 }),
    );
    starboard.position.set(1.8, 2.45, 0);
    body.add(starboard);
    this.strobeMat = new SpriteMaterial({
      map: glowTexture("rgba(255,255,255,1)", "rgba(120,220,255,0)"),
      blending: AdditiveBlending,
      depthWrite: false,
      transparent: true,
      opacity: 0,
    });
    this.strobe = new Sprite(this.strobeMat);
    this.strobe.scale.setScalar(1.6);
    this.strobe.position.set(0, 7.66, 0);
    body.add(this.strobe);
    this.engineLight = new PointLight(10406143, 0, 90, 1.6);
    this.engineLight.position.y = -4.6;
    this.engineAssembly.add(this.engineLight);
    this.root.scale.setScalar(1);
  }
  /** Animate plume, lights and strobe. `dt` is wall-clock seconds. */
  update(dt, throttle) {
    this.time += dt;
    const t = clamp(throttle, 0, 1);
    this.plumeUniformsInner.uTime.value = this.time;
    this.plumeUniformsOuter.uTime.value = this.time;
    this.plumeUniformsInner.uThrottle.value = t;
    this.plumeUniformsOuter.uThrottle.value = t;
    const flicker =
      0.92 +
      Math.sin(this.time * 41) * 0.04 +
      Math.sin(this.time * 17.3) * 0.04;
    this.plumeGroup.visible = t > 0.01;
    this.plumeGroup.scale.set(0.8 + 0.4 * t, 0.55 + 0.85 * t, 0.8 + 0.4 * t);
    this.plumeCoreMat.opacity = t * 0.9;
    this.nozzleGlow.material.opacity = t * 0.9 * flicker;
    this.nozzleGlow.scale.setScalar((3.2 + 3.4 * t) * flicker);
    this.engineLight.intensity = t * 2200 * flicker;
    const phase = this.time % 1.6;
    this.strobeMat.opacity = phase < 0.09 ? 1 : 0;
  }
  setGimbal(angle) {
    this.engineAssembly.rotation.z = angle;
  }
  /** World-space position of the engine nozzle, for camera shake and audio. */
  nozzleWorld(target) {
    return target
      .set(0, -4.4, 0)
      .applyQuaternion(this.root.quaternion)
      .add(this.root.position);
  }
}
