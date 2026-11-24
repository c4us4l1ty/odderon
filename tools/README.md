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

### The static server, and a real bug it had

`tools/serve.mjs` is the preview server; `tools/check-shell.mjs` and `tools/check-cache.mjs` each start
one too, so their browser runs are self-contained. That is **three** static servers, and on 2026-11-22 a
bug hunt found that all three handled paths differently, and **two leaked files**:

| file | behaviour before the fix |
| --- | --- |
| `serve.mjs` | `GET /dist/../.git/config` returned **the repository's own git config, status 200, 435 bytes**. The `/dist/` prefix was tested on the *raw* request path, before normalising, and the containment check that followed was a bare `startsWith`. |
| `check-cache.mjs` | `GET /../.git/config` returned the same file. It had **no containment check at all** — just `readFile(join(webRoot, req.url))`. |
| `check-shell.mjs` | Correct, but only by accident: it happened to use `new URL()` first, which collapsed the `..`. Nothing in that file said the URL parser was load-bearing for security. |

All three now call one resolver, `tools/static-path.mjs`, and `node tools/check-static-path.mjs` (also a
CI step, and `npm run check:paths`) asserts the property that actually matters — **containment**: every
resolved path lies inside `web/` or `dist/`, or the request is refused. It also greps every file that
imports `node:http` and fails if any of them hand-rolls `join(root, req.url)` again, because the bug was
a copy-paste problem rather than a typo.

Two notes on how the test is written, both learned the hard way while writing it:

- It asserts *containment*, not "returns null". A traversal that normalises to `web/.git/config` is
  **safe** — the file does not exist — so asserting null would fail on correct code and would push the
  resolver toward conflating "forbidden" with "absent".
- It strips comments before scanning source. The first version flagged the comments that *explain* the
  bug, and a checker that fails on prose is one people learn to ignore.

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

---

## 4. The licensing gate that runs here, not in Rust

`npm run check:classes` — and a hard job in `ci.yml` — reads `src/classes.json` and compares it to the
kernel tree. This is the one compliance check that does not need a compiler, because the question is
*"may this record be in the binary at all"*, and that is a property of the source, not of a data file.

`tools/check-classes.mjs` is the gate; `tools/check-classes-selftest.mjs` injects ten violations and
requires each to be caught, including the comment-only case that must **not** be caught. Run both, or
just `npm run check:classes`, which runs both.

The rendered table in `docs/CLASSES.md` is generated by `node tools/check-classes.mjs --table`. Edit
`src/classes.json`, then regenerate; CI fails if the document and the list disagree.

What it deliberately does not do is in `docs/CLASSES.md` §3, and the first item is the important one: it
cannot verify a licence. It enforces that every record *has* a classification, and that the
classification was made by a named person. The reading of the terms page is still a human act, recorded
with a URL and a date.

---

## 5. Seeing the view — including from a phone

Claim 5 ("the window view is the best-looking thing on your laptop") is a judgement, and the only way to
put a judgement in front of somebody is to put the **pixels** in front of them. There are two capture
paths because there are two very different places the picture can come from.

### 5a. From this sandbox (desktop, headless)

```sh
node tools/screenshot.mjs                      # -> .artifacts/shell.png, 1600x1000
node tools/screenshot.mjs --out .artifacts/big.png --width 2560 --height 1600
node tools/screenshot.mjs --url http://127.0.0.1:8080/ --hide-ui
```

It starts and stops its own server. It refuses to hand you a picture that is not settled: exit 1 on any
console error, exit 2 if frames never stop arriving, exit 3 if it drew while idle. **The renderer here is
SwiftShader**, so this is the right *picture* and the wrong *timing* — never quote a frame time from it.

### 5b. From the phone

The page captures **itself**, because the device whose picture matters has no debugging cable:

1. Open the page (`python -m http.server 8080 -d web` in Termux, then `http://127.0.0.1:8080`).
2. Set the quality you want to show: render scale, shadow taps, full/simple geometry.
3. Press **`capture this frame as a PNG`**. It measures first, then saves a PNG to Downloads.

The saved file is the **scene** with a provenance strip burned into the bottom edge:

```
odderon 2026-11-24T08:42:42.577Z
canvas 420 x 760 (0.32 Mpx)
scale 1.00 taps 4 full geometry
renderer ANGLE (Google, Vulkan 1.3.0 (SwiftShader Device ...
R2 GPU timer returned no samples R3 12.0 / 32
gpu timer no-samples
```

That strip is not decoration. A frame at 0.50x render scale is a quarter of the pixels of the same frame
at 1.00x, and the two look identical in a PNG — so a picture without its scale is a picture that cannot
be argued with. Hide the panel first (`hide`) if you want to judge the art rather than the instrument;
the strip is added either way.

Why it is captured in-page rather than with `page.screenshot()`: `preserveDrawingBuffer` is `false` (it
costs bandwidth, and this renderer is about bandwidth), so the drawing buffer is cleared as soon as the
frame is composited. A readback from outside a draw returns an empty buffer. The only honest way to
sample the image is from inside the frame that drew it — the same rule the pixel probe has always
followed.

### 5c. Reading a PNG that came back

```sh
node tools/capture-page.mjs ~/Downloads/odderon-1764000000000.png --stamp "scale 0.50 taps 4 simple"
```

Prints the real dimensions read out of the PNG header, the shaded megapixels at that scale, and echoes the
provenance stamp. `--json` for machine-readable output. It **does not** score the image: claim 5 is a
human verdict, and a tool that produced a number for it would be inventing a metric to stand in for a
judgement.

### Why no screenshot is ever committed

`ci/kernel-policy.sh` R5 forbids binary assets in `git ls-files`, and `GRAPHICS.md` R1 gives the reason:
every surface must be a function S(u,v,t), not a stored bitmap. A screenshot *is* a stored bitmap. The
rule has no documentation exception — an exception is how a rule stops being a rule — so captures go to
`.artifacts/`, which `.gitignore` covers. What gets committed is the **numbers and the stamp**, quoted as
text, which is why the strip is burned into the image in the first place.
