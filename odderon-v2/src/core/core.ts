/* Bridge to the Rust flight core.
 *
 * The core is a dependency-free crate compiled to wasm32 (`rust/`). It has no
 * wasm-bindgen glue: exports take f64 arguments, write their result into a
 * shared 64-double scratch buffer, and return how many doubles they wrote. We
 * read them back through a Float64Array view over the module's memory.
 *
 * If the module is missing, fails to instantiate, or reports an unexpected
 * ABI version, every wrapper returns null and the TypeScript implementation in
 * `orbit.ts` takes over. The game must never fail to boot because of this
 * file.
 */

import { CORE_WASM_BASE64 } from "./core-bytes";

type Exports = {
  memory: WebAssembly.Memory;
  oc_scratch_ptr: () => number;
  oc_version: () => number;
  oc_self_test: () => number;
  oc_elements: (
    rx: number, ry: number, rz: number,
    vx: number, vy: number, vz: number,
    mu: number,
  ) => number;
  oc_state_from_elements: (
    px: number, py: number, pz: number,
    qx: number, qy: number, qz: number,
    p: number, e: number, nu: number, mu: number,
  ) => number;
  oc_kepler: (
    rx: number, ry: number, rz: number,
    vx: number, vy: number, vz: number,
    dt: number, mu: number,
  ) => number;
  oc_rk4: (
    rx: number, ry: number, rz: number,
    vx: number, vy: number, vz: number,
    dt: number, ax: number, ay: number, az: number,
    j2: number, mu: number,
  ) => number;
  oc_time_to_apsis: (a: number, e: number, nu: number, apoapsis: number, mu: number) => number;
  oc_score: (pe: number, ap: number, tol: number, fuel: number, time: number) => number;
  oc_accuracy: (pe: number, ap: number, tol: number) => number;
};

const EXPECTED_VERSION = 2;
const SLOTS = 64;

let exports: Exports | null = null;
let view: Float64Array | null = null;
let status: "idle" | "ready" | "unavailable" = "idle";

function decode(base64: string): Uint8Array<ArrayBuffer> {
  if (typeof atob === "function") {
    const binary = atob(base64);
    const out = new Uint8Array(binary.length);
    for (let i = 0; i < binary.length; i++) out[i] = binary.charCodeAt(i);
    return out;
  }
  // Node (harnesses) falls back to Buffer.
  const buf = (globalThis as { Buffer?: { from(s: string, e: string): Uint8Array } }).Buffer;
  if (!buf) throw new Error("no base64 decoder available");
  return buf.from(base64, "base64") as Uint8Array<ArrayBuffer>;
}

/** Instantiate the core. Safe to call more than once. */
export function bootCore(): boolean {
  if (status !== "idle") return status === "ready";
  status = "unavailable";
  try {
    if (!CORE_WASM_BASE64) return false;
    const forced = typeof location !== "undefined" && new URLSearchParams(location.search).get("core") === "ts";
    if (forced) return false;
    const module = new WebAssembly.Module(decode(CORE_WASM_BASE64));
    const instance = new WebAssembly.Instance(module, {});
    const api = instance.exports as unknown as Exports;
    if (typeof api.oc_scratch_ptr !== "function") return false;
    if (api.oc_version() !== EXPECTED_VERSION) return false;
    if (api.oc_self_test() !== 1) return false;
    exports = api;
    view = new Float64Array(api.memory.buffer, api.oc_scratch_ptr(), SLOTS);
    status = "ready";
    return true;
  } catch {
    exports = null;
    view = null;
    status = "unavailable";
    return false;
  }
}

export function coreReady(): boolean {
  return status === "ready";
}

export function coreStatus(): "idle" | "ready" | "unavailable" {
  return status;
}

/** Read `count` doubles written by the last call. The memory view is rebuilt
 *  each time because a growing memory detaches the old ArrayBuffer. */
function read(count: number): Float64Array | null {
  if (!exports) return null;
  if (!view || view.buffer !== exports.memory.buffer) {
    view = new Float64Array(exports.memory.buffer, exports.oc_scratch_ptr(), SLOTS);
  }
  if (count <= 0 || count > SLOTS) return null;
  return view.slice(0, count);
}

/** 23 doubles: a, e, i, raan, argp, nu, p, pe, ap, period, energy, h, e_vec,
 *  p_hat, q_hat. */
export function coreElements(
  rx: number, ry: number, rz: number,
  vx: number, vy: number, vz: number,
  mu: number,
): Float64Array | null {
  if (!exports) return null;
  try {
    return read(exports.oc_elements(rx, ry, rz, vx, vy, vz, mu));
  } catch {
    return null;
  }
}

export function coreKepler(
  rx: number, ry: number, rz: number,
  vx: number, vy: number, vz: number,
  dt: number, mu: number,
): Float64Array | null {
  if (!exports) return null;
  try {
    return read(exports.oc_kepler(rx, ry, rz, vx, vy, vz, dt, mu));
  } catch {
    return null;
  }
}

export function coreRk4(
  rx: number, ry: number, rz: number,
  vx: number, vy: number, vz: number,
  dt: number,
  ax: number, ay: number, az: number,
  j2: boolean, mu: number,
): Float64Array | null {
  if (!exports) return null;
  try {
    return read(exports.oc_rk4(rx, ry, rz, vx, vy, vz, dt, ax, ay, az, j2 ? 1 : 0, mu));
  } catch {
    return null;
  }
}

export function coreTimeToApsis(
  a: number, e: number, nu: number, apoapsis: boolean, mu: number,
): number | null {
  if (!exports) return null;
  try {
    const t = exports.oc_time_to_apsis(a, e, nu, apoapsis ? 1 : 0, mu);
    return Number.isFinite(t) ? t : null;
  } catch {
    return null;
  }
}

export function coreScore(
  peError: number, apError: number, tolerance: number, fuelLeft: number, timeFraction: number,
): number | null {
  if (!exports) return null;
  try {
    return exports.oc_score(peError, apError, tolerance, fuelLeft, timeFraction);
  } catch {
    return null;
  }
}
