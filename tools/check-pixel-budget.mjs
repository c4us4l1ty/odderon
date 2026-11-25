#!/usr/bin/env node
/**
 * The pixel clause, the constant, and the doc must be one number derived one way.
 *
 * ## Why this exists
 *
 * `MAX_PIXELS` was a bare `1_150_000`. It had no derivation in the code, it sat above the 1.228 Mpx
 * that a 1.5 ms budget actually buys on Reference B, and it disagreed with `GRAPHICS.md`, which named
 * the clause as **0.68 Mpx**. Both numbers were "documented"; neither was true to the other, and the
 * disagreement had been open for some time -- `GRAPHICS.md` said so, in the words "the code and the doc
 * have been disagreeing for some time; that disagreement is **still open**". A doc that honestly
 * records a disagreement it never resolves is a doc that has learned to live with a wrong number.
 *
 * Worse, the 0.68 Mpx figure was itself a ghost: it is `1.5 ms / (2.29 ms at 1.04 Mpx)`, and the
 * 2.29 ms texture-read row is one this project **retracted** after re-measuring the same device at
 * 1.65 ms. So the doc's clause was derived from a retracted measurement, and the code's constant was
 * derived from nothing at all.
 *
 * ## What it checks
 *
 * Three statements of one quantity, each parsed from where it actually lives:
 *
 *   1. the declaration -- `CLAUSE_MPX` and `MAX_PIXELS` in `web/index.html`;
 *   2. the doc -- the clause and the measurement row in `docs/GRAPHICS.md`;
 *   3. the arithmetic -- 1.5 ms / (the measured ms/Mpx) / (1 + margin).
 *
 * If any two disagree, this fails. That is the difference between a resolved disagreement and a
 * documented one: nothing here is a promise that a human will notice.
 *
 * It also refuses the specific wrong derivations, because "no silent numeric fallback" is a standing
 * constraint: a clause derived from the retracted 2.29 ms row must not pass, and neither may a clause
 * larger than what the budget buys with no margin at all.
 *
 * Usage: node tools/check-pixel-budget.mjs
 * No browser needed -- arithmetic over source text, so it runs in any job.
 */

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const HERE = dirname(fileURLToPath(import.meta.url));
const ROOT = join(HERE, "..");

const checks = [];
const fail = (m) => checks.push(["FAIL", m]);
const ok = (m) => checks.push(["ok  ", m]);

// --- the inputs, as the docs state them --------------------------------------------------------
const BUDGET_MS = 1.5;        // R2: "frame cost must fall below 1.5 ms"
const MEASURED_MS = 1.27;     // GRAPHICS.md measured table: blitFramebuffer, no shader
const MEASURED_MPX = 1.04;    // the resolution that row was measured at
const MARGIN = 0.15;          // BUILD-PLAN.md: "leaving ~15% margin on the larger of the two"
const RETRACTED_MS = 2.29;    // the withdrawn texture-read row, kept so its clause can be refused

const derived = (BUDGET_MS / (MEASURED_MS / MEASURED_MPX)) / (1 + MARGIN);
const reachNoMargin = BUDGET_MS / (MEASURED_MS / MEASURED_MPX);
const fromRetracted = BUDGET_MS / (RETRACTED_MS / MEASURED_MPX);

// --- 1. the declaration ------------------------------------------------------------------------
const indexHtml = readFileSync(join(ROOT, "web", "index.html"), "utf8");

const clauseMatch = indexHtml.match(/const CLAUSE_MPX = ([\d.]+);/);
const maxMatch = indexHtml.match(/const MAX_PIXELS = Math\.round\(CLAUSE_MPX \* 1e6\);/);

if (!clauseMatch) {
  fail("web/index.html has no `const CLAUSE_MPX = <number>;` -- the clause must be a named constant");
} else if (!maxMatch) {
  fail("MAX_PIXELS is not derived from CLAUSE_MPX in web/index.html");
} else {
  const declared = Number(clauseMatch[1]);
  if (Math.abs(declared - derived) > 0.0005) {
    fail(
      `CLAUSE_MPX = ${declared} but the measurement derives ${derived.toFixed(4)} -- ` +
        `1.5 ms / (${MEASURED_MS} ms at ${MEASURED_MPX} Mpx) / (1 + ${MARGIN})`,
    );
  } else {
    ok(`CLAUSE_MPX = ${declared} matches the derivation (${derived.toFixed(4)} Mpx)`);
  }

  if (Math.abs(declared - fromRetracted) < 0.01) {
    fail(
      `CLAUSE_MPX = ${declared} is the clause derived from the RETRACTED ${RETRACTED_MS} ms ` +
        `texture-read row (${fromRetracted.toFixed(3)} Mpx). That measurement was withdrawn.`,
    );
  } else if (declared > reachNoMargin + 1e-9) {
    fail(
      `CLAUSE_MPX = ${declared} exceeds ${reachNoMargin.toFixed(3)} Mpx, what 1.5 ms buys with NO ` +
        `margin at all -- the clause would be unreachable by its own derivation.`,
    );
  } else {
    ok(`CLAUSE_MPX = ${declared} is inside the no-margin reach ${reachNoMargin.toFixed(3)} Mpx`);
  }
}

// --- 2. the doc --------------------------------------------------------------------------------
const graphics = readFileSync(join(ROOT, "docs", "GRAPHICS.md"), "utf8");

if (/clause is therefore \*\*0\.68 Mpx\*\*/.test(graphics)) {
  fail(
    "docs/GRAPHICS.md still names the clause as 0.68 Mpx. That figure is 1.5 ms / (2.29 ms at " +
      "1.04 Mpx), and the 2.29 ms row is retracted; a clause may not rest on a withdrawn measurement.",
  );
} else {
  ok("docs/GRAPHICS.md no longer derives the clause from the retracted 2.29 ms row");
}

const docClause = graphics.match(/clause is (?:now )?\*\*([\d.]+) Mpx\*\*/);
if (!docClause) {
  fail("docs/GRAPHICS.md does not state the clause as `clause is **<n> Mpx**`");
} else if (clauseMatch) {
  const docN = Number(docClause[1]);
  const declared = Number(clauseMatch[1]);
  if (Math.abs(docN - declared) > 0.0005) {
    fail(`docs/GRAPHICS.md says the clause is ${docN} Mpx; web/index.html declares ${declared}`);
  } else {
    ok(`docs/GRAPHICS.md and web/index.html agree the clause is ${docN} Mpx`);
  }
}

if (!/\| `blitFramebuffer`, no shader \| \*\*1\.27 ms\*\* \|/.test(graphics)) {
  fail("docs/GRAPHICS.md does not show the 1.27 ms blitFramebuffer row the clause is derived from");
} else {
  ok("docs/GRAPHICS.md shows the 1.27 ms blitFramebuffer row the clause derives from");
}

// --- report ------------------------------------------------------------------------------------
for (const [tag, msg] of checks) console.log(`  ${tag} ${msg}`);
console.log("");
if (checks.some(([t]) => t === "FAIL")) {
  console.log("pixel budget check FAILED");
  process.exit(1);
}
console.log(`pixel budget check passed  (clause ${derived.toFixed(3)} Mpx = 1.5 ms / ` +
  `(${MEASURED_MS} ms at ${MEASURED_MPX} Mpx) / ${1 + MARGIN})`);
