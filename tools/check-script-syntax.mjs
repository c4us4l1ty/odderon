#!/usr/bin/env node
/**
 * The page's module script must parse.
 *
 * ## The failure this exists for, measured four times in one sitting
 *
 * `web/index.html` holds its fragment shader in a JavaScript TEMPLATE LITERAL. That means a backtick
 * anywhere inside the GLSL -- including in a comment, where it reads as typography -- terminates the
 * literal early. What follows is parsed as JavaScript, and the browser throws
 * `Unexpected identifier 'patch'` with a line number pointing into a shader.
 *
 * Four separate edits to the room hit this. Each one cost a render cycle to find, because the symptom
 * is a completely blank canvas and no console error from WebGL: the module never ran at all. A search
 * for unbalanced backticks does not catch it, because the count stays even -- one stray backtick closes
 * the literal and the next one re-opens it.
 *
 * So this does not count anything. It extracts the script and asks Node to parse it, which is the same
 * question the browser asks, in one tenth of a second, with no browser and no GPU.
 *
 * Usage: node tools/check-script-syntax.mjs
 */
import { readFile, writeFile, mkdtemp } from "node:fs/promises";
import { execFile } from "node:child_process";
import { tmpdir } from "node:os";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";

const run = promisify(execFile);
const HERE = dirname(fileURLToPath(import.meta.url));
const PAGE = join(HERE, "..", "web", "index.html");

const html = await readFile(PAGE, "utf8");
const m = /<script type="module">([\s\S]*?)<\/script>/.exec(html);
if (!m) {
  console.log("  FAIL no <script type=\"module\"> in web/index.html");
  process.exit(1);
}
const src = m[1];

// The HTML holds the script at a line offset; report the page's own line numbers, not the extract's.
const offset = html.slice(0, m.index).split("\n").length - 1;

const dir = await mkdtemp(join(tmpdir(), "odderon-syntax-"));
const file = join(dir, "module.mjs");
await writeFile(file, src);

try {
  await run(process.execPath, ["--check", file]);
  console.log(`  ok   web/index.html module script parses (${src.split("\n").length} lines)`);
} catch (e) {
  const msg = (e.stderr || e.message || "").trim().split("\n");
  // Node reports "file:LINE" and then the offending line; map it back onto the page.
  const loc = msg.find((l) => l.includes("module.mjs:"));
  const n = loc ? Number(loc.split(":")[1]) : null;
  console.log(`  FAIL web/index.html module script does not parse${n ? ` (page line ${n + offset})` : ""}`);
  for (const l of msg.slice(0, 6)) console.log(`       ${l}`);
  console.log("");
  console.log("  A stray backtick inside a GLSL template literal is the usual cause: GLSL comments are");
  console.log("  full of words like `patch`, and one backtick ends the literal early.");
  process.exit(1);
}
