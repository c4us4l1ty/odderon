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
import { readFile } from "node:fs/promises";
import { dirname, extname, join, resolve, normalize } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, "..");
const webRoot = join(root, "web");

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

const server = createServer(async (req, res) => {
  const url = (req.url ?? "/").split("?")[0];
  const rel = url === "/" ? "/index.html" : url;

  // `/dist/*` serves build output (the wasm module, when it exists); everything else comes from web/.
  const base = rel.startsWith("/dist/") ? root : webRoot;
  const path = normalize(rel.startsWith("/dist/") ? rel : join("/", rel));

  try {
    const file = join(base, path);
    if (!file.startsWith(base)) throw new Error("traversal");
    const body = await readFile(file);
    res.writeHead(200, {
      "content-type": MIME[extname(file)] ?? "application/octet-stream",
      // The wasm module will be fetched cross-origin from this origin's perspective only if it ever
      // moves to a CDN. Sending the header now means the page does not need changing then.
      "access-control-allow-origin": "*",
      "cache-control": "no-store",
    });
    res.end(body);
  } catch {
    res.writeHead(404, { "content-type": "text/plain" }).end("not found: " + rel);
  }
});

server.listen(port, "0.0.0.0", () => {
  console.log(`shell on http://0.0.0.0:${port}/  (web root: ${webRoot})`);
});
