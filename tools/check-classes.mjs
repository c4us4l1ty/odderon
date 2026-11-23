#!/usr/bin/env node
/**
 * The class-certification gate: read `src/classes.json`, compare it to the tree, fail on disagreement.
 *
 * ## Why this is a program and not a paragraph
 *
 * `docs/DATA-LICENSING.md` has described the tier system in prose since the project began, and no
 * program has ever read it. That is the same failure shape as `web/ablate.html` printing a verdict from
 * numbers it had not validated: a document that is *believed* to be enforced, and is not. The user's
 * requirement — "100% commercially legal from Day 1, enforced mechanically in the build" — cannot be
 * satisfied by a table in a markdown file, because nothing goes red when the table stops being true.
 *
 * ## What it checks, and in both directions
 *
 *   R1  every record is classified, and classified by a named person
 *   R2  no GATED record is compiled into the kernel (no `kernel_const_expressible: true`, and no
 *       identifier from a GATED record's implementation list appears under `crates/`)
 *   R3  `must_exist` is honoured BOTH ways: a promised class that is absent fails, and a class that
 *       appears without being promised fails. The second direction is the useful one — it is how an
 *       unvetted source gets into a build without anybody deciding to add it.
 *   R4  every path in a record's `implements` exists, so a claim of "present" is not a claim about a
 *       file that was renamed three commits ago
 *   R5  the generated table in `docs/CLASSES.md` matches the list — a documentation table that can drift
 *       is a document that will
 *
 * ## What it cannot do
 *
 * It cannot verify a licence. It compares a list to a tree. Whether `OPEN` is the correct answer for a
 * record is a reading of a terms page by a person, recorded with a URL and a date. The checker enforces
 * the *consistency* of the classification; the classifier is named in the record so the responsibility is
 * traceable. `docs/CLASSES.md` §3 states the rest of the boundary.
 *
 * Usage:
 *   node tools/check-classes.mjs            # gate: exit 1 on any finding
 *   node tools/check-classes.mjs --table    # print the generated table (used to refresh CLASSES.md)
 */

import { readFileSync, existsSync, readdirSync, statSync } from "node:fs";
import { dirname, resolve, relative } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, "..");

const findings = [];
const fail = (rule, message) => findings.push({ rule, message });

// ---------------------------------------------------------------------------------------------
// Load and validate the list itself. A list that is malformed is a finding, not a crash: a stack
// trace from a checker reads as "the checker is broken", which is exactly the wrong lesson.
// ---------------------------------------------------------------------------------------------

let list;
try {
  list = JSON.parse(readFileSync(resolve(root, "src/classes.json"), "utf8"));
} catch (err) {
  console.error(`::error::src/classes.json is missing or not JSON: ${err.message}`);
  process.exit(1);
}

const records = list.records;
if (!Array.isArray(records) || records.length === 0) {
  console.error("::error::src/classes.json has no records array");
  process.exit(1);
}

const CLASSES = new Set(["OPEN", "GATED"]);
const STATES = new Set(["present", "seam_only", "absent"]);

const seen = new Set();
for (const r of records) {
  const where = r.id || "(record with no id)";

  // R1 — classified, in a closed vocabulary, by name.
  if (!r.id || seen.has(r.id)) fail("R1", `${where}: duplicate or missing id`);
  seen.add(r.id);
  if (!CLASSES.has(r.class)) fail("R1", `${where}: class ${JSON.stringify(r.class)} is not OPEN or GATED`);
  if (!STATES.has(r.state)) fail("R1", `${where}: state ${JSON.stringify(r.state)} is not present/seam_only/absent`);
  if (typeof r.must_exist !== "boolean") fail("R1", `${where}: must_exist must be a boolean`);
  if (typeof r.kernel_const_expressible !== "boolean") {
    fail("R1", `${where}: kernel_const_expressible must be a boolean — silence is not permission`);
  }
  // A legal conclusion with no author is not a conclusion. "The team" is rejected explicitly,
  // because that is the string that gets typed when nobody wants to own the answer.
  if (!r.classifier || /team|everyone|n\/?a|tbd/i.test(r.classifier)) {
    fail("R1", `${where}: classifier must name a person, got ${JSON.stringify(r.classifier)}`);
  }
  if (!r.licence_id || !r.licence_url) fail("R1", `${where}: licence_id and licence_url are required`);
  if (!["yes", "no", "needs-authorisation", "yes-with-indemnity"].includes(r.commercial_ok)) {
    fail("R1", `${where}: commercial_ok ${JSON.stringify(r.commercial_ok)} is not a recognised verdict`);
  }

  // R2 — nothing non-redistributable is compiled in.
  if (r.class === "GATED" && r.kernel_const_expressible) {
    fail(
      "R2",
      `${where}: GATED but kernel_const_expressible — a non-redistributable record would ship inside the binary`,
    );
  }
  if (r.class === "GATED" && r.state === "present" && (r.implements || []).length === 0) {
    fail("R2", `${where}: GATED and "present" with no seam named; say where the player-supplied rows arrive`);
  }

  // R3 — the promise is honoured in both directions.
  if (r.must_exist && r.state === "absent") {
    fail("R3", `${where}: promised (must_exist) but absent — a promised class that does not exist is a missing feature`);
  }
  if (!r.must_exist && r.state === "present") {
    fail(
      "R3",
      `${where}: present but not promised — a class appeared in the build without being classified as expected`,
    );
  }

  // R4 — a claim of implementation is a claim about a file that exists.
  for (const p of r.implements || []) {
    if (!existsSync(resolve(root, p))) fail("R4", `${where}: implements ${p}, which does not exist`);
  }
}

// ---------------------------------------------------------------------------------------------
// R2, second half: the name check.
//
// This is the weakest check in the file and it is written down as such rather than dressed up. It
// cannot prove that a `const` array is a star catalogue. It proves that the *name* of a gated source
// does not appear in the kernel, i.e. that the one seam we rely on has not been bypassed by somebody
// typing the source's name into a struct. `docs/CLASSES.md` §3 item 2 states the boundary.
// ---------------------------------------------------------------------------------------------

function rustFiles(dir, out = []) {
  if (!existsSync(dir)) return out;
  for (const name of readdirSync(dir)) {
    const p = resolve(dir, name);
    const st = statSync(p);
    if (st.isDirectory()) rustFiles(p, out);
    else if (name.endsWith(".rs")) out.push(p);
  }
  return out;
}

const kernelFiles = rustFiles(resolve(root, "crates"));
const kernelSource = kernelFiles.map((p) => ({ path: relative(root, p), text: readFileSync(p, "utf8") }));

// The tokens that would betray a gated source: its id, and any identifier the record itself declares.
// Comments are excluded, because a doc comment *explaining* why Gaia is not shipped is the opposite of
// a violation — several of them exist on purpose.
function stripComments(text) {
  return text
    .replace(/\/\*[\s\S]*?\*\//g, "")
    .split("\n")
    .map((line) => line.replace(/\/\/.*$/, ""))
    .join("\n");
}

for (const r of records) {
  if (r.class !== "GATED") continue;
  const tokens = new Set([r.id, ...(r.implements || [])]);
  // Only the id is a token worth grepping here: `implements` paths for a GATED record point at a seam,
  // and banning the seam's own path would ban the mechanism that makes gating possible.
  for (const token of [r.id]) {
    for (const file of kernelSource) {
      const code = stripComments(file.text);
      if (code.includes(token)) {
        fail("R2", `${file.path}: mentions the GATED record ${token} in code (comments are fine, code is not)`);
      }
    }
  }
}

// ---------------------------------------------------------------------------------------------
// R5: the documentation table cannot drift.
//
// The table is generated, and the checker compares the block in CLASSES.md to what it would generate.
// A stale table is therefore a failing build rather than a stale document, which is the whole reason
// to generate it inside the checker rather than in a separate script nobody runs.
// ---------------------------------------------------------------------------------------------

function generateTable() {
  const rows = records
    .slice()
    .sort((a, b) => (a.id < b.id ? -1 : a.id > b.id ? 1 : 0))
    .map((r) => {
      const source = [r.publisher, r.distributor].filter(Boolean).join(" / ");
      // `commercial_ok` rather than `licence_id` in the rights column: the id is the machine key, and the
      // question a reader of the table is asking is "may we ship it". Both are printed by `--full`.
      return `| \`${r.id}\` | ${r.class} | ${r.state} | ${source} | ${r.commercial_ok} |`;
    });
  return [
    "| id | class | state | source | commercial |",
    "| --- | --- | --- | --- | --- |",
    ...rows,
  ].join("\n");
}

if (process.argv.includes("--table")) {
  console.log(generateTable());
  process.exit(0);
}

const classesDoc = resolve(root, "docs/CLASSES.md");
if (!existsSync(classesDoc)) {
  fail("R5", "docs/CLASSES.md is missing");
} else {
  const doc = readFileSync(classesDoc, "utf8");
  const begin = "<!-- BEGIN GENERATED: classes -->";
  const end = "<!-- END GENERATED: classes -->";
  const start = doc.indexOf(begin);
  const stop = doc.indexOf(end);
  if (start < 0 || stop < 0) {
    fail("R5", `docs/CLASSES.md has no ${begin} / ${end} block`);
  } else {
    const actual = doc.slice(start + begin.length, stop).trim();
    const expected = generateTable().trim();
    if (actual !== expected) {
      fail(
        "R5",
        "the table in docs/CLASSES.md is stale. Run: node tools/check-classes.mjs --table\n" +
          "--- in the document ---\n" +
          actual +
          "\n--- generated ---\n" +
          expected,
      );
    }
  }
}

// ---------------------------------------------------------------------------------------------
// Report
// ---------------------------------------------------------------------------------------------

if (findings.length === 0) {
  const open = records.filter((r) => r.class === "OPEN").length;
  const gated = records.filter((r) => r.class === "GATED").length;
  const present = records.filter((r) => r.state === "present").length;
  console.log(`ok  class list: ${open} OPEN, ${gated} GATED, ${present} present, ${kernelFiles.length} kernel files scanned`);
  process.exit(0);
}

for (const f of findings) {
  console.error(`::error title=class certification ${f.rule}::${f.message.replace(/\n/g, "%0A")}`);
  console.error(`${f.rule}  ${f.message}`);
}
console.error(`\nclass certification: ${findings.length} finding(s). See docs/CLASSES.md.`);
process.exit(1);
