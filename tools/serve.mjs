#!/usr/bin/env node
/**
 * Serve the shell for a browser preview.
 *
 * Bind 0.0.0.0, not 127.0.0.1: the page is opened from the user's browser, which is a different
 * machine from this sandbox. A server bound to loopback is invisible to it.
 *
 * Usage: node tools/serve.mjs [port]
 */

import { createServer } from "node:http";
import { readFile, stat } from "node:fs/promises";
import { extname, join } from "node:path";
import { resolveStaticPath, WEB_ROOT, REPO_ROOT, DIST_ROOT } from "./static-path.mjs";

const MIME = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".mjs": "text/javascript; charset=utf-8",
  ".wasm": "application/wasm",
  ".json": "application/json",
  ".css": "text/css; charset=utf-8",
  ".png": "image/png",
};

const port = Number(process.argv[2] ?? process.env.PORT ?? 8080);

// ---------------------------------------------------------------------------------------------
// `dist/` is git-ignored, so a sandbox serving straight from a clone has no kernel to serve, and the
// page can only draw geometry. That is the whole reason a preview reads "— no kernel" while CI, which
// downloads the artifact, is green. The preview is the one place the game is actually looked at, so it
// is the one place this cannot be left as an explanation.
//
// The fallback order is: the served `dist/` first (so a `npm run fetch:wasm` artifact always wins), then
// cargo's own output (so a local build just works), then nothing -- in which case the page's banner
// fires, which is correct.
// ---------------------------------------------------------------------------------------------
const CARGO_DIST = join(REPO_ROOT, "target", "wasm32-unknown-unknown", "release");

async function exists(p) {
  try {
    return (await stat(p)).isFile();
  } catch {
    return false;
  }
}

/// Resolve a `/dist/*` request across both possible build directories.
async function findDist(rel) {
  for (const root of [DIST_ROOT, CARGO_DIST]) {
    const p = join(root, rel);
    if (await exists(p)) return p;
  }
  return null;
}

const server = createServer(async (req, res) => {
  // One resolver, shared with `tools/check-shell.mjs`, so the two servers cannot disagree about which
  // root a path belongs to. They used to, and the disagreement was an arbitrary-file-read: this file
  // tested `startsWith("/dist/")` on the RAW path and only then resolved and contained it, so
  // `/dist/../.git/config` was served. `tools/check-static-path.mjs` is the regression test.
  const file = resolveStaticPath(req.url);

  if (file === null) {
    res.writeHead(403, { "content-type": "text/plain" }).end("forbidden");
    return;
  }

  // A `/dist/` path that is not in the served directory is looked for in cargo's release directory
  // before it is declared missing. Containment is unchanged -- the path was already resolved and
  // contained by `resolveStaticPath`, and this only chooses which directory to read it from.
  let target = file;
  if (!(await exists(target))) {
    const name = req.url.split("?")[0];
    if (name.startsWith("/dist/")) {
      const alt = await findDist(name.slice("/dist/".length));
      if (alt) target = alt;
    }
  }

  try {
    // stat first: readFile on a directory reports EISDIR on Linux but the message differs by platform,
    // and a directory is a 404 here either way.
    const s = await stat(target);
    if (!s.isFile()) throw new Error("not a file");
    const body = await readFile(target);
    res.writeHead(200, {
      "content-type": MIME[extname(file)] ?? "application/octet-stream",
      // The wasm module will be fetched cross-origin from this origin's perspective only if it ever
      // moves to a CDN. Sending the header now means the page does not need changing then.
      "access-control-allow-origin": "*",
      "cache-control": "no-store",
    });
    res.end(body);
  } catch {
    res.writeHead(404, { "content-type": "text/plain" }).end("not found");
  }
});

server.listen(port, "0.0.0.0", () => {
  console.log(`shell on http://0.0.0.0:${port}/  (web root: ${WEB_ROOT})`);
});
