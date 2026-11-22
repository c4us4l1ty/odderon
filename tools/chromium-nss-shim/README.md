# chromium-nss-shim — run a headless Chromium in a container with no NSS

`tools/ensure-chromium.mjs` can download a prebuilt Chromium from npm
(`@sparticuz/chromium`), but that binary is dynamically linked against
`libnspr4.so`, `libnss3.so` and `libnssutil3.so`. Minimal sandboxes
(e.g. the Arena / E2B images) do not ship them and `apt-get` cannot reach
Debian mirrors.

Chromium only needs NSS for the *system certificate store*. A headless renderer
that loads `http://127.0.0.1` never touches TLS, so a no-op shim is enough.
`build-shim.sh` produces the three `.so` files from `shim.c`:

```
libnspr4.so     – empty, right SONAME
libnssutil3.so  – NSS_SetAlgorithmPolicy -> 0
libnss3.so      – the ~40 CERT_/PK11_/SECMOD_ symbols Chromium imports,
                  versioned NSS_3.2 … NSS_3.30 (glibc refuses to bind a
                  versioned reference to an unversioned object, so the
                  version script in nss.map is required, not optional)
```

Everything returns `0` (= `SECSuccess` / `NULL`), and `NSS_VersionCheck`
returns `1` so Chromium's `EnsureNSSInit()` is satisfied.

`ld.so` prints `no version information available (required by …)` for the
nspr/util stubs on stderr; that is expected and harmless.

This directory is **tooling only** — it is not part of the game, it ships no
bytes to the browser, and it is ignored by the zero-asset budget check.
