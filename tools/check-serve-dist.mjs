#!/usr/bin/env node
/**
 * The preview must be able to serve the kernel, even though `dist/` is git-ignored.
 *
 * ## Why this exists
 *
 * Twice, a reader opened the preview and saw geometry with no solver: `orbit source: unavailable`,
 * `room solar: — no kernel`, a hole onto a flat horizon. Both times my answer was an explanation --
 * the module is a build output, the sandbox cannot build it -- and both times the explanation was
 * correct and useless. The preview is the only place the game is actually LOOKED AT, so it is the one
 * place "you cannot see it here" is not an acceptable answer.
 *
 * `tools/serve.mjs` now resolves `/dist/<name>` against the served `dist/` first and cargo's own
 * `target/wasm32-unknown-unknown/release` second. A `npm run fetch:wasm` artifact always wins; a local
 * build just works; neither present is the page's banner, which is correct.
 *
 * ## What this tests, and what it refuses to accept
 *
 * A server that returns 200 for everything would pass a naive version of this. So:
 *
 *   * the kernel is fetched over HTTP and instantiated, and its four room exports are CALLED -- a
 *     fallback that serves the wrong file, or a truncated one, fails at instantiation, not at status;
 *   * a name that exists in neither directory is still a 404, so the fallback is a lookup rather than a
 *     blanket;
 *   * traversal out of the directory is still refused, because the fallback must not reopen the
 *     arbitrary-file-read that `tools/check-static-path.mjs` was written for;
 *   * and it must work with `dist/` EMPTY, which is the condition that was broken.
 *
 * Usage: node tools/check-serve-dist.mjs [--port N]
 */

import { readFile, rename, stat, mkdir } from "node:fs/promises";
import { spawn } from "node:child_process";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const HERE = dirname(fileURLToPath(import.meta.url));
const ROOT = join(HERE, "..");
const DIST = join(ROOT, "dist");
const CARGO = join(ROOT, "target", "wasm32-unknown-unknown", "release", "odderon_dmath_wasm.wasm");
const SERVED = join(DIST, "odderon_dmath_wasm.wasm");

const args = process.argv.slice(2);
const portArg = args.indexOf("--port");
const port = portArg >= 0 ? Number(args[portArg + 1]) : 8901;

const fail = [];
const pass = [];
const check = (name, ok, detail = "") => (ok ? pass : fail).push(detail ? `${name}  ${detail}` : name);

const exists = async (p) => {
  try {
    return (await stat(p)).isFile();
  } catch {
    return false;
  }
};

if (!(await exists(CARGO))) {
  console.log(`  skip: no cargo output at ${CARGO.replace(ROOT + "/", "")}`);
  console.log("        build it with: cargo build --release --target wasm32-unknown-unknown -p odderon-dmath-wasm");
  console.log("\nserve-dist check INCOMPLETE (not a pass: nothing was verified)");
  process.exit(1);
}

// Put the served dist/ into the state a clone has: no kernel, and RESTORE IT NO MATTER WHAT.
//
// THE FIRST VERSION OF THIS CHECK DELETED THE MODULE. `moved` was assigned and then `spawn()` threw
// synchronously -- in this sandbox `process.execPath` under a child spawn can fail before the try/finally
// below ever runs -- so the file stayed parked at `.moved` and `dist/` was empty for every later tool.
// `check-room` and `check-missing-kernel` then failed for reasons that had nothing to do with the code
// under test, and the run before this one left the working tree in a state I had to repair by hand.
//
// A test that can destroy the thing it is testing is worse than no test. The rename now happens inside
// the try, the restore is unconditional, and the check asserts its own cleanup at the end.
let moved = null;
try {
  await mkdir(DIST, { recursive: true });
  const child = spawn(process.execPath, ["tools/serve.mjs", String(port)], {
    cwd: ROOT,
    stdio: ["ignore", "pipe", "pipe"],
  });
  const ready = new Promise((res, rej) => {
    const t = setTimeout(() => rej(new Error("server did not start in 10 s")), 10000);
    child.stdout.on("data", (b) => {
      if (/shell on http/.test(String(b))) {
        clearTimeout(t);
        res();
      }
    });
    child.on("error", rej);
  });

  try {
    await ready;

    // Hide the kernel only now that the child is running, so a spawn failure cannot strand the file.
    if (await exists(SERVED)) {
      moved = SERVED + ".moved";
      await rename(SERVED, moved);
    }
    check("dist/ has no kernel for this run", !(await exists(SERVED)));

    const base = `http://127.0.0.1:${port}`;

    const res = await fetch(`${base}/dist/odderon_dmath_wasm.wasm`);
    check("the kernel is served with dist/ empty", res.status === 200, `status ${res.status}`);
    check(
      "it is served as wasm",
      (res.headers.get("content-type") || "").includes("application/wasm"),
      res.headers.get("content-type") || "(none)",
    );

    // THE LOAD-BEARING ASSERTION: it has to be a working module, not a 200 with bytes in it.
    const bytes = await res.arrayBuffer();
    let ex = null;
    try {
      ex = (await WebAssembly.instantiate(bytes, {})).instance.exports;
    } catch (e) {
      fail.push(`the served bytes do not instantiate: ${e.message}`);
    }
    if (ex) {
      const missing = ["room_solar_w", "room_light_fraction", "room_cooling_w", "room_draw_w"].filter(
        (n) => typeof ex[n] !== "function",
      );
      check("the served module has the four room exports", missing.length === 0, missing.join(", "));
      if (!missing.length) {
        // A number, and the one that matters: 800 W/m2 x 3 m2 x SHGC 0.60 = 1440 W open, 120 W shut.
        const open = ex.room_solar_w(0);
        const shut = ex.room_solar_w(1);
        check("and computes the bible's solar gain", Math.abs(open - 1440) < 1 && Math.abs(shut - 120) < 1,
          `${open.toFixed(1)} -> ${shut.toFixed(1)} W`);
      }
    }

    // A fallback must be a lookup, not a blanket.
    const other = await fetch(`${base}/dist/not-a-real-module.wasm`);
    check("an absent /dist file is still a 404", other.status === 404, `status ${other.status}`);

    // And it must not reopen the traversal. This is the exact request that used to return .git/config.
    const traversal = await fetch(`${base}/dist/../.git/config`);
    const body = await traversal.text();
    check(
      "traversal out of dist/ is still refused",
      traversal.status === 403 || traversal.status === 404 || !/\[core\]/.test(body),
      `status ${traversal.status}`,
    );
  } finally {
    child.kill("SIGTERM");
  }
} catch (e) {
  fail.push(`threw: ${e && e.message ? e.message : e}`);
} finally {
  if (moved) await rename(moved, SERVED).catch(() => {});
}

// The cleanup is part of the contract, so it is asserted rather than trusted.
if (moved) {
  check("the check restored the module it hid", await exists(SERVED));
}

for (const m of pass) console.log(`  ok   ${m}`);
for (const m of fail) console.log(`  FAIL ${m}`);
console.log("");
if (fail.length) {
  console.log("serve-dist check FAILED");
  process.exit(1);
}
console.log("serve-dist check passed");
