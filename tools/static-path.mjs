/**
 * Map a request path onto a file inside one of two static roots — safely.
 *
 * ## Why this is a module and not four lines in each server
 *
 * `tools/serve.mjs` and `tools/check-shell.mjs` both serve `web/`, and both grew a special case for
 * `/dist/` so a build artifact could be fetched. Their special cases disagreed, and the disagreement
 * was a vulnerability rather than an inconsistency:
 *
 *   serve.mjs       tested `rel.startsWith("/dist/")` on the RAW request path, then computed
 *                   `join(root, rel)` and checked `startsWith(base)`. `/dist/../.git/config` passes the
 *                   prefix test, joins to `<repo>/.git/config`, passes the containment test, and is
 *                   served. Any file in the repository root was readable, `.git/config` included.
 *   check-shell.mjs tested the prefix on `new URL(req.url).pathname`, which has already collapsed
 *                   `..`, so `/dist/../.git/config` became `/.git/config`, failed the prefix test, and
 *                   was correctly refused.
 *
 * The check-shell version was safe by accident — it used the URL parser for its own convenience, not
 * for containment — so it was one refactor away from the bug in the other file. One implementation,
 * tested directly, is the only way that stays true.
 *
 * ## The rule
 *
 * Normalise FIRST (the way a browser would), decide the root SECOND, resolve THIRD, and prove
 * containment on the resolved path with a separator, not with a bare prefix. A bare `startsWith(base)`
 * accepts `/srv/web` for a file in `/srv/web-secrets`, which is the classic way this check is written
 * wrong.
 */

import { resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

export const WEB_ROOT = fileURLToPath(new URL("../web", import.meta.url));
export const REPO_ROOT = fileURLToPath(new URL("..", import.meta.url));
export const DIST_ROOT = resolve(REPO_ROOT, "dist");

/**
 * Is `full` inside directory `base`? A bare `startsWith` is the classic way to get this wrong: it
 * accepts `<repo>/web-secrets/x` for base `<repo>/web`. The separator is what makes it a directory
 * test rather than a string test, and it is exported so the rule can be tested directly instead of
 * only through a URL that may or may not be able to reach the case.
 */
export function isInside(base, full) {
  return full === base || full.startsWith(base + sep);
}

/**
 * @param {string | undefined} rawUrl  the request path, query string allowed
 * @param {object} [roots]
 * @param {string} [roots.webRoot]   directory served for ordinary paths
 * @param {string} [roots.distRoot]  directory served for `/dist/*`
 * @param {string} [roots.index]     file served for `/`
 * @returns {string | null} absolute file path, or null if the request must be refused
 */
export function resolveStaticPath(rawUrl, roots = {}) {
  const webRoot = resolve(roots.webRoot ?? WEB_ROOT);
  const distRoot = resolve(roots.distRoot ?? DIST_ROOT);
  const index = roots.index ?? "index.html";

  // Take the pathname off with the URL parser. It collapses `.` and `..` exactly as a browser does,
  // which is the whole point: the traversal has to be gone BEFORE the root is chosen, or the root is
  // chosen by a string the attacker controls. It also strips `?query` and `#fragment` for us.
  let pathname;
  try {
    pathname = new URL(rawUrl ?? "/", "http://x").pathname;
  } catch {
    return null;                       // unparseable request target
  }

  // A NUL byte is the classic way to defeat extension checks in code that hands the string to a C
  // library. There is no legitimate reason for one in a request path. Both forms have to be caught:
  // the URL parser percent-encodes a literal NUL, so a request carrying one arrives as `%00` and the
  // literal check alone never fires. (The test that caught this asserted the literal and reported a
  // pass-through, which is exactly the sort of "looks handled" gap this module exists to close.)
  if (pathname.includes("\0") || /%00/i.test(pathname)) return null;

  if (pathname === "/" || pathname === "") pathname = "/" + index;

  // `/dist/` and `/dist` are both build output. `/dist-secrets/` must NOT be, which is why this is a
  // segment test and not a `startsWith("/dist")`.
  const isDist = pathname === "/dist" || pathname.startsWith("/dist/");
  const base = isDist ? distRoot : webRoot;
  const rel = isDist ? pathname.slice("/dist".length) : pathname;

  const full = resolve(base, "." + rel);

  // Containment on the resolved path. `full === base` is a directory request, not a file, and the
  // caller's stat() rejects it — but it is not a traversal, so it is returned as-is.
  return isInside(base, full) ? full : null;
}
