/* Deep sky. A sparse hemisphere of stars, a faint galactic band and a few
 * hero stars, all generated at startup. Rendered in its own scene so the star
 * sphere never fights the planet for depth precision. */

import {
  AdditiveBlending,
  BufferAttribute,
  BufferGeometry,
  Color,
  PerspectiveCamera,
  Points,
  PointsMaterial,
  Scene,
  Vector3,
} from "three";
import { mulberry32 } from "../core/math";
import { starDotTexture } from "./textures";

function direction(rand: () => number): Vector3 {
  const u = rand() * 2 - 1;
  const phi = rand() * Math.PI * 2;
  const s = Math.sqrt(Math.max(0, 1 - u * u));
  return new Vector3(s * Math.cos(phi), u, s * Math.sin(phi));
}

export class Sky {
  readonly scene = new Scene();
  readonly camera = new PerspectiveCamera(62, 1, 1, 60_000);
  private readonly stars: Points;
  private readonly galaxy: Points;
  private readonly radius = 20_000;

  constructor() {
    const rand = mulberry32(0x51a72);
    const positions: number[] = [];
    const colors: number[] = [];

    for (let i = 0; i < 2600; i++) {
      const d = direction(rand).multiplyScalar(this.radius * (0.85 + rand() * 0.15));
      positions.push(d.x, d.y, d.z);
      // Power-law brightness: mostly faint, a handful of bright anchors.
      const mag = Math.pow(rand(), 3.2);
      const c = new Color().setHSL(0.55 + rand() * 0.14, 0.25 + mag * 0.4, 0.52 + mag * 0.45);
      colors.push(c.r, c.g, c.b);
    }

    const geo = new BufferGeometry();
    geo.setAttribute("position", new BufferAttribute(new Float32Array(positions), 3));
    geo.setAttribute("color", new BufferAttribute(new Float32Array(colors), 3));
    const mat = new PointsMaterial({
      size: 2.2,
      sizeAttenuation: false,
      map: starDotTexture(),
      transparent: true,
      depthWrite: false,
      blending: AdditiveBlending,
      vertexColors: true,
      opacity: 0.95,
    });
    this.stars = new Points(geo, mat);
    this.stars.frustumCulled = false;

    // Galactic band: points concentrated around a tilted plane.
    const bandPositions: number[] = [];
    const bandColors: number[] = [];
    const tilt = new Vector3(1, 0.35, 0.2).normalize();
    const basisA = new Vector3().crossVectors(tilt, new Vector3(0, 1, 0)).normalize();
    const basisB = new Vector3().crossVectors(tilt, basisA).normalize();
    for (let i = 0; i < 2200; i++) {
      const theta = rand() * Math.PI * 2;
      const spread = (rand() + rand() + rand() - 1.5) * 0.20;
      const d = new Vector3()
        .addScaledVector(basisA, Math.cos(theta))
        .addScaledVector(basisB, Math.sin(theta))
        .addScaledVector(tilt, spread)
        .normalize()
        .multiplyScalar(this.radius * 0.9);
      bandPositions.push(d.x, d.y, d.z);
      const v = 0.22 + rand() * 0.20;
      bandColors.push(v * 0.75, v * 0.82, v);
    }
    const bandGeo = new BufferGeometry();
    bandGeo.setAttribute("position", new BufferAttribute(new Float32Array(bandPositions), 3));
    bandGeo.setAttribute("color", new BufferAttribute(new Float32Array(bandColors), 3));
    this.galaxy = new Points(
      bandGeo,
      new PointsMaterial({
        size: 1.5,
        sizeAttenuation: false,
        map: starDotTexture(),
        transparent: true,
        depthWrite: false,
        blending: AdditiveBlending,
        vertexColors: true,
        opacity: 0.55,
      }),
    );
    this.galaxy.frustumCulled = false;

    this.scene.add(this.stars, this.galaxy);
  }

  resize(aspect: number): void {
    this.camera.aspect = aspect;
    this.camera.updateProjectionMatrix();
  }

  /** The sky is at infinity: it inherits rotation only, never translation. */
  follow(camera: PerspectiveCamera): void {
    this.camera.quaternion.copy(camera.quaternion);
    this.camera.fov = camera.fov;
    this.camera.aspect = camera.aspect;
    this.camera.updateProjectionMatrix();
  }
}
