/* The 3D flight view: renderer, camera rig, sun, Earth, ship and orbit lines.
 *
 * The render frame is vessel-centred. Orbital state is in Earth-centred metres;
 * every object is shifted by -r on the way into the scene. This keeps a 400 km
 * orbit and a 9 m spacecraft in the same float space without jitter. */

import {
  ACESFilmicToneMapping,
  AdditiveBlending,
  DirectionalLight,
  AmbientLight,
  PerspectiveCamera,
  Scene,
  Sprite,
  SpriteMaterial,
  Vector3,
  WebGLRenderer,
} from "three";
import { EARTH_RADIUS, clamp, smoothstep } from "../core/math";
import { Elements } from "../core/orbit";
import { EarthSystem } from "./earth";
import { Ship } from "./ship";
import { Sky } from "./sky";
import { Trajectory } from "./trajectory";
import { glowTexture, markerTexture } from "./textures";

export type CameraMode = "flight" | "chase" | "map";

export interface SceneInput {
  elements: Elements;
  /** Inertial position and velocity of the vessel, metres / m·s⁻¹. */
  position: Vector3;
  velocity: Vector3;
  orientationQuaternion: { x: number; y: number; z: number; w: number };
  throttle: number;
  actualThrottle: number;
  gimbal: number;
  planned: Elements | null;
  target: Elements | null;
  nodePosition: Vector3 | null;
  showTarget: boolean;
  time: number;
  mode: CameraMode;
  paused: boolean;
}

export interface CameraInput {
  yaw: number;
  pitch: number;
  distance: number;
  cinematic: boolean;
}

export class FlightView {
  readonly renderer: WebGLRenderer;
  readonly scene = new Scene();
  readonly camera = new PerspectiveCamera(52, 1, 0.08, 6e8);
  private readonly sky = new Sky();
  readonly earth: EarthSystem;
  readonly ship = new Ship();
  private readonly trajectory = new Trajectory();
  private readonly sun = new Vector3(0.65, 0.42, 0.62).normalize();
  private readonly sunSprite: Sprite;
  private readonly sunCore: Sprite;
  private readonly shipMarker: Sprite;
  private readonly sunLight: DirectionalLight;
  private readonly earthShine: DirectionalLight;

  private readonly up = new Vector3();
  private readonly forward = new Vector3();
  private readonly side = new Vector3();

  private camMode: CameraMode = "flight";
  private camPos = new Vector3();
  private camTarget = new Vector3();
  private lastTime = 0;
  private shakeSeed = 0;
  private camInitialised = false;
  private cameraInput: CameraInput = { yaw: 0.55, pitch: 0.16, distance: 27, cinematic: false };

  constructor(canvas: HTMLCanvasElement) {
    this.renderer = new WebGLRenderer({
      canvas,
      antialias: true,
      alpha: false,
      powerPreference: "high-performance",
      logarithmicDepthBuffer: true,
      stencil: false,
    });
    this.renderer.setClearColor(0x010204, 1);
    this.renderer.outputColorSpace = "srgb";
    this.renderer.toneMapping = ACESFilmicToneMapping;
    this.renderer.toneMappingExposure = 1.02;
    this.renderer.autoClear = false;
    this.renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));

    this.earth = new EarthSystem(this.sun);
    this.scene.add(this.earth.group, this.ship.root, this.trajectory.group);

    this.sunLight = new DirectionalLight(0xfff1dd, 3.6);
    this.sunLight.position.copy(this.sun).multiplyScalar(1e6);
    this.scene.add(this.sunLight);
    this.scene.add(this.sunLight.target);

    // Earthshine: a cool fill from the planet below.
    this.earthShine = new DirectionalLight(0x3d78bd, 0.85);
    this.earthShine.position.set(0, -1, 0);
    this.scene.add(this.earthShine);
    this.scene.add(this.earthShine.target);
    this.scene.add(new AmbientLight(0x1a2634, 0.65));

    const sunMat = new SpriteMaterial({
      map: glowTexture("rgba(255,253,244,1)", "rgba(255,150,60,0)", "rgba(255,196,120,0.35)"),
      blending: AdditiveBlending,
      transparent: true,
      depthWrite: false,
      depthTest: true,
    });
    this.sunSprite = new Sprite(sunMat);
    this.sunSprite.scale.setScalar(1.9e6);
    this.sunSprite.position.copy(this.sun).multiplyScalar(9e6);
    this.scene.add(this.sunSprite);

    this.sunCore = new Sprite(
      new SpriteMaterial({
        map: glowTexture("rgba(255,255,255,1)", "rgba(255,255,255,0)", "rgba(255,255,255,0.9)"),
        blending: AdditiveBlending,
        transparent: true,
        depthWrite: false,
      }),
    );
    this.sunCore.scale.setScalar(2.2e5);
    this.sunCore.position.copy(this.sun).multiplyScalar(9e6 - 1);
    this.scene.add(this.sunCore);

    this.shipMarker = new Sprite(
      new SpriteMaterial({
        map: markerTexture("VEHICLE", "#ffd38a"),
        transparent: true,
        depthWrite: false,
        depthTest: false,
        opacity: 0.95,
      }),
    );
    this.shipMarker.visible = false;
    this.shipMarker.renderOrder = 12;
    this.scene.add(this.shipMarker);
  }

  resize(width: number, height: number): void {
    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    this.renderer.setPixelRatio(dpr);
    this.renderer.setSize(width, height, false);
    this.camera.aspect = width / height;
    this.camera.updateProjectionMatrix();
    this.sky.resize(width / height);
  }

  setCameraInput(input: Partial<CameraInput>): void {
    this.cameraInput = { ...this.cameraInput, ...input };
  }

  get cameraState(): CameraInput {
    return this.cameraInput;
  }

  setCameraMode(mode: CameraMode): void {
    this.camMode = mode;
  }

  get mode(): CameraMode {
    return this.camMode;
  }

  update(dt: number, input: SceneInput): void {
    const simDt = input.paused ? 0 : dt;
    this.lastTime += simDt;

    // The orbital frame at the vessel: radial up, prograde forward.
    this.up.copy(input.position).normalize();
    this.forward.copy(input.velocity).normalize();
    this.side.crossVectors(this.up, this.forward).normalize();

    this.ship.root.quaternion.set(
      input.orientationQuaternion.x,
      input.orientationQuaternion.y,
      input.orientationQuaternion.z,
      input.orientationQuaternion.w,
    );
    this.ship.update(dt, input.actualThrottle);
    this.ship.setGimbal(input.gimbal);

    // Earth sits opposite the vessel; the render frame origin is the vessel.
    this.earth.group.position.copy(input.position).multiplyScalar(-1);
    this.earth.update(this.lastTime);

    this.updateCamera(dt, input);
    this.updateSun(dt);

    this.trajectory.update(
      input.elements,
      input.position,
      input.planned,
      input.target,
      input.nodePosition,
      input.showTarget,
    );
    this.trajectory.sizeMarkers(this.camera);

    this.sky.follow(this.camera);
  }

  private updateSun(dt: number): void {
    void dt;
    this.sunLight.position.copy(this.sun).multiplyScalar(1e6);
    this.sunCore.position.copy(this.sun).multiplyScalar(9e6 - 1);
    this.earthShine.position.copy(this.up).multiplyScalar(-1e6);
  }

  private updateCamera(dt: number, input: SceneInput): void {
    if (input.mode !== this.camMode) this.camMode = input.mode;
    const damp = 1 - Math.exp(-Math.max(dt, 1 / 240) * 5.5);
    const ship = new Vector3(0, 0, 0);
    const e = input.elements;
    const mapScale = Math.max(Number.isFinite(e.ap) ? e.ap : e.pe, e.pe) * 2.0;

    let desiredPos = new Vector3();
    let desiredTarget = new Vector3();

    if (this.camMode === "flight") {
      const yaw = this.cameraInput.yaw + (this.cameraInput.cinematic ? Math.sin(this.lastTime * 0.05) * 0.35 : 0);
      const pitch = clamp(this.cameraInput.pitch, -0.45, 0.9);
      const dist = this.cameraInput.distance;
      desiredPos
        .copy(this.side)
        .multiplyScalar(Math.cos(yaw) * Math.cos(pitch) * dist)
        .addScaledVector(this.forward, Math.sin(yaw) * Math.cos(pitch) * dist)
        .addScaledVector(this.up, Math.sin(pitch) * dist);
      desiredTarget.copy(ship).addScaledVector(this.up, 1.2).addScaledVector(this.forward, 1.5);
    } else if (this.camMode === "chase") {
      const nose = new Vector3(0, 1, 0).applyQuaternion(this.ship.root.quaternion);
      const dorsal = new Vector3(0, 0, 1).applyQuaternion(this.ship.root.quaternion);
      const right = new Vector3(1, 0, 0).applyQuaternion(this.ship.root.quaternion);
      desiredPos
        .copy(ship)
        .addScaledVector(nose, -13)
        .addScaledVector(dorsal, 5.5)
        .addScaledVector(right, 3.5);
      desiredTarget.copy(ship).addScaledVector(nose, 8);
    } else {
      const earthCenter = this.earth.group.position;
      const normal = new Vector3().crossVectors(e.pHat, e.qHat).normalize();
      desiredPos
        .copy(earthCenter)
        .addScaledVector(normal, mapScale * 1.5)
        .addScaledVector(this.up, mapScale * 0.55);
      desiredTarget.copy(earthCenter);
    }

    // Snap on the first frame, then smooth mode changes.
    if (!this.camInitialised) {
      this.camPos.copy(desiredPos);
      this.camTarget.copy(desiredTarget);
      this.camInitialised = true;
    } else {
      this.camPos.lerp(desiredPos, damp);
      this.camTarget.lerp(desiredTarget, damp);
    }

    // Engine shake: subtle, and only when the camera is near the ship.
    this.shakeSeed += dt * 60;
    const shakeAmount = input.actualThrottle * (this.camMode === "map" ? 0 : 0.045);
    const shake = new Vector3(
      Math.sin(this.shakeSeed * 1.7) * shakeAmount,
      Math.cos(this.shakeSeed * 2.3) * shakeAmount,
      Math.sin(this.shakeSeed * 1.1) * shakeAmount,
    );

    this.camera.position.copy(this.camPos).add(shake);
    this.camera.up.copy(this.up);
    this.camera.lookAt(this.camTarget);

    this.shipMarker.visible = this.camMode === "map";
    if (this.shipMarker.visible) {
      const dist = this.camera.position.length();
      this.shipMarker.position.set(0, 0, 0);
      this.shipMarker.scale.setScalar(Math.max(dist * 0.05, 2e5));
    }

    if (this.camMode === "map") {
      this.camera.up.copy(new Vector3().crossVectors(e.pHat, e.qHat).normalize());
      this.camera.lookAt(this.camTarget);
    }

    // The sun sprite tracks the camera so its angular size stays stable.
    const sunDist = 9e6;
    this.sunSprite.position.copy(this.camera.position).addScaledVector(this.sun, sunDist);
    this.sunCore.position.copy(this.camera.position).addScaledVector(this.sun, sunDist - 1);
    this.sunSprite.scale.setScalar(sunDist * 0.22);
    this.sunCore.scale.setScalar(sunDist * 0.018);

    // Keep the planet's shader informed of the true sun direction.
    this.earth.setSun(this.sun);
  }

  /** Screen projection for HUD markers. Returns null when behind the camera. */
  project(point: Vector3): { x: number; y: number; visible: boolean } | null {
    const v = point.clone().project(this.camera);
    if (v.z < -1 || v.z > 1) return null;
    const x = (v.x * 0.5 + 0.5) * this.renderer.domElement.clientWidth;
    const y = (-v.y * 0.5 + 0.5) * this.renderer.domElement.clientHeight;
    return { x, y, visible: Math.abs(v.x) < 1.05 && Math.abs(v.y) < 1.05 };
  }

  render(): void {
    this.renderer.clear();
    this.renderer.render(this.sky.scene, this.sky.camera);
    this.renderer.clearDepth();
    this.renderer.render(this.scene, this.camera);
  }

  get sunDirection(): Vector3 {
    return this.sun.clone();
  }

  setSunDirection(dir: Vector3): void {
    this.sun.copy(dir).normalize();
    this.sunLight.position.copy(this.sun).multiplyScalar(1e6);
    this.earth.setSun(this.sun);
  }

  setOrbitLinesVisible(visible: boolean): void {
    this.trajectory.setVisible(visible);
  }

  /** Fraction of the Earth disc currently visible, used for exposure hints. */
  earthVisibility(): number {
    const d = this.earth.group.position.length();
    return smoothstep(EARTH_RADIUS * 1.2, EARTH_RADIUS * 6, d);
  }
}
