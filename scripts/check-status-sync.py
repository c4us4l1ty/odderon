#!/usr/bin/env python3
"""Fail when public status claims drift behind shipped wire/product constants."""
from pathlib import Path
import re

root = Path(__file__).resolve().parents[1]
read = lambda path: (root / path).read_text(encoding="utf-8")

fdr = read("crates/odd-report/src/fdr.rs")
onboarding = read("web/onboarding.js")
contracts = read("crates/odd-contracts/src/lib.rs")
status = read("docs/project-status.md")
story_path = root / "docs/onboarding-storyboard.md"
story = story_path.read_text(encoding="utf-8") if story_path.exists() else ""

schema_match = re.search(r"pub const SCHEMA_VERSION: u16 = (\d+);", fdr)
if not schema_match:
    raise SystemExit("could not read SCHEMA_VERSION")
schema = int(schema_match.group(1))

lesson_block = onboarding.split("export const LESSONS", 1)[1].split("]);", 1)[0]
lessons = len(re.findall(r"\btitle:\s*'", lesson_block))
contract_count = len(re.findall(r"\n\s*id:\s*10[1-9],", contracts))

errors = []
if f"schema {schema}" not in status.lower():
    errors.append(f"project status does not name current schema {schema}")
contract_word = {3: "three"}.get(contract_count, str(contract_count))
if not re.search(rf"\b(?:{contract_count}|{contract_word})\s+(?:browser\s+)?mission contracts\b", status, re.I):
    errors.append(f"project status does not claim {contract_count} mission contracts")
word = {6: "six", 7: "seven"}.get(lessons, str(lessons))
if not re.search(rf"\b(?:{lessons}|{word})[- ]lesson\b", status, re.I):
    errors.append(f"project status does not claim {lessons} lessons")
if story and not re.search(rf"Lessons?\s+1[–-]{lessons}\b", story, re.I):
    errors.append(f"storyboard does not cover Lessons 1–{lessons}")

workflow_files = list((root / ".github/workflows").glob("*.yml"))
for workflow in workflow_files:
    text = workflow.read_text(encoding="utf-8")
    if "cancel-in-progress: true" not in text or "github.workflow" not in text or "github.ref" not in text:
        errors.append(f"{workflow.relative_to(root)} lacks canceling workflow/ref concurrency")

if errors:
    raise SystemExit("status sync failed:\n- " + "\n- ".join(errors))
print(f"oracle_status_doc_and_workflow_sync ok: schema={schema}, contracts={contract_count}, lessons={lessons}, workflows={len(workflow_files)}")
