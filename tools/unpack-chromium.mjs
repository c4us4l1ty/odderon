#!/usr/bin/env node
/**
 * One-time setup for the headless browser in this sandbox.
 *
 * `@sparticuz/chromium` ships a headless Chromium and a set of shared objects, both compressed. The
 * compressed Chromium happens to self-extract when `executablePath()` is first called, but the shared
 * objects do not — and without them the binary exits 127 on `libnspr4.so`.
 *
 * This script decompresses the bundled tar into /tmp so `tools/gl-info.mjs` and any future screenshot
 * harness can point `LD_LIBRARY_PATH` at it.
 *
 * Usage: node tools/unpack-chromium.mjs
 */

import { readFileSync, writeFileSync, mkdirSync, existsSync } from "node:fs";
import { brotliDecompressSync } from "node:zlib";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { execFileSync } from "node:child_process";

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, "..");

const pkg = resolve(root, "node_modules/@sparticuz/chromium/bin/al2023.tar.br");
const outDir = "/tmp/al2023";
const tarPath = "/tmp/al2023.tar";

if (!existsSync(pkg)) {
  console.error(`missing ${pkg}\n  run: npm install`);
  process.exit(1);
}

console.log("decompressing shared libraries…");
writeFileSync(tarPath, brotliDecompressSync(readFileSync(pkg)));

mkdirSync(outDir, { recursive: true });
execFileSync("tar", ["-xf", tarPath, "-C", outDir]);

console.log(`extracted to ${outDir}`);
console.log(`  export LD_LIBRARY_PATH=${outDir}/lib`);
console.log("done. note: this sandbox has no real GPU — see tools/README.md §3.");
