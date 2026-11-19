/* The vehicle: a single-stage orbital tug with a pressure-fed vacuum engine,
 * reaction wheels, RCS and a finite propellant load.
 *
 * Mass, thrust and flow rate are physically linked (ṁ = T / (Isp·g₀)), so the
 * Δv the HUD predicts is the Δv the tank can actually pay for. Attitude is a
 * rigid body integrated with a quaternion; SAS is a real controller, not a
 * hard snap. */

import { Quaternion, Vector3 } from "three";
import { G0, clamp, EARTH_MU } from "./math";
import { basisQuaternion, elementsFromState, State } from "./orbit";

export type SasMode = "off" | "stability" | "prograde" | "retrograde" | "radial" | "normal" | "node";

export interface VesselSpec {
  name: string;
  dryMass: number;
  propellant: number;
  thrust: number;
  isp: number;
  /** Rigid-body inertia about each control axis, kg·m². */
  inertia: number;
  /** Reaction-wheel torque, N·m. RCS triples it. */
  wheelTorque: number;
  maxRate: number;
}

export const KESTREL: VesselSpec = {
  name: "KESTREL-9",
  dryMass: 1650,
  propellant: 2500,
  thrust: 64_000,
  isp: 340,
  inertia: 12_500,
  wheelTorque: 2400,
  maxRate: 0.55,
};

export interface VesselInput {
  pitch: number;
  yaw: number;
  roll: number;
  throttle: number;
  engineArmed: boolean;
}

export class Vessel {
  spec: VesselSpec;
  state: State;
  orientation: Quaternion;
  angularVelocity = new Vector3();
  propellant: number;
  throttle = 0;
  engineArmed = false;
  /** Visual/audio spool value, 0..1. Not used by physics. */
  spool = 0;
  sas: SasMode = "prograde";
  rcs = false;
  /** Commanded attitude, used by SAS and the node autopilot. */
  targetAttitude: Quaternion | null = null;
  /** Gimbal deflection for the renderer, radians. */
  gimbal = 0;
  fuelUsed = 0;
  burnTime = 0;

  constructor(spec: VesselSpec, state: State, orientation: Quaternion) {
    this.spec = spec;
    this.state = { r: state.r.clone(), v: state.v.clone() };
    this.orientation = orientation.clone().normalize();
    this.propellant = spec.propellant;
  }

  get mass(): number {
    return this.spec.dryMass + Math.max(0, this.propellant);
  }

  get thrust(): number {
    return this.engineArmed ? this.spec.thrust * this.throttle : 0;
  }

  get massFlow(): number {
    return this.thrust / (this.spec.isp * G0);
  }

  get deltaVRemaining(): number {
    return this.spec.isp * G0 * Math.log(this.mass / this.spec.dryMass);
  }

  get maxAccel(): number {
    return this.spec.thrust / this.mass;
  }

  get fuelFraction(): number {
    return this.propellant / this.spec.propellant;
  }

  get elements() {
    return elementsFromState(this.state.r, this.state.v, EARTH_MU);
  }

  /** Nose direction in the inertial frame (+Y body axis). */
  nose(): Vector3 {
    return new Vector3(0, 1, 0).applyQuaternion(this.orientation);
  }

  burnDirection(): Vector3 {
    return this.nose();
  }

  setThrottle(t: number): void {
    this.throttle = clamp(t, 0, 1);
  }

  /** One control tick. Returns the thrust acceleration vector for the force
   *  model; orbital integration is done by the caller with RK4. */
  update(dt: number, input: VesselInput): Vector3 {
    if (!(dt > 0) || !Number.isFinite(dt)) return new Vector3();
    this.engineArmed = input.engineArmed && this.propellant > 0;

    // ---- propellant and thrust
    const requestedThrottle = clamp(input.throttle, 0, 1);
    this.throttle = this.engineArmed ? requestedThrottle : 0;
    let thrust = this.thrust;
    if (this.propellant <= 0) {
      this.propellant = 0;
      thrust = 0;
      this.throttle = 0;
    }
    const mdot = thrust / (this.spec.isp * G0);
    const burn = Math.min(this.propellant, mdot * dt);
    if (mdot > 0) {
      this.propellant -= burn;
      this.fuelUsed += burn;
      this.burnTime += dt;
    }
    this.spool += ((thrust > 0 ? 1 : 0) - this.spool) * clamp(dt * 6, 0, 1);
    const accel = this.burnDirection().multiplyScalar(thrust / Math.max(this.mass, 1));

    // ---- attitude control
    if (this.sas !== "node") this.updateAttitudeTarget();

    const torque = new Vector3();
    // Torque is in N·m; the angular acceleration below divides by inertia once.
    const wheel = this.spec.wheelTorque * (this.rcs ? 3.2 : 1);

    const stick = new Vector3(
      clamp(input.pitch, -1, 1),
      clamp(input.roll, -1, 1),
      clamp(input.yaw, -1, 1),
    );
    if (stick.lengthSq() > 0) {
      // Local X = pitch, Y = roll, Z = yaw, re-expressed in the inertial frame.
      const localTorque = new Vector3(stick.x, stick.y, stick.z).multiplyScalar(wheel * 0.9);
      torque.add(localTorque.applyQuaternion(this.orientation));
      this.targetAttitude = null;
    }

    if (this.targetAttitude && stick.lengthSq() === 0) {
      const err = this.targetAttitude.clone().multiply(this.orientation.clone().invert());
      if (err.w < 0) {
        err.x = -err.x;
        err.y = -err.y;
        err.z = -err.z;
        err.w = -err.w;
      }
      const angle = 2 * Math.acos(clamp(err.w, -1, 1));
      const axis = new Vector3(err.x, err.y, err.z);
      if (angle > 1e-4 && axis.lengthSq() > 1e-12) {
        axis.normalize();
        const kp = 2.6;
        const kd = 1.6;
        torque.addScaledVector(axis, Math.min(angle * kp, 1.25) * wheel);
        torque.addScaledVector(this.angularVelocity, -kd * wheel);
      } else {
        torque.addScaledVector(this.angularVelocity, -3.2 * wheel);
      }
    } else if (this.sas !== "off" && stick.lengthSq() === 0) {
      torque.addScaledVector(this.angularVelocity, -1.1 * wheel);
    }

    const limit = wheel * 1.4;
    if (torque.length() > limit) torque.multiplyScalar(limit / torque.length());

    this.angularVelocity.addScaledVector(torque, dt / this.spec.inertia);
    const rate = this.angularVelocity.length();
    if (rate > this.spec.maxRate) this.angularVelocity.multiplyScalar(this.spec.maxRate / rate);
    if (this.angularVelocity.length() < 1e-5) this.angularVelocity.set(0, 0, 0);

    const omega = this.angularVelocity;
    const dq = new Quaternion(omega.x * 0.5 * dt, omega.y * 0.5 * dt, omega.z * 0.5 * dt, 0).multiply(
      this.orientation,
    );
    this.orientation.set(
      this.orientation.x + dq.x,
      this.orientation.y + dq.y,
      this.orientation.z + dq.z,
      this.orientation.w + dq.w,
    ).normalize();

    this.gimbal += (clamp(input.yaw, -1, 1) * 0.035 - this.gimbal) * clamp(dt * 4, 0, 1);
    return accel;
  }

  /** Resolve the SAS command into a target attitude for the current state. */
  updateAttitudeTarget(): void {
    const { r, v } = this.state;
    const h = new Vector3().crossVectors(r, v).normalize();
    const prograde = v.clone().normalize();
    const radial = r.clone().normalize();
    switch (this.sas) {
      case "prograde":
        this.targetAttitude = basisQuaternion(prograde, radial);
        break;
      case "retrograde":
        this.targetAttitude = basisQuaternion(prograde.multiplyScalar(-1), radial);
        break;
      case "radial":
        this.targetAttitude = basisQuaternion(radial, prograde.clone().multiplyScalar(-1));
        break;
      case "normal":
        this.targetAttitude = basisQuaternion(h, radial);
        break;
      case "stability":
      case "off":
        this.targetAttitude = null;
        break;
      case "node":
        // Held by the manoeuvre autopilot.
        break;
    }
  }

  alignError(): number {
    if (!this.targetAttitude) return 0;
    const err = this.targetAttitude.clone().multiply(this.orientation.clone().invert());
    if (err.w < 0) err.set(-err.x, -err.y, -err.z, -err.w);
    return 2 * Math.acos(clamp(err.w, -1, 1));
  }
}
