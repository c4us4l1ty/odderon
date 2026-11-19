/* Smoke test for the compiled wasm artifact.
 *
 * Runs the same golden values as `tests/parity.rs`, but through the real
 * WebAssembly boundary the browser uses, so a broken ABI cannot slip through
 * on the strength of the Rust tests alone.
 *
 *   node smoke.mjs target/wasm32-unknown-unknown/release/odderon_core.wasm
 */

import { readFile } from "node:fs/promises";

const path = process.argv[2] ?? "target/wasm32-unknown-unknown/release/odderon_core.wasm";
const bytes = await readFile(path);
const { instance } = await WebAssembly.instantiate(bytes, {});
const x = instance.exports;

const SLOTS = 64;
const scratch = () =>
  new Float64Array(x.memory.buffer, x.oc_scratch_ptr(), SLOTS);

let failures = 0;
const check = (name, ok, detail) => {
  if (!ok) failures += 1;
  console.log(`${ok ? "ok  " : "FAIL"} ${name}${detail ? ` — ${detail}` : ""}`);
};
const near = (actual, expected, tol) => Math.abs(actual - expected) <= tol;

const R = [-6575229.422777696, -3346948.1516555264, 1038359.3665586751];
const V = [2182.7500708667935, -5958.9428475686445, -3197.413134061305];
const MU = 3.986004418e14;

check("version", x.oc_version() === 2, `v${x.oc_version()}`);
check("self test", x.oc_self_test() === 1);

const n = x.oc_elements(...R, ...V, MU);
check("elements writes 23 doubles", n === 23, `${n}`);
const el = scratch().slice(0, 23);
const [a, e, i, raan, argp, nu, p, pe, ap, period] = el;
check("semi-major axis", near(a, 7055000, 0.01), a.toFixed(3));
check("eccentricity", near(e, 0.07058823529411831, 1e-9), e.toFixed(12));
check("inclination", near(i, 0.4974188368183839, 1e-9), i.toFixed(9));
check("raan", near(raan, 0.7330382858376184, 1e-9), raan.toFixed(9));
check("argp", near(argp, 5.969026041820617, 1e-9), argp.toFixed(9));
check("true anomaly", near(nu, 2.531045009370307, 1e-9), nu.toFixed(9));
check("periapsis", near(pe, 6557000, 0.01), pe.toFixed(3));
check("apoapsis", near(ap, 7553000, 0.01), ap.toFixed(3));
check("period", near(period, 5897.344626441641, 1e-6), period.toFixed(6));
check("semi-latus rectum", near(p, 7019847.058823518, 0.01), p.toFixed(3));

const k = x.oc_kepler(...R, ...V, period, MU);
check("kepler writes 6 doubles", k === 6, `${k}`);
const after = scratch().slice(0, 6);
const drift = Math.hypot(after[0] - R[0], after[1] - R[1], after[2] - R[2]);
check("one period returns to the start", drift < 0.5, `${drift.toFixed(4)} m`);

const rk = x.oc_rk4(...R, ...V, 30, 0, 0, 0, 0, MU);
check("rk4 writes 6 doubles", rk === 6, `${rk}`);
const rkState = scratch().slice(0, 6);
const rkDrift = Math.hypot(
  rkState[0] - R[0],
  rkState[1] - R[1],
  rkState[2] - R[2],
) - drift; // both are compared against the same start below
void rkDrift;
const kep = x.oc_kepler(...R, ...V, 30, MU);
const kepState = scratch().slice(0, 6);
const rkVersusKep = Math.hypot(
  rkState[0] - kepState[0],
  rkState[1] - kepState[1],
  rkState[2] - kepState[2],
);
check("rk4 agrees with kepler over 30 s", rkVersusKep < 1.0, `${rkVersusKep.toFixed(4)} m`);

const tApo = x.oc_time_to_apsis(a, e, nu, 1, MU);
check("time to apoapsis", tApo > 500 && tApo < 700, `${tApo.toFixed(2)} s`);

const composite = x.oc_score(-18800, -2300, 45000, 0.874, 0.99);
check("score composite", near(composite, 0.884, 0.02), composite.toFixed(4));

const perfect = x.oc_score(0, 0, 45000, 1, 1);
check("perfect flight scores 1", near(perfect, 1, 1e-12), perfect.toFixed(6));

const size = bytes.byteLength;
console.log(`wasm size ${size} bytes (${(size / 1024).toFixed(1)} KiB)`);
if (failures > 0) {
  console.error(`${failures} check(s) failed`);
  process.exit(1);
}
console.log("all wasm checks passed");
