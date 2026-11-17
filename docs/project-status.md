# Project status — 2026-11-17

This is the current implementation boundary. Vision documents describe intended
systems; this page says what a player and an operator can actually use.

## Shipped in the repository

- An Earth–Moon observatory and three browser mission contracts.
- Deterministic Rust physics shared by native, WebAssembly and authority replay.
- Multi-node planning, finite-burn propagation, perturbations, impact/entry,
  numerical perigee targeting, prediction locking, causal explanations and
  exact four-player Shapley attribution.
- Physical engine design, cryogenic fit-out, instantaneous electrical-bus
  allocation, Chief-AI delegation and v4 coupled propellant/thermal flight.
- Browser-local pilot identity, signed FDRs, replay verification and dossiers.
- Server-authoritative attempt commitments, receipts, Elo, callsigns,
  leaderboard and public replay, with native SQLite and Cloudflare-compatible
  D1/R2 adapters.
- Patched lunar-encounter preview and B-plane inspection/targeting. B-plane
  targeting is a practice planning aid; it is not a lunar-capture optimizer.

These form a deep vertical slice, not the complete civilization-scale game.

## Foundations that are not player features

The tree contains WGS84 launch-site state generation, bounded Earth-orientation
inputs and a gravity-turn guidance law. No production flow calls the launch-site
or gravity-turn APIs. There is no surface-launch contract, pad hold-down,
staging, throttle program, ascent UI or signed ascent replay. Describe this as
an **ascent foundation**, not a shipped launch system.

The Level-1 conic planner intentionally excludes thermal loss. Level 3 and FDR
schema 4 integrate tanks, venting and engine depletion in one coupled state.
There is no second callback-based approximation in the preview; the modeled
flight is the authority on thermal feasibility.

The mission page now includes the initial measured six-lesson onboarding flow:
it is opt-in, checks real mission state, offers four hint rungs and stores
progress only after explicit local consent. The full visual storyboard,
placement test and playtest protocol in [`onboarding.md`](onboarding.md) remain
unimplemented acceptance work.

## Deployment boundary

No production authority deployment is represented by this repository:

- `crates/odd-worker-cloudflare/wrangler.toml` still requires a real D1 ID;
- `web/authority-config.js` has no production public-key pin by default;
- production key custody, migrations, backups, rate limits, monitoring,
  rollback and safe R2 orphan cleanup are operator work.

A static deployment is practice-only. Miniflare/workerd tests and Wrangler
`--dry-run` are not production evidence.

## Verification state

The latest merge to `main`, `1002748d8a40490996b3a507dad830d7143b52e7`,
was not executed by GitHub Actions: GitHub refused to start the job because the
account's payment or spending limit needs attention. This is an infrastructure
failure, not a test failure, but it means that merge has no independent hosted
acceptance result. Restore Actions billing and rerun the workflow.

Local acceptance remains `./scripts/gate.sh`. On 2026-11-17 the complete gate
passed on branch commit `57920b0`: formatting, clippy, workspace and named v4
oracles, native/WASM/Chrome parity, browser and idle checks, assembly, native
SQLite authority, workerd D1/R2 authority and concurrency/fault tests. A green
gate covers implemented contracts and selected browser/runtime paths; it is not
proof of real-world flight accuracy, distributed production behavior or all
hardware.

## Deliberate sacrifices and known defects

- Scientific models are deterministic and inspectable rather than operationally
  complete. Ephemeris coverage, orientation, atmosphere, tanks and propulsion
  have documented bounds.
- Contract 103 reaches lunar-distance apoapsis but its reference trajectory does
  not encounter or capture the Moon.
- First-attempt ranking is per public key, not per human; it is neither Sybil-
  proof nor proof that no external simulator was used before commitment.
- Incremental authority replay was dropped. `/submit-segment` performs a full
  replay before checkpoint-chain checks.
- The frozen score uses catalog dry mass in its fuel threshold and prediction
  term. Fitted hardware can therefore create a measured score/display mismatch.
  Fixing it requires a versioned scoring migration, not a silent formula edit.
- Production operations and the broader campaign/economy remain out of scope.

## Ordered next work

1. Restore hosted CI and obtain a real green run.
2. Keep auditing new kernels through the shipped browser and signed-record path;
   direct library tests alone are not acceptance.
3. Complete and playtest the detailed onboarding storyboard. Keep its scored
   lesson on the unchanged baseline Contract 101 until scoring semantics are
   explicitly versioned; do not teach redesign scores through the known
   catalog-dry-mass defect.
4. Use local, opt-in observations to refine that path before adding another
   major physics subsystem.
5. Choose one operational direction: deploy ranked authority with a real
   runbook, or remain practice-only while building the full ascent work package.
