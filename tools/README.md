# tools — the build loop, given no local Rust

This sandbox has Node and a Chromium but **no Rust toolchain and no route to `crates.io`**. So Rust is
compiled by **GitHub Actions**, and this directory holds the two scripts that close the loop: push to
get a build, pull the artifact back to *run* it.

The division of labour is deliberate and worth stating, because it is the architectural point:

| Step | Where | Why there |
| --- | --- | --- |
| Compile Rust → wasm | **GitHub Actions** (`ci.yml`) | The only place a toolchain exists |
| Run wasm, compare hashes | **This sandbox** (Node) | Node runs wasm natively; no browser needed |
| Render, benchmark, visual check | **Your machine** (Reference A/B) | SwiftShader here cannot time a frame |

That third row is the whole reason `docs/BUILD-PLAN.md` says *you* are the performance measurement. This
sandbox can prove a shader is **correct**; only your laptop can prove it is **fast**.

---

## 1. Bootstrap (once per clone)

```sh
npm install                 # puppeteer-core + @sparticuz/chromium, pinned in package.json
npm run fetch:wasm          # pull the latest `wasm` artifact into ./dist
```

`fetch:wasm` uses `gh run download`, so it needs the `gh` CLI authenticated (it is, in this sandbox).

---

## 2. Cross-platform determinism check

The claim that matters: **the same transcendentals produce the same bits natively and in wasm.** This
script is the proof, and it needs no browser because Node runs the wasm module directly.

```sh
npm run determinism
```

It loads `dist/odderon_dmath.wasm`, drives the exported `golden()` over the probe set, and compares the
result against `crates/odderon-dmath/tests/golden.txt` — the same file the native test checks. Two
platforms, one number, or a failure with the first divergent input named.

**This is the highest-value test in the project.** If it passes, determinism is real and Esse is
achievable. If it fails, everything downstream of a trajectory hash is unverifiable, and we find out now
rather than after building a rating system on sand.

---

## 3. Rendering harness

Not needed until there is something to render, but the capability is verified working, so here is what was
actually tested on 2026-11-22 rather than assumed:

- `@sparticuz/chromium` ships a compressed headless Chromium; unpack it with `chromium.executablePath()`.
- Its `.so` dependencies come from the same package as a Brotli'd tar; extract them and point
  `LD_LIBRARY_PATH` at them, or the binary exits 127 on `libnspr4.so`.
- WebGL2 **works**: `ANGLE (Google, Vulkan 1.3.0, SwiftShader Device)` — a *software* rasteriser via
  `--use-gl=angle --use-angle=swiftshader --enable-unsafe-swiftshader`.

So this sandbox can take screenshots and verify visual correctness. It must never be used to claim a
frame time. `npm run gl:info` prints the renderer string so a screenshot never gets mistaken for a
benchmark.
