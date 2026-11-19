/* Contracts. A contract defines the orbit the tug is stranded in, the orbit the
 * operator must achieve, and how the flight is scored. Everything is derived
 * from the real state vector; there are no scripted outcomes. */

import { Vector3 } from "three";
import { EARTH_MU, EARTH_RADIUS, clamp, rad } from "./math";
import { Elements, perifocalBasis, State, periodOf } from "./orbit";

export interface TargetOrbit {
  peAlt: number;
  apAlt: number;
  inc: number;
  /** Apsidal tolerance in metres. */
  tolerance: number;
}

export interface Contract {
  id: string;
  code: string;
  name: string;
  agency: string;
  brief: string;
  detail: string;
  difficulty: string;
  reward: number;
  /** Starting mean elements. */
  start: {
    peAlt: number;
    apAlt: number;
    inc: number;
    raan: number;
    argp: number;
    nu: number;
  };
  target: TargetOrbit;
  /** Seconds of mission time before the window closes. */
  window: number;
  /** Whether the target must be circular, and how it is described on the HUD. */
  kind: "circularise" | "raise" | "deorbit";
}

export const CONTRACTS: Contract[] = [
  {
    id: "kestrel-9",
    code: "CTR-2214",
    name: "Kestrel-9",
    agency: "VASHTI LOGISTICS",
    brief: "An insertion burn cut short. The tug is falling toward the atmosphere with a full tank.",
    detail:
      "Recover the Kestrel-9 upper stage. Raise periapsis from its present low point and park the stack in the 1 200 km circular relay slot. You have one orbit and the fuel in the tank.",
    difficulty: "CADET",
    reward: 800,
    start: { peAlt: 186_000, apAlt: 1_182_000, inc: 28.5, raan: 42, argp: 18, nu: 145 },
    target: { peAlt: 1_200_000, apAlt: 1_205_000, inc: 28.5, tolerance: 45_000 },
    window: 5 * 3600,
    kind: "circularise",
  },
  {
    id: "meridian-2",
    code: "CTR-2218",
    name: "Meridian-2",
    agency: "HELIOS SURVEY",
    brief: "A survey satellite is stranded in a low relay orbit. Lift it to the high-latitude survey ring.",
    detail:
      "Meridian-2 needs a 958 km circular orbit at 51.6° inclination. Trim the apoapsis first, then close the orbit at altitude. The survey window closes in ninety minutes.",
    difficulty: "PILOT",
    reward: 1250,
    start: { peAlt: 418_000, apAlt: 424_000, inc: 51.6, raan: 118, argp: 0, nu: 12 },
    target: { peAlt: 958_000, apAlt: 962_000, inc: 51.6, tolerance: 30_000 },
    window: 90 * 60,
    kind: "raise",
  },
  {
    id: "nightjar-4",
    code: "CTR-2226",
    name: "Nightjar-4",
    agency: "ODDERON FLIGHT OFFICE",
    brief: "A spent sun-synchronous platform must come down inside the disposal corridor.",
    detail:
      "Nightjar-4 is derelict in a 320 km sun-synchronous orbit. Lower periapsis into the 45 km disposal corridor without disturbing apoapsis. The corridor must be entered on this pass.",
    difficulty: "SPECIALIST",
    reward: 1650,
    start: { peAlt: 318_000, apAlt: 324_000, inc: 98.2, raan: 246, argp: 0, nu: 62 },
    target: { peAlt: 45_000, apAlt: 324_000, inc: 98.2, tolerance: 8_000 },
    window: 45 * 60,
    kind: "deorbit",
  },
];

export function findContract(id: string): Contract {
  return CONTRACTS.find((c) => c.id === id) ?? CONTRACTS[0]!;
}

/** Build the initial inertial state for a contract. */
export function initialFlightState(contract: Contract): State {
  const s = contract.start;
  const pe = EARTH_RADIUS + s.peAlt;
  const ap = EARTH_RADIUS + s.apAlt;
  const a = (pe + ap) / 2;
  const e = (ap - pe) / (ap + pe);
  const p = a * (1 - e * e);
  const { p: pHat, q: qHat } = perifocalBasis(rad(s.raan), rad(s.inc), rad(s.argp));
  const nu = rad(s.nu);
  const r = p / (1 + e * Math.cos(nu));
  const pos = new Vector3().copy(pHat).multiplyScalar(r * Math.cos(nu)).addScaledVector(qHat, r * Math.sin(nu));
  const k = Math.sqrt(EARTH_MU / p);
  const vel = new Vector3()
    .copy(pHat)
    .multiplyScalar(-k * Math.sin(nu))
    .addScaledVector(qHat, k * (e + Math.cos(nu)));
  return { r: pos, v: vel };
}

export function targetElements(contract: Contract, source: Elements): Elements {
  const t = contract.target;
  const pe = EARTH_RADIUS + t.peAlt;
  const ap = EARTH_RADIUS + t.apAlt;
  const a = (pe + ap) / 2;
  const e = (ap - pe) / (ap + pe);
  const { p: pHat, q: qHat } = perifocalBasis(source.raan, rad(t.inc), source.argp);
  return {
    a,
    e,
    i: rad(t.inc),
    raan: source.raan,
    argp: source.argp,
    nu: 0,
    p: a * (1 - e * e),
    pe,
    ap,
    period: periodOf(a),
    energy: -EARTH_MU / (2 * a),
    h: new Vector3().crossVectors(pHat, qHat).normalize().multiplyScalar(Math.sqrt(EARTH_MU * a * (1 - e * e))),
    eVec: pHat.clone().multiplyScalar(e),
    pHat,
    qHat,
    closed: e < 1,
  };
}

export interface Evaluation {
  inTolerance: boolean;
  peError: number;
  apError: number;
  incError: number;
  accuracy: number;
  complete: boolean;
  progress: number;
}

export function evaluate(contract: Contract, el: Elements): Evaluation {
  const t = contract.target;
  const targetPe = EARTH_RADIUS + t.peAlt;
  const targetAp = EARTH_RADIUS + t.apAlt;
  const peError = el.pe - targetPe;
  const apError = el.ap - targetAp;
  const incError = Math.abs(el.i - rad(t.inc));
  const tol = t.tolerance;
  const inTolerance =
    Math.abs(peError) <= tol && Math.abs(apError) <= tol && Number.isFinite(el.period) && incError < rad(3);
  const accuracy = clamp(1 - (Math.abs(peError) + Math.abs(apError)) / (4 * tol), 0, 1);
  const progress = clamp(
    1 - (Math.abs(peError) + Math.abs(apError)) / Math.max(tol * 12, 1),
    0,
    1,
  );
  return { inTolerance, peError, apError, incError, accuracy, complete: inTolerance, progress };
}

export interface FlightResult {
  contract: Contract;
  passed: boolean;
  score: number;
  grade: string;
  peError: number;
  apError: number;
  fuelLeft: number;
  fuelUsed: number;
  missionTime: number;
  accuracy: number;
}

export function scoreFlight(
  contract: Contract,
  el: Elements,
  fuelLeftFraction: number,
  fuelUsed: number,
  missionTime: number,
): FlightResult {
  const ev = evaluate(contract, el);
  const timeFraction = clamp(1 - missionTime / contract.window, 0, 1);
  const score = Math.round(
    (0.62 * ev.accuracy + 0.24 * clamp(fuelLeftFraction, 0, 1) + 0.14 * timeFraction) * 1000,
  );
  const grade = score >= 950 ? "S" : score >= 850 ? "A" : score >= 700 ? "B" : score >= 550 ? "C" : "D";
  return {
    contract,
    passed: ev.inTolerance,
    score,
    grade,
    peError: ev.peError,
    apError: ev.apError,
    fuelLeft: fuelLeftFraction,
    fuelUsed,
    missionTime,
    accuracy: ev.accuracy,
  };
}
