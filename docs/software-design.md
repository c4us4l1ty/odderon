# Software design and current boundaries

Odderon is an experimental space-engineering game, not an operational
flight-dynamics product. This document describes the current implementation;
[blueprint.md](../blueprint.md) is the original proposal, not a deployment
claim. Governing principle: **Odderon's ambition is civilization-scale; its
execution must be bottleneck-scale.**

## Architecture

| Layer | Responsibility |
|---|---|
| `odd-core`, `odd-ephem`, `odd-astro` | SI quantities, frames, packed DE440s, orbital elements, perturbations, shared DP45 integration and attribution. |
| `odd-propulsion`, `odd-systems` | Physical liquid-engine solve; cryogenic tank, cooler and bus models. |
| `odd-sim`, `odd-contracts` | Maneuver plans, locked predictions, three catalog contracts, scoring, thermal designs and chief substitutions. |
| `odd-identity`, `odd-report` | Ed25519 licenses, canonical signed FDR, vector dossier and QR. |
| `odd-verify` | Portable `compute` and replay of signed flight records. |
| `odd-view`, `web/` | WASM scalar boundary, WebGL/Canvas UI, one-shot flight workers and browser identity. |
| `odd-worker` | Transport-independent authority, canonical Astro-Elo, commitments, receipts, and optional local HTTP/SQLite adapter. |
| `odd-authority-wasm`, `odd-worker-cloudflare` | Bare WASM authority boundary and a JavaScript/D1/R2 Worker shell. |

The orbit and tank ODEs share `odd-astro::dp45`; the independent orbital and
thermal oracle fixtures guard the numerical models. The native verifier,
browser WASM and edge authority invoke the same replay code. The gate compares
native, Node and Chrome fingerprints, and a browser-signed record is also
replayed by the native CLI.

## Flight, delegation and the frozen record

The engine lab separates editable preview inputs from the physical design
fitted to the spacecraft. Thrust, Isp and engine mass are recalculated from the
signed design; any derived fields in the record are claims checked by the
verifier, not trusted inputs. See [propulsion.md](propulsion.md).

Four deterministic department chiefs substitute textbook Propulsion, Thermal,
Power or GNC hardware. Each delegated discipline adds its own +35% hardware
mass and +30% relative cost margin. The actual flown mass includes the
fit-out and delegation penalties, and the signed four-bit mask caps the
score's efficiency component at 0.65 whenever any department is delegated.
Chief-owned controls are locked and the substitution is independently
recomputed at replay. See `oracle_chief_hardware_substitution`.

Cryogenic tanks integrate heat leak, venting and cooling along the orbital
spline with instantaneous two-tank bus priority. The optional, paid 103 card
is available directly in the assembly UI. The multi-day solvability oracle
flies 102 and 103, asserts the catalog objective in manual and fully
delegated modes, and checks thermal losses and an idealized post-loss delta-v
margin. The practice splitter targets the assembled stage, not the catalog
baseline. See [thermal.md](thermal.md) for the model, measured examples and
important one-way coupling limitation.

FDR schema 3 and its 58 report fields remain frozen. It signs the physical
engine and thermal specifications, delegation mask, exact vent history,
per-tank loss and cooler energy. A cryogenic v3 replay requires the matching
stage-0 tank block and fails closed on invalid models; a storable stage has no
block. Old schema 1/2 records remain independently replayable, but new server
submissions require v3. See [fdr-schema-changelog.md](fdr-schema-changelog.md).

## Server-authoritative Astro-Elo

Ranked launch commits a vehicle, ordered maneuver plan and prediction before
the honest browser launches a modeled flight. The authority checks the
pilot's signature, ticket, commitment, canonical contract and complete replay,
then atomically issues an Ed25519 receipt and applies the canonical Elo update
for the first committed attempt per key and contract. Later committed
attempts are Practice; local practice has no server receipt or rating. Callsign
registration, paginated boards, public replay, profile sync and receipt-bearing
dossiers are implemented. See [server-authority.md](server-authority.md).

The browser checks receipts against the **static** public-key pin in
`web/authority-config.js`, empty by default. A ranked dossier's QR is a public
`#flight=` lookup; a local/Practice QR uses a self-contained `#verify=`
fragment. Receipt trust is separate from verifying the pilot's FDR signature.
A deployment must configure a private seed, pin its corresponding public key,
and route `/api/v1/` to the authority on the same origin.

SQLite transactions serialize the complete operation. The Cloudflare shell
uses a revision-guarded D1 batch with retries, and R2 writes can leave safe
orphans. Tests cover workerd races and injected failures, but neither an
emulator nor a Wrangler dry-run establishes real distributed deployment
correctness. No production deployment is implied.

## Known limitations

- **Not hindsight-proof:** the ticket binds design, plan and prediction, but
  cannot establish whether an untrusted client, another simulator or another
  key ran the model before committing. The practice guard is page-session UX,
  not an identity or security boundary.
- **Not incremental:** checkpoint submission replays the full flight first,
  then checks storable checkpoint metadata against it. Cryogenic checkpoints
  are rejected; there is no per-segment CPU budget or free-tier guarantee.
- **Thermal coupling is one-way:** boil-off is integrated and reported but
  not deducted from orbital mass or the burn fuel budget. Electric-propulsion
  surplus is calculated but not applied as thrust in the chemical contracts.
  The score's fuel-failure threshold and fuel prediction use catalog baseline
  dry mass, which can differ from the fitted stage and the debrief's displayed
  remaining propellant. A lighter fitted stage can score a fuel failure even
  with fuel remaining. Changing these frozen replay behaviors requires a
  versioned migration.
- **Identity:** the license is browser-local with export/restore, not an
  account, revocation, Sybil prevention or custody system. A private server
  seed, static key pin and explicit rotation strategy are operator duties.
- **Scope:** quasi-1D perfect-gas propulsion and a lumped thermal model are
  approximations. There is no launch-window optimizer, B-plane targeting,
  lunar orbit insertion, terrain/landing or space-weather atmosphere model.
  Numerical three-kick targeting is a bounded practice aid, not an optimizer.

## Acceptance

`./scripts/gate.sh` runs formatting, clippy, workspace and local-authority
Rust tests, native/WASM parity, Node tests, observatory/idle and assembly
browser tests, both authority browser runtimes, and D1/R2 race/fault/migration
tests. The six requested chief, solvability, commitment, first-attempt,
checkpoint-parity and localStorage-tamper oracles run within that sweep.
The sandbox toolchain lacks `rustdoc`, so the local script uses `--lib --tests`;
CI also runs the normal workspace tests and doctests with a complete toolchain.
