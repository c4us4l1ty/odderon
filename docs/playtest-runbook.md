# External onboarding playtest runbook

This runbook turns the shipped placement diagnostic and Lessons 1–7 into a
repeatable external study without hidden tracking. It does not authorize
network telemetry. Participants decide whether to record locally and whether to
hand an export to the facilitator.

## Question and first cohort

**Question:** Can a new player find an appropriate starting point, complete the
orbital and Schema-5 mass-budget lessons, and explain why fitted dry mass changes
remaining propellant?

Use 5–8 participants for the first cohort:

- at least three with no orbital-mechanics experience;
- at least two who have played a spaceflight or engineering game;
- no contributor who already knows Odderon's controls in the primary cohort.

This is a usability sample, not a statistically representative population. Do
not turn completion percentages from one small cohort into marketing claims.

## Build and environment

1. Test the exact public commit recorded with the session notes.
2. Serve the built WebAssembly browser application over HTTP(S), not `file://`.
3. Use a clean browser profile for each participant.
4. Do not configure a production authority key. The study is practice-only.
5. Confirm the mission page reaches zero animation callbacks and worker messages
   after two idle seconds before the first session.

Record only outside the application:

- anonymous participant label such as `P01`;
- tested commit;
- browser and viewport class;
- facilitator-observed technical failures.

Do not record names, email addresses, voice, screen video or demographic data as
part of the Odderon observation export.

## Consent script

Read before touching the playtest checkbox:

> Odderon can keep a structured observation log only in this browser. Recording
> is off by default. If you opt in, nothing is sent anywhere; you can inspect,
> export or clear it. At the end, you may choose whether to give the exported
> JSON file to us. Declining does not affect the session.

The facilitator must not check the box for the participant. A participant may
uncheck it or clear data at any time. If consent is declined, continue the
session and keep no substitute event log.

## Session procedure

### 1. Unprompted placement

- Open the mission page and ask the participant to choose between **Start guided
  flight**, **Find My Starting Point**, and **Skip for now**.
- Do not explain the three diagnostic answers.
- If the participant chooses placement, allow all deterministic routing to run.
- Record no manual correction to the route. A wrong route is product evidence.

### 2. Lessons

- Ask the participant to think aloud, but do not define controls or acronyms
  before the interface does.
- Intervene only for a technical failure or after the participant explicitly
  says they cannot continue.
- When asked for help, first direct them to **Show a hint**. Do not bypass the
  four-rung sequence.
- Lesson 7 is complete only after fitted dry mass beats the all-Chief baseline,
  remaining-propellant error is below 2 kg, the Contract 103 objective passes,
  efficiency exceeds 0.65, and the current Schema-5 recorder passes Level-3
  replay.

### 3. Exit questions

Ask, without teaching first:

1. Why can a lower orbit catch a target that is ahead?
2. What changed when a Chief-AI department was returned to manual control?
3. If fitted dry mass falls but requested velocity change stays fixed, what
   happens to propellant consumed and why?
4. Which screen or phrase was hardest to understand?
5. Would you rather next operate ranked missions or build and fly a launch from
   the surface?

Keep answers as anonymous facilitator notes only if the participant separately
agrees. They are not fields in `PlaytestSessionLog`.

### 4. Export or destroy

- If the participant chooses to contribute the local log, they click **Export
  Playtest Observation JSON** and hand over that file.
- Rename contributed files `P01-session.json`, `P02-session.json`, and so on;
  the mapping must not contain identifying information.
- The participant then clicks **Clear Playtest Data**.
- If they decline export, click **Clear Playtest Data** and retain nothing.

## Validate and summarize exports

Keep raw exports out of Git. Store them in an access-controlled study folder.
Validate and aggregate copies with:

```sh
cargo run --locked -p odd-app --bin playtest-summary -- \
  /path/to/cohort/P*-session.json > cohort-summary.md
```

A malformed, unsupported or non-finite log fails closed. Review the generated
Markdown for:

- per-lesson completion rate;
- median time to threshold;
- top causal failure rule;
- Schema-5 prediction-error improvement.

Also count placement routes and exact `drop_off_step` values from the consented
exports. Never fill missing logs with facilitator estimates.

## Decision rules after the first cohort

Perform one onboarding refinement pass before choosing a major work package if
any of these occur:

- fewer than 4 of the first 5 eligible participants complete Lesson 1;
- more than half route to a lesson but cannot identify its highlighted control;
- median hint use reaches rung 4 in any lesson;
- two or more participants drop at the same threshold;
- a Lesson-7 completer cannot explain the direction of the mass/propellant
  relationship in the exit question;
- median final Schema-5 prediction error does not improve from the recorded
  pre-redesign error.

After that refinement and a small confirmation cohort:

- prefer **ranked authority deployment** if players complete the loop and ask
  primarily for competition, persistence or public records;
- prefer **full ascent** if they understand the loop but consistently ask for a
  launch-to-orbit beginning;
- choose neither yet if placement or Lesson 7 remains a repeated bottleneck.

## Session report template

```text
Commit:
Cohort size / contributed exports:
Browser and viewport classes:
Placement route counts:
Completed lessons by participant label:
Shared drop-off thresholds:
Top causal rules:
Median Schema-5 error improvement:
Technical failures:
Changes proposed for one refinement pass:
Evidence for ranked authority, ascent, or neither:
```
