# Server-authoritative Astro-Elo (WP 1.9)

The rating and the certificate are decided by the server, not the browser.
The same physics kernel and verifier (`odd_verify::verify_record`) run in
both places, compiled to the same wasm, so a receipt certifies a replay
anyone can re-run. A DevTools-edited local rating with no valid receipt can
never read as certified.

## Hexagonal layout

```
crates/odd-worker/                 pure core + local adapter
  engine.rs      AuthorityEngine   commit / submit / submit-segment
  store.rs       Store port        dumb CRUD; MemoryStore reference impl
  receipt.rs     ServerReceipt     Ed25519 over the canonical payload
  commitment.rs  design/prediction hashes (anti-hindsight)
  segment.rs     chunked hash-chain verification
  protocol.rs    transport-agnostic wire types + ServerError
  sqlite.rs      local durable adapter (system libsqlite3 FFI, `local`)
  http.rs        local std::net HTTP/1.1 server (`local`)
crates/odd-worker-cloudflare/      wasm-only deployment adapter (CI-built)
  src/lib.rs     D1 implements the port; R2 holds FDR blobs by hash
```

The engine talks only to the `Store` port and never reads the wall clock —
the adapter passes `now_s` in, so a replay is reproducible and a test can pin
it. An adapter gives the engine exclusive access for one operation (a mutex
locally, a D1 batch in the cloud) so the read–compute–write of a ranked
result is atomic.

## The commit → submit flow

1. **`POST /api/v1/commit`** `{contract_id, pubkey, design_hash,
   prediction_hash, client_sig}`. The server verifies the pilot's Ed25519
   signature over the canonical commitment, atomically increments
   `attempts(pubkey, contract_id)`, stores the commitment and returns a
   signed `CommitTicket`. **Only `attempt_num == 1` is Ranked**; every later
   commit — including commit→refresh→commit — is forced to Practice.
2. **`POST /api/v1/submit`** (or `/submit-segment`). The server re-checks its
   own signature on the ticket, decodes the FDR, and:
   - refuses a design or prediction that does not hash to the commitment
     (`DesignCommitmentMismatch`, `PredictionCommitmentMismatch`);
   - ignores the client's contract parameters and resolves the
     **server-canonical** `Contract`, checking the record's framing against
     it (`certify_contract`);
   - replays the physics with `verify_record` — engine, metrics, final
     state, vents and boil-off must all reproduce;
   - if segments are present, verifies the checkpoint chain against the
     authoritative trajectory, binding to the same `final_state_hash`;
   - takes `score_s` from the replay (`report[4]`), never a client claim;
   - updates player **and** mission Elo atomically, but only for a ranked
     first attempt; practice is stored and replayable, never rated;
   - stores the FDR by content hash and issues the receipt. Re-submitting
     the same attempt is idempotent: same receipt, no second Elo move.

## The receipt

`ServerReceipt` is the server's Ed25519 signature over the canonical payload

```
RECEIPT_DOMAIN ‖ schema_v3 ‖ fdr_hash ‖ pubkey ‖ contract_id ‖ attempt_num ‖
mode ‖ score_s_bits ‖ elo_before_bits ‖ elo_after_bits ‖
mission_elo_after_bits ‖ timestamp_s
```

(all big-endian; floats as IEEE-754 bits). The browser embeds the server
public key (`server_pk`) and re-checks the signature. The certification
banner is a pure function of the receipt, never of the stored rating:

| Condition | Banner |
|---|---|
| valid signature **and** Ranked | `★ CERTIFIED SERVER-RANKED RECEIPT` |
| valid signature, Practice | `SERVER-VERIFIED PRACTICE` |
| no receipt / invalid signature | `UNVERIFIED LOCAL / PRACTICE` |

Editing `elo_after` to 2999 in localStorage breaks the signature, so the
label falls back to UNVERIFIED; on sync the authority's stored rating is
restored.

## Canonical Elo

`odd_contracts::elo` is the single authoritative home of the rating math
(blueprint §6.6): `E = 1/(1+10^((R_m−R_p)/400))`, `K_p = 40` for the first 20
ranked missions then `20`, `K_m = 8`, start 1200. `tests/elo_parity.rs` pins
it bit-for-bit to the client's local copy (`odd_app::debrief::elo_update`)
so practice feedback cannot lie about what the ranked update will do.

## Chunked verification

A long mission carries a `SegmentCheckpoint{t_us, state_vector, prop_kg,
tank_temps, cum_hash}` every ~30 sim-days. Each link chains a cumulative
hash over every prior checkpoint, and each checkpoint must match the
authoritative trajectory sampled at its own `t_us` bit-for-bit. A one-bit
edit — even with the downstream hashes recomputed — is caught at its own
segment boundary by the physics sample. The terminal checkpoint reproduces
the full replay's `final_state_hash`, so chunked and whole submissions agree
on hash, verdict and receipt. The cloud adapter can replay each segment
inside its per-request CPU budget; the chain guarantees the pieces compose.

## Adapters and the offline gate

- **Local HTTP/SQLite** (`odd-worker`, `local` feature): a dependency-free
  `std::net` server over a thin FFI binding to the system `libsqlite3`. No
  vendored database crate. `./scripts/gate.sh` runs the anti-cheat oracles
  through it with **no cloud credentials**.
- **Cloudflare D1/R2** (`odd-worker-cloudflare`, excluded from the offline
  workspace): D1 implements the port, R2 holds the FDR payloads. Needs
  `workers-rs`, so it compiles in CI with network. It runs the *identical*
  engine over a D1 snapshot, then flushes; it adds no logic that could cheat.

## Oracle gates (all in `./scripts/gate.sh`)

| Gate | What it proves |
|---|---|
| `oracle_server_anti_hindsight_commitment` | a re-signed prediction B fails `PredictionCommitmentMismatch` |
| `oracle_server_first_attempt_only_ranked` | attempt 2 is forced Practice and never moves Elo |
| `oracle_chunked_segment_verification_parity` | identical hash/verdict/receipt; a 1-bit intermediate tamper fails at that boundary |
| `oracle_devtools_localstorage_tamper_defeated` | a stored 2999 with no receipt reads UNVERIFIED; the true rating is restored on sync |

The browser layer of the DevTools gate, the callsign registry, the
leaderboard and the public verify drawer arrive with Slice 3.
