/* Orbital geometry in the vessel-centred render frame: the orbit the ship is
 * on, the orbit a planned burn would produce, the contract's target ring, and
 * apsis markers. All points are shifted by -r each rebuild so the ship stays at
 * the origin and float precision stays usable. */

import {
  BufferAttribute,
  BufferGeometry,
  Camera,
  Group,
  Line,
  LineBasicMaterial,
  LineDashedMaterial,
  Sprite,
  SpriteMaterial,
  Vector3,
} from "three";
import { Elements, apsisStates, sampleConic } from "../core/orbit";
import { markerTexture } from "./textures";

interface Marker {
  sprite: Sprite;
  position: Vector3;
  visible: boolean;
}

export class Trajectory {
  readonly group = new Group();
  private readonly currentLine: Line;
  private readonly plannedLine: Line;
  private readonly targetLine: Line;
  private readonly currentMat: LineBasicMaterial;
  private readonly plannedMat: LineBasicMaterial;
  private readonly targetMat: LineDashedMaterial;
  private readonly apMarker: Marker;
  private readonly peMarker: Marker;
  private readonly targetApMarker: Marker;
  private readonly targetPeMarker: Marker;
  private readonly nodeMarker: Marker;
  private lastKey = "";
  private readonly tmp = new Vector3();

  constructor() {
    this.currentMat = new LineBasicMaterial({
      color: 0x5fe9ff,
      transparent: true,
      opacity: 0.80,
      depthWrite: false,
    });
    this.plannedMat = new LineBasicMaterial({
      color: 0xffb43f,
      transparent: true,
      opacity: 0.95,
      depthWrite: false,
    });
    this.targetMat = new LineDashedMaterial({
      color: 0x57e39b,
      transparent: true,
      opacity: 0.55,
      dashSize: 220_000,
      gapSize: 160_000,
      depthWrite: false,
    });

    this.currentLine = new Line(new BufferGeometry(), this.currentMat);
    this.plannedLine = new Line(new BufferGeometry(), this.plannedMat);
    this.targetLine = new Line(new BufferGeometry(), this.targetMat);
    for (const l of [this.currentLine, this.plannedLine, this.targetLine]) {
      l.frustumCulled = false;
      l.renderOrder = 2;
    }

    const makeMarker = (label: string, color: string): Marker => {
      const sprite = new Sprite(
        new SpriteMaterial({
          map: markerTexture(label, color),
          transparent: true,
          depthWrite: false,
          depthTest: false,
          opacity: 0.95,
        }),
      );
      sprite.renderOrder = 10;
      sprite.scale.setScalar(400_000);
      sprite.visible = false;
      return { sprite, position: new Vector3(), visible: false };
    };

    this.apMarker = makeMarker("AP", "#ffca6a");
    this.peMarker = makeMarker("PE", "#8ff3ff");
    this.targetApMarker = makeMarker("AP·TGT", "#7dffb8");
    this.targetPeMarker = makeMarker("PE·TGT", "#7dffb8");
    this.nodeMarker = makeMarker("NODE", "#ffb43f");

    this.group.add(
      this.currentLine,
      this.plannedLine,
      this.targetLine,
      this.apMarker.sprite,
      this.peMarker.sprite,
      this.targetApMarker.sprite,
      this.targetPeMarker.sprite,
      this.nodeMarker.sprite,
    );
  }

  private setLine(line: Line, points: Vector3[], offset: Vector3): void {
    const arr = new Float32Array(points.length * 3);
    for (let i = 0; i < points.length; i++) {
      const p = points[i];
      arr[i * 3] = p.x - offset.x;
      arr[i * 3 + 1] = p.y - offset.y;
      arr[i * 3 + 2] = p.z - offset.z;
    }
    const geo = new BufferGeometry();
    geo.setAttribute("position", new BufferAttribute(arr, 3));
    line.geometry.dispose();
    line.geometry = geo;
    if (line === this.targetLine) line.computeLineDistances();
  }

  private setMarker(marker: Marker, pos: Vector3 | null, offset: Vector3): void {
    if (!pos) {
      marker.visible = false;
      marker.sprite.visible = false;
      return;
    }
    marker.position.copy(pos).sub(offset);
    marker.sprite.position.copy(marker.position);
    marker.visible = true;
    marker.sprite.visible = true;
  }

  /**
   * Rebuild the geometry. Cost is only paid when the orbital state or the plan
   * changes meaningfully, which is at most a few times a second.
   */
  update(
    current: Elements,
    offset: Vector3,
    planned: Elements | null,
    target: Elements | null,
    nodePosition: Vector3 | null,
    showTarget: boolean,
  ): void {
    const key = [
      current.p.toFixed(0),
      current.e.toFixed(6),
      current.i.toFixed(5),
      current.raan.toFixed(5),
      current.argp.toFixed(5),
      planned ? `${planned.p.toFixed(0)}:${planned.e.toFixed(5)}:${planned.raan.toFixed(4)}` : "-",
      showTarget ? "t" : "-",
      nodePosition ? `${Math.round(nodePosition.x / 1000)},${Math.round(nodePosition.y / 1000)},${Math.round(nodePosition.z / 1000)}` : "-",
      Math.round(offset.x / 1000),
      Math.round(offset.y / 1000),
      Math.round(offset.z / 1000),
    ].join("|");
    if (key === this.lastKey) return;
    this.lastKey = key;

    this.setLine(this.currentLine, sampleConic(current, 320), offset);
    if (planned) {
      this.setLine(this.plannedLine, sampleConic(planned, 320), offset);
      this.plannedLine.visible = true;
    } else {
      this.plannedLine.visible = false;
    }
    if (target && showTarget) {
      this.setLine(this.targetLine, sampleConic(target, 320), offset);
      this.targetLine.visible = true;
      const tAp = apsisStates(target);
      this.setMarker(this.targetApMarker, tAp.ap, offset);
      this.setMarker(this.targetPeMarker, tAp.pe, offset);
    } else {
      this.targetLine.visible = false;
      this.setMarker(this.targetApMarker, null, offset);
      this.setMarker(this.targetPeMarker, null, offset);
    }

    const ap = apsisStates(current);
    this.setMarker(this.apMarker, ap.ap, offset);
    this.setMarker(this.peMarker, ap.pe, offset);
    this.setMarker(this.nodeMarker, nodePosition, offset);
  }

  /** Screen-space size markers so they stay legible at any camera distance. */
  sizeMarkers(camera: Camera): void {
    const all = [this.apMarker, this.peMarker, this.targetApMarker, this.targetPeMarker, this.nodeMarker];
    for (const m of all) {
      if (!m.visible) continue;
      const d = this.tmp.copy(m.position).sub(camera.position).length();
      m.sprite.scale.setScalar(Math.max(d * 0.028, 120_000));
    }
  }

  setPlannedVisible(visible: boolean): void {
    this.plannedLine.visible = visible;
  }

  setCurrentVisible(visible: boolean): void {
    this.currentLine.visible = visible;
  }

  setVisible(visible: boolean): void {
    this.group.visible = visible;
  }
}
