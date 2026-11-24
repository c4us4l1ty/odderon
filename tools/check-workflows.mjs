#!/usr/bin/env node
/**
 * Validate every workflow file parses, because a workflow that does not parse fails *silently*.
 *
 * ## Why this exists
 *
 * On 2026-11-24 a step named `temporal cache (before: blit only, after: full shade)` was committed to
 * `ci.yml`. The colon inside the unquoted value makes the line invalid YAML, and the failure mode is
 * the bad one: GitHub does not run the workflow, does not report a red job, and does not annotate a
 * line. The run page says "This run likely failed because of a workflow file issue" and nothing else —
 * no job names, no logs, no checks on the PR (`gh pr checks` reports "no checks reported").
 *
 * That cost a push and a confused look at a green-looking branch. A workflow is code, and code that
 * cannot be parsed should fail where somebody can read the error.
 *
 * ## What it does NOT do
 *
 * It does not validate semantics — action versions, step inputs, expression syntax, whether
 * `npm install` is needed. Only that each file loads as YAML and has the two keys a workflow needs.
 * A checker that claims more than it checks is the failure this repository is organised against.
 *
 * Usage: node tools/check-workflows.mjs
 */

import { readdirSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const dir = fileURLToPath(new URL("../.github/workflows", import.meta.url));

let yaml;
try {
  yaml = await import("js-yaml");
} catch {
  // js-yaml is installed by `npm install` for the tools, but this check must not be the thing that
  // breaks a push when it is absent. Skipping loudly beats failing for the wrong reason.
  console.error("js-yaml is not installed; run `npm install` first. Skipping workflow validation.");
  process.exit(0);
}

const files = readdirSync(dir).filter((f) => /\.ya?ml$/.test(f)).sort();
let failures = 0;

for (const f of files) {
  const path = new URL(f, `file://${dir}/`);
  let doc;
  try {
    doc = yaml.load(readFileSync(path, "utf8"));
  } catch (e) {
    failures += 1;
    // The parser gives the line, the column, and a snippet. Print them, because the whole point is
    // that GitHub would have told us none of this.
    console.log(`FAIL  ${f}: ${e.message.split("\n")[0]}`);
    for (const line of e.message.split("\n").slice(1, 8)) console.log(`        ${line}`);
    continue;
  }
  if (!doc || typeof doc !== "object") {
    failures += 1;
    console.log(`FAIL  ${f}: not a mapping`);
    continue;
  }
  if (!doc.jobs || typeof doc.jobs !== "object") {
    failures += 1;
    console.log(`FAIL  ${f}: has no jobs:`);
    continue;
  }
  const jobs = Object.keys(doc.jobs);
  let steps = 0;
  const colonNames = [];
  for (const j of jobs) {
    for (const st of doc.jobs[j].steps ?? []) {
      steps += 1;
      // This is the actual bug, kept as a named check so the error says what to do. YAML allows a
      // colon in a plain scalar only when no space follows it, so `a: b` inside a name has to be
      // quoted. The validator above would catch it anyway; naming it here explains it.
      if (st.name && /:\s/.test(st.name)) colonNames.push(`${j} :: ${st.name}`);
    }
  }
  if (colonNames.length) {
    failures += 1;
    for (const n of colonNames) console.log(`FAIL  ${f}: step name has an unquoted ': ' -- ${n}`);
    continue;
  }
  console.log(`ok    ${f}: ${jobs.length} jobs, ${steps} steps (${jobs.join(", ")})`);
}

console.log(
  failures === 0
    ? `\nall ${files.length} workflow file(s) parse`
    : `\n${failures} workflow file(s) FAILED — GitHub would refuse to run these`,
);
process.exit(failures === 0 ? 0 : 1);
