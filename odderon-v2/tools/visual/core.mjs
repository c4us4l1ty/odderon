/* Parity check: Rust core (wasm) against the TypeScript fallback.
 *
 * The two paths must agree on the same input state; the game itself switches
 * to the fallback with ?core=ts, so this compares exactly what ships.
 */
import chromium from "@sparticuz/chromium";
import { chromium as pw } from "playwright-core";

const browser = await pw.launch({
  executablePath: "/tmp/chromium",
  args: [...chromium.args, "--use-gl=angle", "--use-angle=swiftshader", "--enable-unsafe-swiftshader", "--in-process-gpu"],
  headless: true,
  timeout: 90000,
});

const R = [-6575229.422777696, -3346948.1516555264, 1038359.3665586751];
const V = [2182.7500708667935, -5958.9428475686445, -3197.413134061305];
const MU = 3.986004418e14;

const errors = [];
const page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
page.on("pageerror", (e) => errors.push(e.message.slice(0, 200)));
page.on("console", (m) => { if (m.type() === "error") errors.push(m.text().slice(0, 200)); });

async function sample(query) {
  await page.goto(`http://127.0.0.1:5173/${query}`, { waitUntil: "load" });
  await page.waitForFunction(() => Boolean(window.odderon && window.odderonMath), { timeout: 25000 });
  return page.evaluate(({ R, V, MU }) => {
    const m = window.odderonMath;
    const r = window.odderon.vessel.state.r.clone().set(R[0], R[1], R[2]);
    const v = window.odderon.vessel.state.v.clone().set(V[0], V[1], V[2]);
    const el = m.elementsFromState(r, v);
    const k = m.propagateKepler(r, v, 600, MU);
    return {
      core: window.odderon.coreStatus,
      a: el.a, e: el.e, i: el.i, raan: el.raan, argp: el.argp, nu: el.nu, period: el.period,
      rx: k.r.x, ry: k.r.y, rz: k.r.z, vx: k.v.x, vy: k.v.y, vz: k.v.z,
    };
  }, { R, V, MU });
}

const wasm = await sample("");
const ts = await sample("?core=ts");

const keys = ["a", "e", "i", "raan", "argp", "nu", "period", "rx", "ry", "rz", "vx", "vy", "vz"];
let worst = 0;
for (const k of keys) {
  const rel = Math.abs(wasm[k] - ts[k]) / Math.max(Math.abs(ts[k]), 1);
  worst = Math.max(worst, rel);
  console.log(`${k.padEnd(7)} wasm ${wasm[k].toPrecision(12)}  ts ${ts[k].toPrecision(12)}  d ${rel.toExponential(2)}`);
}
console.log({ wasmCore: wasm.core, tsCore: ts.core, worstRelativeDelta: worst.toExponential(2), errors });
await browser.close();

if (wasm.core !== "ready") { console.error("wasm core did not load"); process.exit(1); }
if (ts.core !== "unavailable") { console.error("fallback was not forced"); process.exit(1); }
if (worst > 1e-10) { console.error("parity exceeded 1e-10"); process.exit(1); }
console.log("parity ok");
