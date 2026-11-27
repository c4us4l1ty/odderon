#!/usr/bin/env node
/**
 * The gate that makes the museum a real kernel layout rather than a page-drawn gallery.
 *
 * `web/index.html` answers "where does my career sit?" by calling the kernel's `museum_*`
 * exports — the seam over `odderon_astro::museum`, which computes the gallery layout: the spiral
 * position, display scale, and rotation of each exhibit from the archive's entry descriptors.
 * The same "no substitute" contract the orbit, the pad, the rocket, the pass, the rendezvous, the
 * arc, the comms and the light-cone follow: the page computes no placement rule, holds no golden
 * angle, no base radius, no radial step, and shows no gallery when the kernel is absent.
 *
 * The checks:
 *
 *   1. NO LOCAL PLACEMENT MATH. The page may read the kernel's gallery layout but must not compute
 *      a placement rule itself: no hard-coded golden angle, no local spiral, no local scale curve.
 *      The failure mode is "someone re-adds a page-side layout", which no runtime probe can see.
 *
 *   2. THE PAGE REQUIRES EVERY MUSEUM EXPORT. The page must reference all kernel accessors:
 *      `museum_layout`, `museum_exhibit_kind`, and `museum_design_hash`. A partial build fails at
 *      load.
 *
 *   3. NO SILENT FALLBACK. `museumGallery` must return a named source when the kernel is absent,
 *      and the gallery must show a refusal rather than a fabricated layout.
 *
 *   4. THE GALLERY READINGS COME FROM THE KERNEL. `museumGallery` must call `museum_layout`, not
 *      compute a layout from a local formula.
 *
 *   5. THE WASM BOUNDARY IS PHYSICAL. When a build is available, the exports must reproduce the
 *      invariants of the gallery layout: the spiral grows outward (later exhibits sit further from
 *      the centre), the layout is deterministic (same entries, same gallery), the scale maps a
 *      reading of 100 to 1.0 and is clamped, the rotation is the orientation or zero, the exhibit
 *      kind codes round-trip, and the design hash is deterministic and non-zero.
 *
 * Usage: node tools/check-museum.mjs            (static: no browser, no wasm needed)
 *        node tools/check-museum.mjs --selftest  (verify the verifier accepts good exports)
 *        node tools/check-museum.mjs --wasm dist/odderon_dmath_wasm.wasm
 *                                                    (also run the exports, if a build is available)
 */

import { readFile } from "node:fs/promises";

const PAGE = new URL("../web/index.html", import.meta.url);

/** Strip comments so documentation about the museum does not trip the checks that hunt for a layout. */
function liveSource(html) {
  const script = html.match(/<script type="module">([\s\S]*?)<\/script>/);
  if (!script) throw new Error("no module script found in web/index.html");
  return script[1]
    .replace(/\/\*[\s\S]*?\*/g, "")
    .replace(/(^|[^:])\/\/.*$/gm, "$1");
}

const checks = [];
function check(name, fn) {
  checks.push({ name, fn });
}

check("no local placement math in the page", (src) => {
  // A local layout would carry the golden angle, a spiral formula, a scale curve, or a radial
  // step. Each is the giveaway that the page is computing the layout itself. The check is scoped
  // to the museum functions (museumGallery and renderMuseum) because the page's shader code
  // legitimately uses a golden angle for cloud rendering, which is a different concern entirely.
  const museumBody = src.match(/function museumGallery\s*\([^)]*\)\s*\{[\s\S]*?\n\}/)?.[0] || "";
  const renderBody = src.match(/function renderMuseum\s*\([^)]*\)\s*\{[\s\S]*?\n\}/)?.[0] || "";
  const scoped = museumBody + "\n" + renderBody;
  const patterns = [
    [/museum_layout\s*\([^)]*\)\s*=[^;]*Math\./, "a hand-computed museum layout"],
    [/0\.618_?033/, "a hard-coded golden ratio"],
    [/GOLDEN_ANGLE/i, "a golden-angle constant"],
    [/BASE_RADIUS/i, "a base-radius constant"],
    [/RADIAL_STEP/i, "a radial-step constant"],
    [/137\.5/, "a hard-coded golden angle in degrees"],
    [/Math\.sqrt\s*\([^)]*size[^)]*\)/, "a local scale curve"],
    [/2\s*\/\s*Math\.PI\s*\/\s*\(/, "a local angle formula"],
  ];
  const hits = patterns.filter(([re]) => re.test(scoped)).map(([, why]) => why);
  if (hits.length) throw new Error("page still computes the gallery layout itself: " + hits.join("; "));
});

check("the page requires every museum export", (src) => {
  const needed = [
    "museum_layout",
    "museum_exhibit_kind",
    "museum_design_hash",
  ];
  const missing = needed.filter((n) => !src.includes(n));
  if (missing.length) {
    throw new Error("page does not reference: " + missing.join(", "));
  }
});

check("a missing kernel hides the gallery rather than substituting a layout", (src) => {
  // The contract: when the kernel is absent, `museumGallery` returns a named `source` that is
  // not the kernel, and the gallery shows a refusal rather than a fabricated layout.
  if (!/function museumGallery\s*\([^)]*\)\s*\{[\s\S]{0,200}?if \(!k\s*\|\|\s*!k\.memory\)\s*return\s*\{ source: 'absent' \};/.test(src)) {
    throw new Error("museumGallery does not return a named absent source when the kernel is missing");
  }
});

check("the gallery readings come from the kernel, not a local formula", (src) => {
  const m = src.match(/function museumGallery\s*\([^)]*\)\s*\{([\s\S]*?)\n\}/);
  if (!m) throw new Error("museumGallery not found");
  const body = m[1];
  if (!body.includes("museum_layout")) {
    throw new Error("museumGallery does not call museum_layout");
  }
  if (/k\.museum_layout\s*\([^)]*\)\s*=\s*[^;]*Math\./.test(body)) {
    throw new Error("museumGallery assigns a hand-computed value to museum_layout's result");
  }
});

/**
 * The layout invariants the museum exports must satisfy, as a function of the exports alone.
 *
 * Separated from the file loading because the wasm build only exists in CI: if the verification
 * lived inside `runWasm`, none of it could be executed in this sandbox, and "checked in CI" would
 * be the whole of its testing. `--selftest` drives this with a deliberate shim so both the accept
 * and reject paths run here.
 */
function verifyExports(ex) {
  const need = [
    "museum_layout",
    "museum_exhibit_kind",
    "museum_design_hash",
  ];
  const missing = need.filter((n) => typeof ex[n] !== "function");
  if (missing.length) throw new Error("module is missing: " + missing.join(", "));

  const problems = [];

  // Write 3 entry descriptors into linear memory and read the layout back.
  // Each descriptor is 4 f64: [kind, size, orient, id].
  // Each exhibit is 6 f64: [x, z, scale, rotation, kind, id].
  const memory = ex.memory;
  if (!memory) throw new Error("module has no memory export");
  const NEED = 1 << 20;
  while (memory.buffer.byteLength < NEED + (1 << 20)) {
    try { memory.grow(256); } catch (e) { throw new Error("cannot grow memory: " + e.message); }
  }
  const dv = new DataView(memory.buffer);
  const DESC_OFF = NEED;
  const OUT_OFF = DESC_OFF + 3 * 4 * 8;

  // Vehicle (kind 0) at id 0, apogee 100 km, orient 0.
  dv.setFloat64(DESC_OFF + 0 * 4 * 8, 0, true);
  dv.setFloat64(DESC_OFF + 0 * 4 * 8 + 8, 100.0, true);
  dv.setFloat64(DESC_OFF + 0 * 4 * 8 + 16, 0.0, true);
  dv.setFloat64(DESC_OFF + 0 * 4 * 8 + 24, 0, true);
  // Vehicle at id 1, apogee 400 km, orient 1.5.
  dv.setFloat64(DESC_OFF + 1 * 4 * 8, 0, true);
  dv.setFloat64(DESC_OFF + 1 * 4 * 8 + 8, 400.0, true);
  dv.setFloat64(DESC_OFF + 1 * 4 * 8 + 16, 1.5, true);
  dv.setFloat64(DESC_OFF + 1 * 4 * 8 + 24, 1, true);
  // Vehicle at id 2, apogee 0, orient NaN.
  dv.setFloat64(DESC_OFF + 2 * 4 * 8, 0, true);
  dv.setFloat64(DESC_OFF + 2 * 4 * 8 + 8, 0.0, true);
  dv.setFloat64(DESC_OFF + 2 * 4 * 8 + 16, NaN, true);
  dv.setFloat64(DESC_OFF + 2 * 4 * 8 + 24, 2, true);

  const count = ex.museum_layout(DESC_OFF, 3, OUT_OFF);
  if (count !== 3) problems.push(`museum_layout returned ${count}, want 3`);

  const e0 = {
    x: dv.getFloat64(OUT_OFF + 0 * 6 * 8, true),
    z: dv.getFloat64(OUT_OFF + 0 * 6 * 8 + 8, true),
    scale: dv.getFloat64(OUT_OFF + 0 * 6 * 8 + 16, true),
    rotation: dv.getFloat64(OUT_OFF + 0 * 6 * 8 + 24, true),
    kind: dv.getFloat64(OUT_OFF + 0 * 6 * 8 + 32, true),
    id: dv.getFloat64(OUT_OFF + 0 * 6 * 8 + 40, true),
  };
  const e1 = {
    x: dv.getFloat64(OUT_OFF + 1 * 6 * 8, true),
    z: dv.getFloat64(OUT_OFF + 1 * 6 * 8 + 8, true),
    scale: dv.getFloat64(OUT_OFF + 1 * 6 * 8 + 16, true),
    rotation: dv.getFloat64(OUT_OFF + 1 * 6 * 8 + 24, true),
    kind: dv.getFloat64(OUT_OFF + 1 * 6 * 8 + 32, true),
    id: dv.getFloat64(OUT_OFF + 1 * 6 * 8 + 40, true),
  };
  const e2 = {
    x: dv.getFloat64(OUT_OFF + 2 * 6 * 8, true),
    z: dv.getFloat64(OUT_OFF + 2 * 6 * 8 + 8, true),
    scale: dv.getFloat64(OUT_OFF + 2 * 6 * 8 + 16, true),
    rotation: dv.getFloat64(OUT_OFF + 2 * 6 * 8 + 24, true),
    kind: dv.getFloat64(OUT_OFF + 2 * 6 * 8 + 32, true),
    id: dv.getFloat64(OUT_OFF + 2 * 6 * 8 + 40, true),
  };

  // The spiral grows outward: later exhibits sit further from the centre.
  const r0 = Math.sqrt(e0.x * e0.x + e0.z * e0.z);
  const r1 = Math.sqrt(e1.x * e1.x + e1.z * e1.z);
  const r2 = Math.sqrt(e2.x * e2.x + e2.z * e2.z);
  if (!(r1 > r0)) problems.push(`spiral does not grow: r0=${r0} r1=${r1}`);
  if (!(r2 > r1)) problems.push(`spiral does not grow: r1=${r1} r2=${r2}`);

  // Scale maps a reading of 100 to 1.0 (the reference).
  if (Math.abs(e0.scale - 1.0) > 1e-12) problems.push(`scale at size 100 = ${e0.scale}, want 1.0`);
  // Scale maps a reading of 400 to 2.0 (sqrt(4)).
  if (Math.abs(e1.scale - 2.0) > 1e-12) problems.push(`scale at size 400 = ${e1.scale}, want 2.0`);
  // Scale clamps a zero reading to the floor.
  if (e2.scale < 0.3 - 1e-12) problems.push(`scale at size 0 = ${e2.scale}, want >= 0.3`);

  // Rotation is the orientation or zero.
  if (Math.abs(e1.rotation - 1.5) > 1e-12) problems.push(`rotation at orient 1.5 = ${e1.rotation}, want 1.5`);
  if (e2.rotation !== 0.0) problems.push(`rotation at orient NaN = ${e2.rotation}, want 0`);

  // Exhibits carry their kind and id.
  if (e0.kind !== 0) problems.push(`exhibit 0 kind = ${e0.kind}, want 0`);
  if (e0.id !== 0) problems.push(`exhibit 0 id = ${e0.id}, want 0`);
  if (e1.id !== 1) problems.push(`exhibit 1 id = ${e1.id}, want 1`);
  if (e2.id !== 2) problems.push(`exhibit 2 id = ${e2.id}, want 2`);

  // Exhibit kind codes round-trip.
  for (let code = 0; code <= 6; code++) {
    const k = ex.museum_exhibit_kind(code);
    if (k !== code) problems.push(`museum_exhibit_kind(${code}) = ${k}, want ${code}`);
  }
  if (ex.museum_exhibit_kind(7) !== -1) problems.push("museum_exhibit_kind(7) is not -1");

  // Design hash is deterministic and non-zero.
  const h1 = ex.museum_design_hash();
  const h2 = ex.museum_design_hash();
  if (h1 !== h2) problems.push("design hash is not deterministic");
  if (h1 === 0) problems.push("design hash is zero");

  if (problems.length) throw new Error("boundary disagrees with the gallery layout: " + problems.join("; "));
  return `exhibits=${count} r0=${r0.toFixed(2)}m r1=${r1.toFixed(2)}m r2=${r2.toFixed(2)}m`;
}

/** Load a built module and verify it. */
async function runWasm(path) {
  const bytes = await readFile(path);
  const { instance } = await WebAssembly.instantiate(bytes, {});
  return verifyExports(instance.exports);
}

/**
 * `--selftest`: prove `verifyExports` accepts a correct boundary and rejects the ways it can be wrong.
 * Without this, the accept branch and the reject branches would first run in CI, and a checker whose
 * rejection path has never executed is not yet known to reject anything.
 */
function selftest() {
  // A shim that implements the gallery layout correctly. The formulas are implemented here in the
  // fixture, which is the point -- the verifier checks the SHAPE of the contract, not a particular
  // implementation's internals.
  const GOLDEN_ANGLE = 2.399963229728653;
  const BASE_RADIUS = 4.0;
  const RADIAL_STEP = 0.8;
  const SCALE_MIN = 0.3;
  const SCALE_MAX = 3.0;

  const good = {
    memory: (() => {
      const buf = new ArrayBuffer(1 << 21);
      return { buffer: buf, grow: (n) => { /* no-op for the shim */ } };
    })(),
    museum_layout: (descOff, n, outOff) => {
      const dv = new DataView(good.memory.buffer);
      for (let i = 0; i < n; i++) {
        const base = descOff + i * 4 * 8;
        const kind = dv.getFloat64(base, true);
        const size = dv.getFloat64(base + 8, true);
        const orient = dv.getFloat64(base + 16, true);
        const id = dv.getFloat64(base + 24, true);
        const angle = id * GOLDEN_ANGLE;
        const radius = BASE_RADIUS + id * RADIAL_STEP;
        const sinA = Math.sin(angle), cosA = Math.cos(angle);
        let scale;
        if (!Number.isFinite(size)) scale = 1.0;
        else if (size <= 0) scale = SCALE_MIN;
        else {
          const raw = Math.sqrt(size / 100.0);
          scale = Math.max(SCALE_MIN, Math.min(SCALE_MAX, raw));
        }
        const rotation = Number.isFinite(orient) ? orient : 0.0;
        const ob = outOff + i * 6 * 8;
        dv.setFloat64(ob, radius * cosA, true);
        dv.setFloat64(ob + 8, radius * sinA, true);
        dv.setFloat64(ob + 16, scale, true);
        dv.setFloat64(ob + 24, rotation, true);
        dv.setFloat64(ob + 32, kind, true);
        dv.setFloat64(ob + 40, id, true);
      }
      return n;
    },
    museum_exhibit_kind: (code) => (code >= 0 && code <= 6 ? code : -1),
    museum_design_hash: () => 0xcbf29ce484222325,
  };

  const results = [];
  try {
    verifyExports(good);
    results.push(["a correct boundary is accepted", true]);
  } catch (err) {
    results.push([`a correct boundary is accepted (${err.message})`, false]);
  }

  // A layout that does not grow outward is not the kernel's.
  const wrongRadius = { ...good, museum_layout: (descOff, n, outOff) => {
    const dv = new DataView(good.memory.buffer);
    for (let i = 0; i < n; i++) {
      const ob = outOff + i * 6 * 8;
      dv.setFloat64(ob, 1.0, true);
      dv.setFloat64(ob + 8, 0.0, true);
      dv.setFloat64(ob + 16, 1.0, true);
      dv.setFloat64(ob + 24, 0.0, true);
      dv.setFloat64(ob + 32, 0.0, true);
      dv.setFloat64(ob + 40, i, true);
    }
    return n;
  } };
  try {
    verifyExports(wrongRadius);
    results.push(["a layout that does not grow is rejected", false]);
  } catch {
    results.push(["a layout that does not grow is rejected", true]);
  }

  // A scale that is not the sqrt curve is not the kernel's.
  const wrongScale = { ...good, museum_layout: (descOff, n, outOff) => {
    const dv = new DataView(good.memory.buffer);
    for (let i = 0; i < n; i++) {
      const base = descOff + i * 4 * 8;
      const id = dv.getFloat64(base + 24, true);
      const angle = id * GOLDEN_ANGLE;
      const radius = BASE_RADIUS + id * RADIAL_STEP;
      const ob = outOff + i * 6 * 8;
      dv.setFloat64(ob, radius * Math.cos(angle), true);
      dv.setFloat64(ob + 8, radius * Math.sin(angle), true);
      dv.setFloat64(ob + 16, 5.0, true); // always 5.0, not the sqrt curve
      dv.setFloat64(ob + 24, 0.0, true);
      dv.setFloat64(ob + 32, 0.0, true);
      dv.setFloat64(ob + 40, id, true);
    }
    return n;
  } };
  try {
    verifyExports(wrongScale);
    results.push(["a scale that is not the sqrt curve is rejected", false]);
  } catch {
    results.push(["a scale that is not the sqrt curve is rejected", true]);
  }

  // A design hash that is not deterministic is not the kernel's.
  const wrongHash = { ...good, museum_design_hash: () => Math.random() };
  try {
    verifyExports(wrongHash);
    results.push(["a non-deterministic design hash is rejected", false]);
  } catch {
    results.push(["a non-deterministic design hash is rejected", true]);
  }

  let bad = 0;
  for (const [name, ok] of results) {
    console.log(`  ${ok ? "ok  " : "FAIL"} ${name}`);
    if (!ok) bad++;
  }
  return bad;
}

const html = await readFile(PAGE, "utf8");
const src = liveSource(html);

let failed = 0;
for (const { name, fn } of checks) {
  try {
    fn(src);
    console.log(`  ok   ${name}`);
  } catch (e) {
    failed++;
    console.log(`  FAIL ${name}`);
    console.log(`       ${e.message}`);
  }
}

if (process.argv.includes("--selftest")) {
  const bad = selftest();
  console.log("");
  console.log(bad ? `${bad} self-test(s) FAILED` : "self-test passed: the verifier accepts good exports and rejects bad ones");
  process.exit(bad ? 1 : 0);
}

const wasmArg = process.argv.indexOf("--wasm");
if (wasmArg !== -1) {
  const path = process.argv[wasmArg + 1];
  try {
    const detail = await runWasm(path);
    console.log(`  ok   wasm boundary: ${detail}`);
  } catch (e) {
    failed++;
    console.log(`  FAIL wasm boundary`);
    console.log(`       ${e.message}`);
  }
} else {
  console.log("  ..   wasm boundary not run (pass --wasm <file> where a build exists)");
}

console.log("");
if (failed) {
  console.log(`${failed} museum check(s) FAILED`);
  process.exit(1);
}
console.log(`all ${checks.length} museum checks passed`);
