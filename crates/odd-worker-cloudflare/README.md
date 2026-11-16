# Cloudflare authority adapter

This directory contains a JavaScript Cloudflare Worker shell around the
workspace's `odd-authority-wasm` and shared Rust `AuthorityEngine`. D1 stores
attempts, commitments, ratings, callsigns, flight metadata and receipts; R2
stores FDR payloads. See [server-authority.md](../../docs/server-authority.md)
for the trust boundary, routes, checkpoint and hindsight limitations.

The repository has exercised this shell in Miniflare/workerd, including D1/R2
races, migrations and injected failures. A Wrangler **dry-run** has built the
bundle. Neither is a deployed, distributed production validation; no live
Cloudflare deployment is implied.

## Before deployment

1. Install a supported Node 22, Rust toolchain and `wasm32-unknown-unknown`
   target, plus Wrangler. The Wrangler build command compiles
   `odd-authority-wasm`, not `odd-worker-cloudflare` as a Rust crate.
2. Create a D1 database and R2 bucket in the intended Cloudflare account.
   Replace `REPLACE_WITH_YOUR_D1_ID` and set the correct bucket name in
   `wrangler.toml`. Apply **both** migrations to the intended remote D1
   database. Back up any existing database and understand that migration 0002
   does not repair earlier concurrent-write races.
3. Generate a unique, private 32-byte Ed25519 seed using a secure source and
   install it out of band as the `SERVER_SEED` secret (64 hex characters).
   The public test seed (`5e` repeated 32 times) is public and **must never**
   be used in production. Do not put the seed in source, browser assets,
   URLs, logs or shell history.
4. Derive the corresponding public key in a trusted provisioning environment
   (the local authority binary prints it on startup) and pin its 64 hex
   characters in `web/authority-config.js` before building/deploying the
   static site. The default empty pin disables ranked certification. A key
   change invalidates old tickets and requires an explicit old-receipt trust
   and data-migration strategy; automatic rotation is not implemented.
5. Serve the static site and route `/api/v1/*` to the Worker **under the same
   origin**. The browser never contacts a sandbox's localhost directly. Plan
   D1/R2 backup, rate limiting, quota/CPU monitoring, rollback and safe cleanup
   of unreferenced R2 objects before allowing public traffic.

For an operator who has already configured Wrangler credentials and the
resources above, examples from the repository root are:

```sh
# Builds/bundles only. No deployment or remote migration occurs.
npx wrangler@4.162.0 deploy --dry-run --config crates/odd-worker-cloudflare/wrangler.toml

# Real operations: run only against the intended account and resources.
npx wrangler@4.162.0 d1 migrations apply odderon --remote --config crates/odd-worker-cloudflare/wrangler.toml
npx wrangler@4.162.0 secret put SERVER_SEED --config crates/odd-worker-cloudflare/wrangler.toml
npx wrangler@4.162.0 deploy --config crates/odd-worker-cloudflare/wrangler.toml
```

The secret command prompts on the operator's machine. Do not paste a seed into
chat or commit one. A local authority can be started with `ODDERON_SERVER_SEED`
provided by a secure process environment, `ODDERON_DB` set to a private SQLite
path and `ODDERON_BIND` set to its server-side listener. It also needs a
same-origin reverse proxy serving `web/` and forwarding `/api/v1/`.

## Consistency and storage

The Worker obtains a primary-session snapshot and computes a proposal in WASM.
A guarded D1 batch checks/increments one global revision before publishing all
writes; stale proposals are recomputed, up to 16 attempts. Every writer must
participate. R2 writes happen before the D1 batch and can leave orphans after
contention or failure. New keys include both canonical FDR hash and payload
SHA-256; reads bound the size and validate that digest. Legacy rows with a
null R2 key use the old hash-only layout. A collector must compare against D1
references and allow for in-flight writes; no collector is supplied here.

`POST /api/v1/submit-segment` currently performs a **full replay**, then checks
storable checkpoint chains. Cryogenic checkpoints are rejected; full
cryogenic submissions are supported. It is not a free-tier CPU solution or
incremental executor.

## Local tests

From the repository root, `./scripts/gate.sh` builds both authorities and runs
native and workerd browser suites and `npm run test:cloud-races`. Tests use
disposable databases/buckets and a public seed. `npm ci` and the sandbox
provisioning script install their dependencies; no Cloudflare credentials are
needed for these tests.
