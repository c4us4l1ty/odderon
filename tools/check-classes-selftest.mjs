#!/usr/bin/env node
/**
 * Self-test for `tools/check-classes.mjs`: prove it fails when it should.
 *
 * ## Why a gate needs its own gate
 *
 * `check-ablation.mjs` exists because a report printed a verdict from numbers it had not validated.
 * A licence checker has the same shape of hazard one level up: it prints `ok` and everybody infers
 * compliance. A checker that *cannot fail* prints `ok` too, and nobody finds out until a lawyer does.
 *
 * So the failure modes are enumerated and each one is injected:
 *
 *   1. a GATED record marked `kernel_const_expressible`      → R2
 *   2. a GATED record's id in kernel code                    → R2
 *   3. a promised class that is absent                       → R3
 *   4. a class that appears without being promised           → R3
 *   5. a record with no classifier, or classifier "team"     → R1
 *   6. an `implements` path that does not exist              → R4
 *   7. a stale generated table in the document                → R5
 *
 * Case 3 and case 4 are the pair that matters: a checker that only ever complains about *missing*
 * things would happily let an unvetted dataset in, which is the failure the user's Day-1 requirement is
 * about. Both directions are exercised.
 *
 * The checker is run as a subprocess against a **copy of the repository's list in a temp directory**, so
 * the real tree is never modified and a failure here cannot corrupt `src/classes.json`.
 *
 * Usage: node tools/check-classes-selftest.mjs
 */

import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, writeFileSync, mkdirSync, rmSync, cpSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, resolve, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, "..");

const base = JSON.parse(readFileSync(resolve(root, "src/classes.json"), "utf8"));
const work = mkdtempSync(join(tmpdir(), "odderon-classes-"));

// A minimal tree the checker can run against: the real checker script, a `crates/` directory with one
// Rust file, and a classes document. Copying the whole repository would make this test slow for no gain;
// copying the checker itself means the test cannot drift from the thing it tests.
function scaffold() {
  mkdirSync(join(work, "tools"), { recursive: true });
  cpSync(resolve(root, "tools/check-classes.mjs"), join(work, "tools/check-classes.mjs"));
  mkdirSync(join(work, "src"), { recursive: true });
  mkdirSync(join(work, "crates/odderon-astro/src"), { recursive: true });
  // Every path any record claims to implement, so the clean case is clean: the scaffold has to contain the
  // files the base list names or R4 would fire on every case. Derived from the list rather than hardcoded,
  // because a hardcoded `catalogue.rs` made this self-test fail the moment a second record was added that
  // implemented something else -- a gate that breaks when the thing it guards grows is a gate nobody keeps.
  for (const record of base.records ?? []) {
    for (const target of record.implements ?? []) {
      const file = join(work, target);
      mkdirSync(dirname(file), { recursive: true });
      writeFileSync(file, "//! Stated in miniature, for the checker to find.\n");
    }
  }
  mkdirSync(join(work, "docs"), { recursive: true });
  writeFileSync(
    join(work, "crates/odderon-astro/src/lib.rs"),
    '//! A kernel.\nfn main_thing() -> f64 { 1.0 } // comments mention gaia-dr3 on purpose\n',
  );
  writeFileSync(join(work, "docs/CLASSES.md"), "<!-- BEGIN GENERATED: classes -->\nx\n<!-- END GENERATED: classes -->\n");
}

/**
 * Run the checker in the scaffold against a `mutate`d list; return `{code, out}`.
 *
 * `afterDoc` runs *after* the documentation table has been regenerated, which is the only way to test a
 * stale-document finding: regenerating after the case set the document would silently repair the very
 * condition under test, and the first version of this file did exactly that and reported the R5 case as
 * passing when it had in fact been overwritten.
 */
function run(mutate, afterDoc) {
  const list = JSON.parse(JSON.stringify(base));
  if (mutate) mutate(list);
  writeFileSync(join(work, "src/classes.json"), JSON.stringify(list, null, 2));
  // The document table is regenerated from the *unmutated* list for most cases; the R5 case sets it stale
  // deliberately, so the table is regenerated here from the current list and the R5 case overrides it.
  try {
    const table = execFileSync(process.execPath, [join(work, "tools/check-classes.mjs"), "--table"], {
      encoding: "utf8",
    });
    writeFileSync(
      join(work, "docs/CLASSES.md"),
      `<!-- BEGIN GENERATED: classes -->\n${table.trim()}\n<!-- END GENERATED: classes -->\n`,
    );
  } catch {
    /* --table cannot fail on a well-formed list; if it does, the case below will report it */
  }
  if (afterDoc) afterDoc();
  try {
    const out = execFileSync(process.execPath, [join(work, "tools", "check-classes.mjs")], {
      encoding: "utf8",
      stdio: "pipe",
    });
    return { code: 0, out };
  } catch (err) {
    return { code: err.status ?? 1, out: `${err.stdout ?? ""}${err.stderr ?? ""}` };
  }
}

scaffold();

const cases = [];
const expect = (name, rule, mutate, afterDoc) => cases.push({ name, rule, mutate, afterDoc });

expect("a clean list passes", null, null);

expect("GATED compiled into the kernel", "R2", (l) => {
  l.records.find((r) => r.id === "gaia-dr3").kernel_const_expressible = true;
});

expect("GATED record named in kernel code", "R2", (l) => {
  writeFileSync(
    join(work, "crates/odderon-astro/src/leak.rs"),
    "pub const SOURCE: &str = \"gaia-dr3\";\n",
  );
});

expect("GATED record named only in a comment is allowed", null, () => {
  // The scaffold's lib.rs already has `// comments mention gaia-dr3 on purpose`, and nothing else.
});

expect("a promised class that is absent", "R3", (l) => {
  l.records.find((r) => r.id === "body-constants").state = "absent";
});

expect("a class present without being promised", "R3", (l) => {
  l.records.find((r) => r.id === "small-bodies").state = "present";
});

expect("no classifier at all", "R1", (l) => {
  delete l.records.find((r) => r.id === "thermo-cea").classifier;
});

expect("classifier is 'the team'", "R1", (l) => {
  l.records.find((r) => r.id === "thermo-cea").classifier = "the team";
});

expect("an implements path that does not exist", "R4", (l) => {
  l.records.find((r) => r.id === "body-constants").implements.push("crates/odderon-astro/src/imaginary.rs");
});

expect(
  "a stale generated table",
  "R5",
  null,
  () => {
    writeFileSync(
      join(work, "docs/CLASSES.md"),
      "<!-- BEGIN GENERATED: classes -->\n| id | class | state | source | commercial |\n| --- | --- | --- | --- | --- |\n| `outdated` | OPEN | absent | nowhere | yes |\n<!-- END GENERATED: classes -->\n",
    );
  },
);

let failures = 0;
for (const c of cases) {
  // Every case starts from the scaffold with no leaked file, so case 2 cannot poison later cases.
  try {
    rmSync(join(work, "crates/odderon-astro/src/leak.rs"));
  } catch {
    /* absent is the normal state */
  }
  const { code, out } = run(c.mutate, c.afterDoc);
  const passed = c.rule === null ? code === 0 : code !== 0 && out.includes(c.rule);
  if (!passed) {
    failures += 1;
    console.error(`FAIL  ${c.name}`);
    console.error(`      expected ${c.rule === null ? "exit 0" : `a ${c.rule} finding`}, got exit ${code}`);
    console.error(
      out
        .trim()
        .split("\n")
        .map((l) => `      | ${l}`)
        .join("\n"),
    );
  } else {
    console.log(`ok    ${c.name}${c.rule ? ` → ${c.rule}` : ""}`);
  }
}

rmSync(work, { recursive: true, force: true });

if (failures > 0) {
  console.error(`\nclass-certification self-test: ${failures} of ${cases.length} cases did not fail as required`);
  process.exit(1);
}
console.log(`\nclass-certification self-test: ${cases.length} cases, all behave as required`);
