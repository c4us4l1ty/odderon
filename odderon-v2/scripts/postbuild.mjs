/* Rewrite the entry tag so the build also runs from a file:// URL (Electron).
   Chromium refuses module scripts from a file origin, but the bundle is an
   IIFE, so a deferred classic script is enough. */

import { readFile, writeFile } from "node:fs/promises";

const file = new URL("../dist/index.html", import.meta.url);
let html = await readFile(file, "utf8");
const before = html;
html = html.replace(
  /<script type="module" crossorigin src="([^"]+)"><\/script>/g,
  '<script defer src="$1"></script>',
);
if (html === before) {
  console.warn("postbuild: entry script tag not found or already rewritten");
}
await writeFile(file, html);
