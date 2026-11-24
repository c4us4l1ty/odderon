/**
 * Tests for `tools/static-path.mjs` — the path resolver both static servers use.
 *
 * This exists because the resolver had a real arbitrary-file-read bug: `tools/serve.mjs` decided the
 * root from the RAW request path and only then resolved and contained it, so `/dist/../.git/config`
 * was served. The sibling server in `tools/check-shell.mjs` was safe only because it happened to use
 * `new URL()` first. A property that holds in one of two copies is not a property.
 *
 * ## What is actually asserted
 *
 * Not "returns null". The security property is **containment**: whatever comes back must lie inside
 * `web/` or `dist/`. A traversal that normalises to `web/.git/config` is *safe* — that file does not
 * exist and the caller 404s — even though it returned a path. Asserting `null` would fail on a correct
 * implementation and, worse, would push the resolver toward conflating "forbidden" with "absent".
 *
 * The one case that must be `null` rather than merely harmless is the sibling-prefix trap, where a
 * bare `startsWith()` would place a file in `web-secrets/` and call it contained.
 *
 * Run: node tools/check-static-path.mjs
 */

import { sep } from "node:path";
import { readFileSync, readdirSync } from "node:fs";
import { resolveStaticPath, isInside, WEB_ROOT, DIST_ROOT, REPO_ROOT } from "./static-path.mjs";

let failures = 0;
let checks = 0;

function ok(name, condition, detail) {
  checks += 1;
  if (!condition) failures += 1;
  console.log(`  ${condition ? "pass" : "FAIL"}: ${name}${condition ? "" : `\n         ${detail}`}`);
}

/** The invariant: a resolved path is inside web/ or inside dist/, or the request was refused. */
function contained(p) {
  if (p === null) return true;                       // refused, which is always acceptable
  return p.startsWith(WEB_ROOT + sep) || p.startsWith(DIST_ROOT + sep);
}

function refuses(name, raw) {
  ok(name, resolveStaticPath(raw) === null, `served ${resolveStaticPath(raw)}`);
}

function contains(name, raw) {
  const got = resolveStaticPath(raw);
  ok(name, contained(got), `ESCAPPED to ${got}`);
  return got;
}

function serves(name, raw, want) {
  const got = resolveStaticPath(raw);
  ok(name, got === want, `got  ${got}\n         want ${want}`);
}

console.log("legitimate requests resolve to the file they name");
serves("the index", "/", WEB_ROOT + "/index.html");
serves("index.html", "/index.html", WEB_ROOT + "/index.html");
serves("a sibling page", "/ablate.html", WEB_ROOT + "/ablate.html");
// A query string is not part of the filename. The old serve.mjs split on "?" by hand; the URL parser
// does it as a side effect of the same call that removes the traversal.
serves("a page with a query string", "/index.html?v=2", WEB_ROOT + "/index.html");
serves("a dist artifact", "/dist/odderon.wasm", DIST_ROOT + "/odderon.wasm");
// CI does `cp {} dist/`, so build output lands directly in dist/ with no subdirectory.
serves("a dist file with no subdirectory", "/dist/x.js", DIST_ROOT + "/x.js");

console.log("\nthe bug that was real — all of these must stay inside the roots");
// This exact URL returned .git/config, 435 bytes, status 200, before the fix.
const leaked = contains("/dist/../.git/config (raw .. after the /dist/ prefix)", "/dist/../.git/config");
ok(
  "  ...and it does not land on a real repo-root file",
  leaked !== REPO_ROOT + "/.git/config",
  `served the repository's own .git/config at ${leaked}`,
);
contains("/dist/../../etc/passwd", "/dist/../../etc/passwd");
contains("percent-encoded ../ inside a segment", "/dist/%2e%2e/.git/config");
contains("percent-encoded slashes", "/dist/..%2f.git%2fconfig");
contains("traversal to a repo-root file", "/dist/../Cargo.toml");
contains("traversal out of web/", "/../Cargo.toml");
contains("plain traversal", "/../../etc/passwd");
contains("encoded traversal out of web/", "/%2e%2e/%2e%2e/etc/passwd");
contains("a doubled slash", "//dist//odderon.wasm");

console.log("\nnear misses that must not be mistaken for the rules");
// `/distfoo` is not `/dist`. A `startsWith("/dist")` test would call this build output.
const distfoo = contains("a path starting with the letters dist", "/distfoo/x.js");
ok("  ...and it is served from web/, not dist/", distfoo === null || distfoo.startsWith(WEB_ROOT), `served ${distfoo}`);
// A literal NUL byte cannot come through a URL (the parser percent-encodes it), so it is exercised
// directly. This is the string that defeats extension checks in code that hands it to a C library.
refuses("a literal NUL byte", "/index.html\u0000.png");
refuses("a percent-encoded NUL byte", "/index.html%00.png");

console.log("\nthe sibling-prefix trap — tested directly, because a URL cannot reach it");
// A URL parser collapses `..`, so `/../web-secrets/x.js` becomes `/web-secrets/x.js`, which really is
// inside web/. There is therefore no request that reaches the sibling-prefix case through
// resolveStaticPath — which is a good thing, and also means testing it through a URL would assert
// nothing. The rule is exercised where it lives instead.
ok("rejects a sibling directory sharing a prefix", isInside("/srv/web", "/srv/web-secrets/x") === false,
   "isInside said true");
ok("accepts a real child", isInside("/srv/web", "/srv/web/x") === true, "isInside said false");
ok("accepts the directory itself", isInside("/srv/web", "/srv/web") === true, "isInside said false");
ok("rejects the parent", isInside("/srv/web", "/srv") === false, "isInside said true");
// And the property that matters most, over every input in this file:
const probes = ["/", "/index.html", "/dist/x.js", "/dist/../.git/config", "/../../etc/passwd",
                "/dist/%2e%2e/Cargo.toml", "/distfoo/x.js", "//dist//x.wasm"];
const escaped = probes.map((p) => resolveStaticPath(p)).filter((p) => p !== null && !contained(p));
ok("no probe escapes web/ or dist/", escaped.length === 0, `escaped: ${escaped.join(", ")}`);

// ---------------------------------------------------------------------------------------------
// A structural check, because the bug was a copy-paste problem and not a logic problem in one place.
//
// Three files each hand-rolled a static server from `web/`. They disagreed: one was safe, one decided
// its root from the raw path (arbitrary-file-read), and one had no containment check at all. Fixing
// each separately would leave the next person free to write a fourth. So the rule is now mechanical:
// if a file serves the web root, it must go through the resolver, and it must not join a raw URL onto
// a root by hand.
// ---------------------------------------------------------------------------------------------
console.log("\nstructural — every static server goes through the resolver");
const serverFiles = readdirSync(new URL(".", import.meta.url))
  .filter((f) => f.endsWith(".mjs"))
  .map((f) => ({ f, src: readFileSync(new URL(f, import.meta.url), "utf8") }))
  // Match the import, not any mention: this file mentions "createServer" only in a string, and
  // scanning itself produced a false positive that took a debug cycle to explain.
  .filter(({ src }) => /from\s+["']node:http["']/.test(src) && /\bcreateServer\b/.test(src));

ok("found the servers to check", serverFiles.length >= 3, `only ${serverFiles.length} found`);

for (const { f, src } of serverFiles) {
  // Comments are stripped before the code is inspected. The first version of this check scanned the
  // raw source and flagged the very comments that explain the bug -- a checker that fails on prose is
  // a checker people learn to ignore, which is worse than not having one.
  const code = src.replace(/\/\*[\s\S]*?\*\//g, "").replace(/^\s*\/\/.*$/gm, "");

  ok(`${f} uses the shared resolver`, code.includes("resolveStaticPath"), "hand-rolled path handling");
  // The specific mistake: joining a request path onto a root. Two variants shipped, and both let `..`
  // through, because `join` normalises the path but the containment test that followed was a bare
  // prefix in one case and missing entirely in the other.
  const badJoin = /\bjoin\s*\(\s*(?:webRoot|root|here|webR\w*)\s*,\s*(?:req|url)\b/.test(code);
  ok(`${f} does not join the request path onto a root`, !badJoin, "found join(root, req.url)-style handling");
}

console.log(
  failures === 0
    ? `\nall ${checks} assertions passed`
    : `\n${failures} of ${checks} assertion(s) FAILED`,
);
process.exit(failures === 0 ? 0 : 1);
