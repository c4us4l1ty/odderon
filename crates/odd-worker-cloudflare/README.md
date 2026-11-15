# odd-worker-cloudflare

The Cloudflare Workers deployment adapter for the [`odd-worker`](../odd-worker)
authority (WP 1.9). It is a thin shell: D1 implements the storage port and R2
holds the signed FDR payloads by content hash. All anti-cheat logic lives in
the shared, transport-agnostic `AuthorityEngine`, which is exercised locally
by `./scripts/gate.sh` with **no cloud credentials**.

## Why a sibling crate

`workers-rs` and its wasm dependency tree (`wasm-bindgen`, `web-sys`,
`js-sys`, `serde_json`, …) are not vendored for the offline sandbox gate.
Keeping this adapter in a crate that the main workspace `exclude`s lets the
physics + authority build and test fully offline, while this crate compiles
in CI (or on a maintainer's machine) with network access. The engine, the
receipt format, the commitment hashes and the Elo math are bit-identical to
the local adapter — only the storage and transport shells differ.

## Build & deploy

```sh
# From the repository root, with network access:
wrangler deploy --config crates/odd-worker-cloudflare/wrangler.toml

# Or build the wasm directly:
cargo build -p odd-worker-cloudflare --target wasm32-unknown-unknown --release
```

Before the first deploy:

1. Create the D1 database and R2 bucket, and put their ids/names in
   `wrangler.toml`.
2. Apply the migration: `wrangler d1 migrations apply odderon`.
3. Set the server signing seed: `wrangler secret put SERVER_SEED`
   (64 hex characters = 32 bytes). The matching public key is what clients
   embed as `server_pk`; rotate the seed to rotate the key.

## Endpoints

| Method | Path | Purpose |
|---|---|---|
| POST | `/api/v1/commit` | Commit design + prediction, take the next attempt number, return a signed ticket. Only attempt 1 is Ranked. |
| POST | `/api/v1/submit` | Replay the server-canonical contract, update Elo (ranked first attempt only), store the FDR, return the receipt. |
| POST | `/api/v1/submit-segment` | As submit, with a chunked checkpoint chain for the free-tier CPU budget. |
| GET | `/api/v1/verify/{fdr_hash}` | Fetch the stored FDR payload (the public QR target). |

Callsign registration and the leaderboard are added with the Slice 3 UI.

## Storage representation

The local adapter stores keys and hashes as SQLite `BLOB`; D1 stores them as
hex `TEXT` because D1 binds strings most reliably. Both implement the same
`Store` port, so the engine cannot tell them apart. The D1 migration is in
`migrations/0001_init.sql`.
