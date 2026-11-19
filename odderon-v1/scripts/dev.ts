import { resolve, extname } from "node:path";

const root = resolve(import.meta.dir, "../web");
const port = Number(Bun.env.PORT ?? 3000);
const mime: Record<string, string> = {
  ".html": "text/html; charset=utf-8", ".js": "text/javascript; charset=utf-8",
  ".css": "text/css; charset=utf-8", ".wasm": "application/wasm",
  ".jpg": "image/jpeg", ".jpeg": "image/jpeg", ".png": "image/png",
  ".svg": "image/svg+xml"
};

const server = Bun.serve({
  hostname: "127.0.0.1",
  port,
  async fetch(request) {
    const url = new URL(request.url);
    let pathname: string;
    try { pathname = decodeURIComponent(url.pathname); }
    catch { return new Response("Bad request", { status: 400 }); }
    if (pathname.endsWith("/")) pathname += "index.html";
    const path = resolve(root, `.${pathname}`);
    if (path !== root && !path.startsWith(`${root}/`)) return new Response("Forbidden", { status: 403 });
    const file = Bun.file(path);
    if (!await file.exists()) return new Response("Not found", { status: 404 });
    const headers = { "Content-Type": mime[extname(path)] ?? "application/octet-stream", "Cache-Control": "no-store" };
    if (request.method === "HEAD") return new Response(null, { headers });
    if (request.method !== "GET") return new Response("Method not allowed", { status: 405, headers: { Allow: "GET, HEAD" } });
    return new Response(file, { headers });
  }
});

console.log(`Odderon is ready: http://localhost:${server.port}/`);
console.log("Press Ctrl+C to stop.");
