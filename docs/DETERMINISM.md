# Odderon — Determinism, and the CI-as-compiler loop

**Status: binding.** Two subjects, because they are the same subject: how we *prove* that a simulation
can be re-run bit-for-bit, and how we compile Rust when the development environment has no Rust.

Companion docs: [`VISION.md`](VISION.md) §5.2, [`CREDIBILITY.md`](CREDIBILITY.md) §3.4 and §5,
[`BUILD-PLAN.md`](BUILD-PLAN.md).

---

## Part 1 — Determinism

### 1.1 Why it is not optional

Esse is a claim that a stranger can verify (`CREDIBILITY.md` §5). Proof-of-Flight is a `<100 KB` input
log that any machine re-simulates to the same state hash. If a single `sin` differs in its last bit
between Firefox and Chrome, then after a thousand integration steps the trajectories have different
phases, the hashes differ, and the credential is a number a server asked you to trust — which is exactly
the thing the project refuses to be.

So determinism is not a quality attribute to add later. It is the load-bearing wall of the only feature
that makes the game more than a game.

### 1.2 The hazard, stated precisely

WASM fixes the semantics of:

```
+  −  ×  ÷   sqrt   floor/ceil/trunc/round   abs/copysign   min/max   to_bits/from_bits
```

WASM does **not** fix the semantics of:

```
sin  cos  tan  asin  acos  atan  atan2  exp  exp2  ln  log2  log10  pow  cbrt  hypot  sinh … tanh
```

Those are imported from the host's libm. They are *not* required to be correctly rounded, and they
differ between engines and platforms in the last ulp. Every other rule in this document exists to deal
with that paragraph.

### 1.3 The six rules

| # | Rule | Enforced by |
| --- | --- | --- |
| 1 | Transcendentals come from `odderon-dmath`, never from the host. | `kernel-policy` job: `grep` for `.sin(`/`.exp(`/`.ln(`/`.powf(`/`.atan2(` under `crates/` outside `odderon-dmath/src/lib.rs` → build fails. |
| 2 | Time is an argument (`t: i64`, µs since J2000 TDB), never a query. | `kernel-policy`: `Instant::now`/`SystemTime`/`std::time` under `crates/` → build fails. |
| 3 | No hash-order dependence. | `kernel-policy`: `HashMap`/`HashSet` under `crates/` → build fails. `BTreeMap` or indexed vectors instead. |
| 4 | The kernel never touches the presentation layer. | `kernel-policy`: `web_sys`/`js_sys`/`wasm_bindgen` referenced from a kernel crate → build fails. |
| 5 | Arithmetic is `f64` with no fast-math, no FMA reassociation, no `-ffast-math`. | Rust does not apply fast-math by default; `Cargo.toml` sets `panic = "abort"` and fixed codegen units. Revisit only with evidence. |
| 6 | The golden hash is committed and checked on every platform. | `cargo test` natively **and** `tools/determinism.mjs` against the wasm artifact. |

A seventh, subtler one: **allocation order must not affect results.** The kernel uses fixed-capacity
buffers and index-based structures, never address-ordered iteration. This matters serially, not
concurrently, but it is the same class of bug as `HashMap` ordering and is treated the same way.

### 1.4 The golden hash

`odderon_dmath::golden_hash()` folds every function over 34 probe inputs chosen to hit each reduction
branch, and returns one `u64`. It is:

- computed natively by `cargo test` in CI;
- computed in wasm and compared by `tools/determinism.mjs` in the dev sandbox;
- later, computed in a headless browser against the full kernel.

The expected value lives in **`crates/odderon-dmath/tests/golden.txt`**, not as a constant in code, so
that recomputing it is a one-line reviewable diff. **Bootstrap:** the file is absent in the first commit,
which makes the test print the computed hash and pass — the value is then committed from the first CI
log. After that the gate is live and a single differing bit fails the build.

An intentional algorithm change means updating `golden.txt` **and** noting it here, in the same commit.
An unintentional one is a bug with a number attached.

### 1.4b A correction worth keeping visible

The first version of `exp` used **ten** terms of the Taylor series, with a comment claiming "~1e-18
relative". Both halves of that were wrong:

- truncation error is `|r|^n / n!` at the interval edge `|r| = ln2/2 = 0.3466`, which for `n = 10` is
  **6.9e−12** — four orders worse than claimed, and the measured error matched theory to three digits;
- it takes **fourteen** terms to reach the f64 evaluation floor of ~2.2e−16, beyond which more terms buy
  nothing because Horner's rounding dominates.

It was caught by `tools/verify-math.mjs` — a JavaScript transcription of the algorithm — running in the
dev sandbox *before* the Rust test could have caught it. Three lessons, all of which are the project's
stated method working as intended:

1. **The comment was the bug.** A stated tolerance nobody measured is worse than no tolerance, because it
   is believed. Every coefficient count in this crate now derives from a truncation-error bound that is
   written next to it.
2. **Verification does not have to happen in Rust.** IEEE-754 binary64 is IEEE-754 binary64; JS `number`
   and Rust `f64` agree bit for bit on `+ − × ÷`. An algorithm can therefore be checked, and its
   tolerance measured, before it is ever compiled.
3. **The failing test was right and the comment was wrong.** The instinct to widen the tolerance is the
   instinct that produces a renderer which reports a benchmark it never took.

### 1.5 What determinism does *not* mean

- **Not "correctly rounded".** `odderon-dmath` targets ~1e-15 relative accuracy, tested against the
  platform's own implementation. 1-ulp-correct transcendentals are a much harder problem and are not
  required for reproducibility — only for bragging.
- **Not "same speed everywhere".** Only same results.
- **Not "no floating point".** `f64` is fine precisely because its basic operations are specified. The
  enemy is unspecified *functions*, not floating point.

---

## Part 2 — The CI-as-compiler loop

### 2.1 The constraint that produced it

The development sandbox has Node, Python, and a headless Chromium — and **no Rust toolchain, and no
network route to `static.rust-lang.org` or `crates.io`**. Verified, not assumed:

```
rustc / cargo           absent
static.rust-lang.org     unreachable
crates.io                unreachable
registry.npmjs.org       reachable   (this is why the harness is JS)
```

So Rust is compiled by **GitHub Actions**, which has a toolchain and a network. The loop:

```
        ┌───────────────┐   git push    ┌──────────────────┐
        │ dev sandbox   │ ────────────► │ GitHub Actions    │
        │ (no rustc)    │               │ cargo test/build  │
        └───────────────┘               └──────────────────┘
                ▲                                │
                │  gh run download (wasm)        │ artifact
                └────────────────────────────────┘
                │
                ▼
        node tools/determinism.mjs   ← runs the wasm natively, no browser
```

### 2.2 Why this is better than a local toolchain, not merely a substitute

1. **The reproducibility claim gets a witness.** The whole architecture promises that a mission
   re-simulates identically on *any* machine. A local compiler proves nothing about another machine — it
   is the same machine. CI is a *different* machine from day one, so "native and wasm agree" is tested
   for real from the first commit rather than at release.
2. **The lint is not advisory.** Kernel policy (rules 1–4) is a `grep` job on a machine that cannot be
   argued with. Locally, a lint is a habit.
3. **The build environment is pinned by the repository.** `rust-toolchain.toml` fixes 1.83.0, so the
   compiler version is a reviewable file rather than a developer's habit.
4. **Nothing compiles in a dirty tree.** Every artifact is tied to a commit hash, so a benchmark or a
   determinism check can always be traced to the source that produced it.

The cost is real and worth naming: **latency** (a 30–60 s round trip per iteration instead of
instantaneous), and **compile errors arrive in batches** rather than as I type. The mitigation is not to
hide it but to design for it (§2.3).

### 2.3 How the code is written for a compiler it cannot see

1. **Small crates, wide interfaces.** A mistake's blast radius is one crate.
2. **`core`/`std` only. No external crates in the kernel.** Every dependency is a version that must
   resolve on a machine I cannot poll. (This is also a supply-chain position, not just a convenience.)
3. **No cleverness in the kernel.** Concrete types, explicit arithmetic, no trait gymnastics, no
   macros beyond `#[derive]`.
4. **Tests shipped with the code**, because `cargo test` *is* the verification step I cannot perform.
5. **`#![forbid(unsafe_code)]`** in kernel crates. This removes an entire category of bug I cannot
   catch by reading, and it is affordable because the hot paths are arithmetic, not pointer work.
6. **First push of any block is expected to need a fix-up pass.** That is priced into the plan, not
   treated as failure.
7. **A `no_std` + `core` discipline** keeps the wasm artifact small and prevents accidental dependence
   on host facilities — which is the same thing as preventing accidental nondeterminism.

### 2.4 Advisory versus hard gates, during bootstrap

`ci.yml` marks `rustfmt` and `clippy` as `continue-on-error: true` for now. Rationale: the very first
run of code written without a compiler will trip style lints whose fixes are noise, and a red build that
means nothing teaches everyone to ignore red builds. They are armed (`continue-on-error: false`) after
the first green run, and that arming is a one-word diff.

The **hard** gates — tests, wasm build, kernel policy, zero-binary-assets — are never advisory. A red
build there means the physics or the determinism is wrong, which is exactly when a red build is useful.

---

## Part 3 — What would make this fail

| Failure | Early sign | Response |
| --- | --- | --- |
| Golden hash differs native vs wasm | `determinism.mjs` mismatch on first run | Expected possibility: wasm's `f64` `sqrt` and the native one are both IEEE-754-exact, so this should not happen; if it does, the culprit is a function I believed was exact and was not. Find it by bisecting the probe set. |
| Round-trip latency kills flow | Long gaps between iterations | Batch changes deliberately; run `cargo test` locally is impossible, so make each push count. The block plan already limits scope per block. |
| Determinism eroded later for "performance" | A PR adds `f64::sin` with an `allow` | The lint is `-D` by default and an exception requires editing `docs/DETERMINISM.md` — the friction is the point. |
| The rating turns out to be unverifiable | Any unexplained hash mismatch after rung 2 | `CREDIBILITY.md` §14 already names this a top-3 engineering risk; the pre-committed response is that replays become advisory and Esse becomes local-only. That is a large loss, which is why the gate is built first. |

The last row is why this document exists before the first simulation line. **A project that discovers
determinism late cannot afford to fix it.**
