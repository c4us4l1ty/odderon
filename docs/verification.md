# Browser distribution verification — 2026-11-19

**Result: passed locally for the v3 browser distribution.** This checked the working tree, not a published release or a hosted Actions run.

| Layer | Result | Evidence |
| --- | --- | --- |
| Shared Rust game workspace | 390 tests passed, 1 fixture-writing helper ignored | Release-mode library and integration tests, including the `odd-view/cockpit` feature; no authority service started. Rustfmt and cockpit Clippy (`-D warnings`) passed. |
| Emitted WASM | Exact match | A release build in an empty Cargo target directory produced the same bytes as `runtime/odd_view.wasm` **and** `dist/stations/odd_view.wasm`. SHA-256: `466b608dfe45ed152eaec7e64206d7bda330af45c24cb8b42fd391d69b5834a9`. This is a same-toolchain comparison, not a guarantee about other compiler versions. |
| Web distribution | Clean install and build passed | Used `npm ci`, regenerated `public/` and `dist/`, then served the production output, not the Vite development module graph. |
| Station JS and flight ABI | 26 inherited JS tests passed; WASM acceptance passed | All three contracts advanced under pilot commands, signed and replayed; modified results were rejected. A test pilot reached the 101 objective using throttle and coast commands, and a changed engine produced a different measured burn. |
| Browser end-to-end | 7 Chromium scenarios passed | Tests below used the shipped files, checked for page errors and HTTP error responses, and rejected external HTTP(S) or `/api/` requests. |

## Player journeys exercised

1. Record a prediction, fly and pause; export and verify the manual tape, reject a changed tape, file it, reload the page and reverify the saved tape.
2. Open contract 103 in engineering, visit vehicle, Academy and license, load the perigee-kick example, transfer the fitted stage, view the orbit map and obtain a numerical Earth–Moon range from the observatory.
3. Execute a planned experiment, export its signed FDR, import and replay it locally, and download the verified HTML dossier with attribution, engine table and QR verification payload.
4. Change the throat and exit dimensions for 102, apply delegation, transfer the stage, burn, export and replay. The exported flown design contained the changed throat and delegation mask; measured burn Δv was positive.
5. On a 390 × 844 viewport, use the on-screen attitude control to change measured pitch and open flight review. This was browser emulation, not a physical phone test.

6. Hold a steering command while requesting warp twice: the pilot retains control at 1× and can warp after releasing the control.
7. Delay loading the signer, cancel filing and resume: the flight continues and the abandoned request does not save a tape.

## Reproduce

From `odderon-v1/`, with a Rust toolchain and `wasm32-unknown-unknown` installed:

```sh
cargo fmt --all -- --check
cargo clippy --locked -p odd-view --features cockpit --all-targets -- -D warnings
cargo test --locked --release --workspace --exclude odd-worker --exclude odd-authority-wasm --features odd-view/cockpit --lib --tests
cargo build --locked --release -p odd-view --features cockpit --target wasm32-unknown-unknown
npm test
```

From the repository root:

```sh
npm ci
npm test
npm run build
npx playwright install chromium
npm run test:browser
```

## Not certified by this check

- **Desktop:** the optional Electron shell passed JS syntax checking and its locked npm dependencies installed. The executable download failed in this sandbox; there was no desktop window to test. No desktop platform or installer is marked verified.
- **Scope:** the end-to-end suite samples player journeys. It does not demonstrate a manually won 102 or 103, completion of every lesson through the v3 UI, every fit-out, Safari/Firefox, or a Steam release. Numerical and lesson coverage in the inherited Rust tests is separate from UI coverage.
- Cloudflare, ranked Astro-Elo and online leaderboards are intentionally outside the v3 product, not untested local substitutes.
