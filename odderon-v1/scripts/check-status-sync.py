#!/usr/bin/env python3
"""Fail when public status claims drift behind shipped wire/product constants."""
from pathlib import Path
import json
import re

root = Path(__file__).resolve().parents[1]
read = lambda path: (root / path).read_text(encoding="utf-8")

fdr = read("crates/odd-report/src/fdr.rs")
onboarding = read("web/onboarding.js")
contracts = read("crates/odd-contracts/src/lib.rs")
status = read("docs/project-status.md")
status_facts = json.loads(read("docs/project-status.json"))
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
expected_facts = {
    "schema_version": schema,
    "mission_contract_count": contract_count,
    "onboarding_lesson_count": lessons,
}
for key, actual in expected_facts.items():
    if status_facts.get(key) != actual:
        errors.append(f"project-status.json {key} is {status_facts.get(key)!r}, shipped value is {actual!r}")
accepted_main = status_facts.get("accepted_main", "")
accepted_pr = status_facts.get("accepted_pr")
hosted_ci = status_facts.get("hosted_ci")
if not re.fullmatch(r"[0-9a-f]{40}", accepted_main):
    errors.append("project-status.json accepted_main is not a full commit id")
if accepted_main and accepted_main not in status:
    errors.append("project status prose does not name accepted_main")
if accepted_pr and not re.search(rf"\bPR\s+#?{accepted_pr}\b", status):
    errors.append("project status prose does not name accepted_pr")
if hosted_ci not in {"green", "blocked", "red"}:
    errors.append("project-status.json hosted_ci must be green, blocked or red")
if hosted_ci == "green" and not re.search(r"hosted\s+GitHub Actions[^.]*passed|hosted[^.]*green", status, re.I):
    errors.append("project status prose does not report green hosted CI")
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

repo_root = root.parent
workflow_files = list((repo_root / ".github/workflows").glob("*.yml"))
for workflow in workflow_files:
    text = workflow.read_text(encoding="utf-8")
    if "cancel-in-progress: true" not in text or "github.workflow" not in text or "github.ref" not in text:
        errors.append(f"{workflow.relative_to(repo_root)} lacks canceling workflow/ref concurrency")

if errors:
    raise SystemExit("status sync failed:\n- " + "\n- ".join(errors))
print(f"oracle_status_doc_and_workflow_sync ok: baseline={accepted_main[:8]}, ci={hosted_ci}, schema={schema}, contracts={contract_count}, lessons={lessons}, workflows={len(workflow_files)}")
