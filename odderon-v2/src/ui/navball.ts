/* The attitude ball. A projected sphere, not a decorative dial: the horizon,
 * latitude ladder, heading marks and velocity vectors are all derived from the
 * vessel's real quaternion and state vector.
 *
 * Projection: a world direction is expressed in the vessel's body frame
 * (right, nose, dorsal) and drawn orthographically. The ball content is rotated
 * by the roll angle so the local vertical reads "up"; markers are drawn in the
 * same rotated frame, then the fixed aircraft symbol is drawn unrotated. */

import { Quaternion, Vector3 } from "three";
import { clamp } from "../core/math";

export interface AttitudeInfo {
  pitch: number;
  heading: number;
  roll: number;
}

/** Pitch, heading and roll of the nose in the local orbital frame. */
export function attitudeInfo(orientation: Quaternion, position: Vector3, velocity: Vector3): AttitudeInfo {
  const up = position.clone().normalize();
  const north = new Vector3().crossVectors(position, velocity).normalize();
  const east = new Vector3().crossVectors(north, up).normalize();
  const nose = new Vector3(0, 1, 0).applyQuaternion(orientation);
  const dorsal = new Vector3(0, 0, 1).applyQuaternion(orientation);
  const pitch = Math.asin(clamp(nose.dot(up), -1, 1));
  const heading = Math.atan2(nose.dot(east), nose.dot(north));
  const upRef = up.clone().addScaledVector(nose, -up.dot(nose));
  if (upRef.lengthSq() < 1e-8) upRef.copy(east);
  upRef.normalize();
  const vesselUp = dorsal.clone().addScaledVector(nose, -dorsal.dot(nose));
  if (vesselUp.lengthSq() < 1e-8) vesselUp.copy(upRef);
  vesselUp.normalize();
  const roll = Math.atan2(new Vector3().crossVectors(upRef, vesselUp).dot(nose), upRef.dot(vesselUp));
  return { pitch, heading, roll };
}

export class Navball {
  private readonly canvas: HTMLCanvasElement;
  private readonly ctx: CanvasRenderingContext2D;
  private readonly right = new Vector3();
  private readonly nose = new Vector3();
  private readonly dorsal = new Vector3();
  private readonly up = new Vector3();
  private readonly north = new Vector3();
  private readonly east = new Vector3();
  private readonly prograde = new Vector3();
  private readonly normal = new Vector3();
  private readonly tmp = new Vector3();
  private size = 230;

  constructor(canvas: HTMLCanvasElement) {
    this.canvas = canvas;
    const ctx = canvas.getContext("2d");
    if (!ctx) throw new Error("2D canvas is unavailable");
    this.ctx = ctx;
  }

  resize(cssSize: number): void {
    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    this.size = Math.max(120, Math.floor(cssSize));
    this.canvas.width = Math.floor(this.size * dpr);
    this.canvas.height = Math.floor(this.size * dpr);
  }

  private project(w: Vector3, r: number): { x: number; y: number; front: boolean } {
    const bx = w.dot(this.right);
    const by = w.dot(this.nose);
    const bz = w.dot(this.dorsal);
    return { x: bx * r, y: -bz * r, front: by > 0.02 };
  }

  render(orientation: Quaternion, position: Vector3, velocity: Vector3): void {
    const { ctx, canvas } = this;
    const S = canvas.width;
    const cssSize = this.size;
    const scale = S / cssSize;
    ctx.setTransform(scale, 0, 0, scale, 0, 0);
    ctx.clearRect(0, 0, cssSize, cssSize);

    const cx = cssSize / 2;
    const cy = cssSize / 2;
    const R = cssSize / 2 - 3;

    this.right.set(1, 0, 0).applyQuaternion(orientation);
    this.nose.set(0, 1, 0).applyQuaternion(orientation);
    this.dorsal.set(0, 0, 1).applyQuaternion(orientation);
    this.up.copy(position).normalize();
    this.north.crossVectors(position, velocity).normalize();
    this.east.crossVectors(this.north, this.up).normalize();
    this.prograde.copy(velocity).normalize();
    this.normal.copy(this.north);

    const ux = this.up.dot(this.right);
    const uy = this.up.dot(this.nose);
    const uz = this.up.dot(this.dorsal);
    const pitch = Math.asin(clamp(uy, -1, 1));
    const theta = Math.atan2(ux, uz);

    // The ball: clip to the sphere's silhouette.
    ctx.save();
    ctx.translate(cx, cy);
    ctx.beginPath();
    ctx.arc(0, 0, R, 0, Math.PI * 2);
    ctx.clip();

    // Everything that belongs to the world rotates with roll.
    ctx.save();
    ctx.rotate(-theta);

    const skyGrad = ctx.createLinearGradient(0, -R, 0, R);
    skyGrad.addColorStop(0, "#2b709f");
    skyGrad.addColorStop(0.55, "#123d5f");
    skyGrad.addColorStop(1, "#0a2036");
    ctx.fillStyle = skyGrad;
    ctx.fillRect(-R, -R, R * 2, R * 2);

    const sp = Math.sin(pitch);
    const steps = 72;
    ctx.beginPath();
    for (let i = 0; i <= steps; i++) {
      const x = -R + (2 * R * i) / steps;
      const y = sp * Math.sqrt(Math.max(0, R * R - x * x));
      if (i === 0) ctx.moveTo(x, y);
      else ctx.lineTo(x, y);
    }
    ctx.lineTo(R, R);
    ctx.lineTo(-R, R);
    ctx.closePath();
    const groundGrad = ctx.createLinearGradient(0, -R, 0, R);
    groundGrad.addColorStop(0, "#4c3b22");
    groundGrad.addColorStop(0.6, "#3a2d1a");
    groundGrad.addColorStop(1, "#241c11");
    ctx.fillStyle = groundGrad;
    ctx.fill();

    // Latitude / meridian grid.
    const gridPoint = (latDeg: number, lonDeg: number): Vector3 => {
      const lat = (latDeg * Math.PI) / 180;
      const lon = (lonDeg * Math.PI) / 180;
      return this.up
        .clone()
        .multiplyScalar(Math.sin(lat))
        .addScaledVector(this.north, Math.cos(lat) * Math.cos(lon))
        .addScaledVector(this.east, Math.cos(lat) * Math.sin(lon));
    };
    const drawArc = (points: Vector3[], color: string, width: number): void => {
      ctx.strokeStyle = color;
      ctx.lineWidth = width;
      ctx.beginPath();
      let started = false;
      for (const p of points) {
        const pr = this.project(p, R);
        if (!pr.front) {
          started = false;
          continue;
        }
        if (!started) {
          ctx.moveTo(pr.x, pr.y);
          started = true;
        } else ctx.lineTo(pr.x, pr.y);
      }
      ctx.stroke();
    };

    for (let lat = -75; lat <= 75; lat += 15) {
      if (Math.abs(lat) < 0.001) continue;
      const pts: Vector3[] = [];
      for (let lon = 0; lon <= 360; lon += 5) pts.push(gridPoint(lat, lon));
      drawArc(pts, "rgba(190,240,255,0.16)", 0.6);
    }
    for (let lon = 0; lon < 360; lon += 30) {
      const pts: Vector3[] = [];
      for (let lat = -90; lat <= 90; lat += 3) pts.push(gridPoint(lat, lon));
      drawArc(pts, "rgba(190,240,255,0.12)", 0.55);
    }

    // Emphasised horizon.
    ctx.beginPath();
    for (let i = 0; i <= steps; i++) {
      const x = -R + (2 * R * i) / steps;
      const y = sp * Math.sqrt(Math.max(0, R * R - x * x));
      if (i === 0) ctx.moveTo(x, y);
      else ctx.lineTo(x, y);
    }
    ctx.strokeStyle = "rgba(228,249,255,0.82)";
    ctx.lineWidth = 1.1;
    ctx.stroke();

    // Pitch numbers along the north meridian.
    ctx.font = "500 6.5px 'Data', monospace";
    ctx.textAlign = "center";
    ctx.textBaseline = "middle";
    ctx.fillStyle = "rgba(228,249,255,0.52)";
    for (const lat of [-60, -45, -30, -15, 15, 30, 45, 60]) {
      const pr = this.project(gridPoint(lat, 0), R);
      if (!pr.front || Math.hypot(pr.x, pr.y) > R * 0.80) continue;
      ctx.fillText(`${lat > 0 ? "+" : ""}${lat}`, pr.x, pr.y);
    }

    // Velocity and reference vectors, in the rolled ball frame.
    const mark = (w: Vector3, kind: "pro" | "retro" | "radial" | "normal", color: string): void => {
      const pr = this.project(w, R);
      if (!pr.front || Math.hypot(pr.x, pr.y) > R - 3) return;
      ctx.strokeStyle = color;
      ctx.fillStyle = color;
      ctx.lineWidth = 1.1;
      if (kind === "pro") {
        ctx.beginPath();
        ctx.arc(pr.x, pr.y, 4.4, 0, Math.PI * 2);
        ctx.stroke();
        ctx.beginPath();
        ctx.arc(pr.x, pr.y, 1.4, 0, Math.PI * 2);
        ctx.fill();
        ctx.beginPath();
        ctx.moveTo(pr.x - 8, pr.y);
        ctx.lineTo(pr.x - 5.6, pr.y);
        ctx.moveTo(pr.x + 5.6, pr.y);
        ctx.lineTo(pr.x + 8, pr.y);
        ctx.moveTo(pr.x, pr.y - 8);
        ctx.lineTo(pr.x, pr.y - 5.6);
        ctx.stroke();
      } else if (kind === "retro") {
        ctx.beginPath();
        ctx.moveTo(pr.x - 4, pr.y - 4);
        ctx.lineTo(pr.x + 4, pr.y + 4);
        ctx.moveTo(pr.x + 4, pr.y - 4);
        ctx.lineTo(pr.x - 4, pr.y + 4);
        ctx.stroke();
      } else if (kind === "radial") {
        ctx.beginPath();
        ctx.arc(pr.x, pr.y, 3.6, 0, Math.PI * 2);
        ctx.stroke();
        ctx.beginPath();
        ctx.moveTo(pr.x, pr.y - 3.6);
        ctx.lineTo(pr.x, pr.y - 7);
        ctx.stroke();
      } else {
        ctx.beginPath();
        ctx.moveTo(pr.x, pr.y - 5);
        ctx.lineTo(pr.x + 5, pr.y);
        ctx.lineTo(pr.x, pr.y + 5);
        ctx.lineTo(pr.x - 5, pr.y);
        ctx.closePath();
        ctx.stroke();
      }
    };

    mark(this.prograde, "pro", "#7dffb0");
    mark(this.tmp.copy(this.prograde).multiplyScalar(-1), "retro", "#7dffb0");
    mark(this.up, "radial", "#ffcc6a");
    mark(this.tmp.copy(this.up).multiplyScalar(-1), "radial", "#ffcc6a");
    mark(this.normal, "normal", "#c9a3ff");
    mark(this.tmp.copy(this.normal).multiplyScalar(-1), "normal", "#c9a3ff");

    // Sphere shading.
    const shade = ctx.createRadialGradient(-R * 0.35, -R * 0.42, R * 0.08, 0, 0, R * 1.02);
    shade.addColorStop(0, "rgba(255,255,255,0.14)");
    shade.addColorStop(0.5, "rgba(255,255,255,0.015)");
    shade.addColorStop(1, "rgba(0,0,0,0.52)");
    ctx.fillStyle = shade;
    ctx.fillRect(-R, -R, R * 2, R * 2);

    ctx.restore(); // end rolled frame

    ctx.restore(); // end clip

    // Rim.
    ctx.beginPath();
    ctx.arc(cx, cy, R, 0, Math.PI * 2);
    ctx.strokeStyle = "rgba(150,220,235,0.42)";
    ctx.lineWidth = 1;
    ctx.stroke();
    ctx.beginPath();
    ctx.arc(cx, cy, R + 2.4, 0, Math.PI * 2);
    ctx.strokeStyle = "rgba(99,226,244,0.13)";
    ctx.stroke();

    // Fixed aircraft reference.
    ctx.strokeStyle = "rgba(255,214,140,0.95)";
    ctx.lineWidth = 1.2;
    ctx.beginPath();
    ctx.moveTo(cx - 11, cy);
    ctx.lineTo(cx - 4, cy);
    ctx.lineTo(cx - 2, cy + 3);
    ctx.moveTo(cx + 11, cy);
    ctx.lineTo(cx + 4, cy);
    ctx.lineTo(cx + 2, cy + 3);
    ctx.moveTo(cx, cy - 8);
    ctx.lineTo(cx, cy - 3);
    ctx.stroke();
  }
}
