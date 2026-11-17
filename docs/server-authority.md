# Server-authoritative Astro-Elo (WP 1.9)

The authority, not browser storage, issues ranked receipts and updates the
player and mission ratings. `odd_verify::verify_record` replays the signed
flight with the same deterministic physics used by the browser. Local
HTTP/SQLite and Cloudflare-compatible JavaScript/WASM/D1/R2 adapters exist and
are exercised by the local gate. **A successful emulator test or Wrangler
dry-run is not a production deployment or a proof of distributed behavior.**

## Architecture and API

- `crates/odd-worker`: transport-independent `AuthorityEngine`, `Store`
  port, canonical commitments, checkpoint checks, receipts and Elo; the
  `local` feature adds HTTP and SQLite.
- `crates/odd-authority-wasm`: a bare WASM boundary accepting a server-owned
  database snapshot and returning proposed writes from the same Rust engine.
- `crates/odd-worker-cloudflare`: a JavaScript Worker shell, D1 migrations and
  R2 payload storage. It does **not** use the earlier workers-rs adapter.
- `web/authority*.js`, `leaderboard.js` and `verify.js`: same-origin transport,
  callsign and ranking UI, receipt validation and public replay.

| Method and path | Purpose |
|---|---|
| `POST /api/v1/callsign` | Signed, case-insensitive first-come callsign claim. |
| `POST /api/v1/commit` | Commit the design, maneuver plan and prediction; issue a signed attempt ticket. |
| `POST /api/v1/submit` | Replay the signed FDR and return a receipt. |
| `POST /api/v1/submit-segment` | Full replay **plus** supported checkpoint-chain checks, not incremental execution. |
| `GET /api/v1/leaderboard?contract=101&page=1` | Paginated ranked results; omit `contract` for the global board. |
| `GET /api/v1/verify/{fdr_hash}` | Public FDR and receipt lookup. |
| `GET /api/v1/player/{pubkey}` | Authoritative player rating, ranked count and callsign. |

The honest browser locks inputs and gets a valid ticket **before** launching
its modeled flight. A failed commit launches no flight. A first commit for a
`(public key, contract)` is Ranked; later commits are Practice, even after
refresh. A lost commit response can still have consumed the attempt. A retry
of an already submitted, identical ticket and payload returns the original
receipt without a second rating update. Local practice makes no commit and
has no server rating; later server-committed attempts can receive a verified
Practice receipt, but never move Elo or enter ranked boards.

## Commitments and their limits

The pilot signs the contract, key and two hashes under
`odderon-client-commit-v1`. The `odderon-design-hash-v2` hash binds the stage
masses, engine, thermal hardware, delegation mask and **ordered maneuver
commands** including epochs, delta-v and optional guidance. The prediction is
bound separately by `odderon-prediction-hash-v1`. At submission the authority
recomputes both and checks its signed ticket, the stored commitment, pilot
signature and replay. Claimed outcomes are not known at commit and are checked
by replay. A valid pilot signature alone is not evidence that a design,
thermal model or reported result is valid.

**Upgrade boundary:** v2 of the design hash adds the maneuver plan. Tickets
created with the former `odderon-design-hash-v1` hash cannot authorize a
submission or an idempotent submit retry under the new rule; outstanding
attempts may therefore be consumed. This is a commitment-protocol change,
not an FDR schema or Elo change. Previously issued receipts and FDRs retain
their signed bytes and can still be checked with the appropriate trusted key.
Coordinate client/server deployment and resolve outstanding v1 tickets before
an upgrade; silently accepting v1 would retain the plan-substitution hole.

The server can prove that a submitted prediction and plan match a ticket; it
**cannot prove the pilot had never simulated them before requesting it**.
The browser suppresses unlocked modeled-flight previews and makes numerical
tug targeting practice-only, with a best-effort guard for the current page
session. A reload, modified client, separate simulator or new key can bypass
that guard; the first-attempt rule is per key, not per person. These are
security limits, not a claim of hindsight-proof or Sybil-proof play.

## Replay, thermal validation and Elo

New authority submissions require the **current FDR schema, 4**, in both
modes. Schemas 1 and 2 remain independently readable and replayable offline
but lack the thermal/delegation proof and cannot receive new server receipts.
Schema 3 also cannot be rated as a new flight: it carries no coupled
propellant state, so the authority refuses it rather than certify a model it
no longer flies. The rule is pinned by
`crates/odd-worker/tests/schema_acceptance.rs`. For any accepted record a
storable stage must have no cryogenic block; a cryogenic stage must have
exactly one stage-0 block with matching propellant and bit-exact derived tank
volumes. The verifier checks the tank model and declared bus inputs, fixes the
structure/sink to the contract baseline and refuses supercritical relief
isotherms, recomputes vehicle mass and delegated hardware, and fails closed if
thermal integration fails. This verification-rule change can invalidate old
custom v3 records even when their existing receipt signature is valid; see the
[schema changelog](fdr-schema-changelog.md). The signed metrics, final hash,
vent events, per-tank loss and cooler energy must reproduce. The authoritative
score is replay `report[4]`, never a client-supplied rating.

Player Elo starts at 1200; the player K factor is 40 for the first 20 ranked
flights, then 20. Mission K is 8. Both changes are committed atomically for a
ranked submission; Practice does not change either. The formulas and bit-level
parity tests live in `crates/odd-contracts/src/elo.rs` and
`crates/odd-contracts/tests/elo_parity.rs`.

The physics remains a simplified model. The v3 tank sizing has an unresolved
ullage/envelope inconsistency; see [thermal.md](thermal.md). In addition,
thermal loss is integrated and signed **after** the orbit propagation: vented
mass is not subtracted from orbital mass, burn fuel or the score. The score's fuel-failure
threshold and prediction term also use the catalog baseline dry mass rather
than the fitted stage's dry mass. The debrief's displayed remaining propellant
uses the fitted dry mass, so these can differ, and a lighter fitted stage can
score a fuel failure even with fuel remaining. These behaviors preserve the
frozen replay and score pins; see [thermal.md](thermal.md). A receipt certifies this model's
replay, not real-world mission feasibility.

## Receipts, trust and dossiers

The Ed25519 receipt domain is `odderon-server-receipt-v3`; its canonical
big-endian payload binds schema, FDR hash, pilot, contract, attempt, mode,
score, player Elo before/after, mission Elo after and timestamp. Floating
values are represented by their IEEE-754 bits. Client-recorded Elo in the FDR
is not an authoritative rating.

`web/authority-config.js` contains the **static** `SERVER_PUBLIC_KEY` pin and
is empty by default, so an unconfigured deployment only offers local practice.
Never learn the trust anchor from the receipt, an API response or localStorage.
The browser validates the signature and its FDR/pilot/contract/mode binding
before showing `CERTIFIED SERVER-RANKED RECEIPT`; a valid Practice receipt is
labelled separately. Local edits to cached Elo cannot manufacture a valid
receipt. The browser guards in-flight player snapshots and delayed receipts
against newer changes in the same page, retains the original revision on retry,
and refreshes the player after a successful receipt. A cached receipt is only
a historical rating snapshot: its timestamp has second resolution and may
reflect request start rather than commit order. The player GET has no signed
global revision, so offline views and other tabs/devices are not guaranteed to
reflect the newest transaction; sync against the authority for a fresh value.
Existing receipts will not validate under a different pin; key rotation/old-key
trust and migration need an explicit operational plan and are not implemented
automatically.

A verified dossier carries an independently checkable receipt in Annex D.
Its ranked QR resolves to the deployment's `mission.html#flight=<fdr_hash>`,
which fetches the public record and receipt for local replay. A local or
Practice dossier uses a self-contained `#verify=` fragment. When a deployment
origin is known the QR is an absolute URL; otherwise it is explicitly labelled
relative to that origin. A public lookup requires the corresponding authority
to remain available.

## Checkpoint integrity is not incremental verification

The optional checkpoint chain is checked **after one complete authoritative
replay**. For storable stages, every link and strictly increasing epoch, sampled
state, propellant mass and zero tank-temperature sentinel must match, and the
terminal checkpoint must bind to the same final-state hash. Recomputed client
hash links alone confer no trust. Cryogenic checkpoint temperatures are not
sampled by this replay and `/submit-segment` rejects cryogenic records with
`ThermalSegmentsUnsupported`; complete cryogenic submissions still work.

There is no resumed-segment executor, per-segment API, CPU-budget reduction,
free-tier timing guarantee or `?seg=i` implementation. The test named
`oracle_chunked_segment_verification_parity` proves integrity and receipt
parity of this full-replay path only.

The v4 helpers for the *next* step exist and are tested, but nothing in
production calls them: `verify_resumable_segment`, `validate_resumable` and
`SegmentRateLimiter` in `odd-worker::segment`. `oracle_true_single_segment_resumption_cost`
proves the coupled propagator reproduces a single-pass flight from a checkpoint
and charges only the requested window. Shipping that as a protocol is blocked
on a security question rather than a numerical one: a verifier standing at a
segment boundary can only audit the transition it starts from, so resuming from
a client-supplied checkpoint would trust that client for every boundary but the
launch. Authority-issued links or chained proofs have to come first.

## Transactions and deployment boundary

SQLite uses `BEGIN IMMEDIATE` for the entire read/compute/write operation.
Attempt counters are bounded to the protocol's `u32` range; an existing zero,
malformed or exhausted counter fails rather than issuing a new first attempt.
SQLite rejects invalid persisted ratings, malformed commitment/flight index
fields and unreadable receipt rows rather than coercing or truncating them.
The engine checks receipt signatures and bindings on retries. When the public
flight still indexes that attempt, its receipt must equal the idempotency copy;
public lookup also requires those copies to match. This catches missing or
divergent rows without preventing a later Practice attempt from reusing the
same signed flight. Storage errors return HTTP 503 rather than being reported
as a client error; existing damaged data needs repair from trusted backups.

A D1 batch alone does not protect earlier reads. The Cloudflare shell reads a
primary-session snapshot and a global revision, invokes Rust, and publishes
proposed D1 writes in a batch guarded by a compare-and-increment of that
revision. A stale snapshot is retried up to 16 times; missing or invalid
revision and storage errors fail closed. All writers must obey this guard.
These checks do not reconstruct a lost idempotency row if the old flight is no
longer indexed or a different record is submitted; durable storage and
recoverable backups remain prerequisites.

R2 writes precede the guarded D1 batch and use an FDR-hash/payload-SHA-256
key. A failed or stale transaction can leave an unreferenced object; it must
not publish a receipt or Elo update. New-layout reads verify the digest and
size; migrated rows with a null R2 key use the old hash-only lookup. Production
operators need safe orphan cleanup, backups, access controls, abuse/rate
limits and a deployment/rollback plan. Do not delete objects referenced by D1
or assume old racy data was repaired by the migration.

Browser requests are relative `/api/v1/...` URLs: static hosting must route
those paths to the chosen authority on the **same origin**. A bare static
server is practice-only. See the [Cloudflare adapter README](../crates/odd-worker-cloudflare/README.md)
for prerequisites; no live Cloudflare deployment is implied by this repository.

## Local acceptance

`./scripts/gate.sh` builds native and WASM authorities and runs the six named
oracles, workspace tests, browser flight/QR/receipt tests, native HTTP/SQLite,
Miniflare/workerd D1/R2 flows, race/fault/migration tests and native/Node/Chrome
parity. The browser tests use a public disposable seed, not a production key.
The checkpoint, plan-substitution and signed cryogenic forgery regressions test
the boundaries above. Emulator concurrency coverage is useful evidence, not
an assertion about a deployed Cloudflare account or all possible failures.
