#!/usr/bin/env bash
# Idempotent sandbox provisioning for restricted-egress environments.
#
# Restores everything the tracked tree deliberately does not carry: the Rust
# toolchain, vendored crate sources, node_modules and a headless Chromium.
# Safe to re-run; each stage is skipped when its output already exists.
#
#   ./scripts/provision.sh          # provision
#   source scripts/env.sh           # then put the tools on PATH
#   ./scripts/gate.sh               # run the full acceptance sweep
set -euo pipefail
cd "$(dirname "$0")/.."
ROOT="$PWD"
CACHE="$ROOT/.cache"
mkdir -p "$CACHE"

say() { printf '\n==> %s\n' "$*"; }

# ---------------------------------------------------------------- toolchain
if [ -x "$CACHE/rust/prefix/bin/cargo" ]; then
  say "Rust toolchain already present"
else
  say "Installing Rust toolchain from PyPI"
  [ -d .venv ] || python3 -m venv .venv
  .venv/bin/pip install --quiet --disable-pip-version-check \
    "arena-rust-toolchain[all]" zstandard
  # The packaged installer shells out to a `zstd` binary that is not present
  # on this image, so decompress the split parts with Python instead.
  .venv/bin/python - <<'PY'
import pathlib, tarfile, zstandard as zstd
base = pathlib.Path(".venv/lib/python3.11/site-packages")
parts = []
for i in (1, 2, 3, 4):
    d = base / f"arena_rust_toolchain_data{i}"
    if d.is_dir():
        parts += sorted(p for p in d.iterdir()
                        if p.is_file() and not p.name.startswith("_"))
if not parts:
    raise SystemExit("no toolchain parts found")
class Reader:
    def __init__(self, paths):
        self.paths, self.i = paths, 0
        self.f = open(paths[0], "rb")
    def read(self, n=-1):
        buf = b""
        while n != 0:
            chunk = self.f.read(n if n > 0 else -1)
            if chunk:
                buf += chunk
                if n > 0:
                    n -= len(chunk)
                if n == 0 or len(buf) > 64 * 2**20:
                    break
            else:
                self.f.close()
                self.i += 1
                if self.i >= len(self.paths):
                    break
                self.f = open(self.paths[self.i], "rb")
        return buf
target = pathlib.Path(".cache/rust")
target.mkdir(parents=True, exist_ok=True)
with zstd.ZstdDecompressor().stream_reader(Reader(parts)) as sr:
    with tarfile.open(fileobj=sr, mode="r|") as tf:
        tf.extractall(target)
print("toolchain extracted to", target)
PY
fi
export PATH="$CACHE/rust/prefix/bin:$PATH"
export CARGO_HOME="$CACHE/cargo"
mkdir -p "$CARGO_HOME"
rustc --version
if [ ! -d "$CACHE/rust/prefix/lib/rustlib/wasm32-unknown-unknown" ]; then
  echo "ERROR: wasm32-unknown-unknown std missing from toolchain" >&2
  exit 1
fi

# ------------------------------------------------------------------- vendor
say "Vendoring crate sources"
python3 scripts/vendor.py
# Keep source replacement out of the tracked tree: CI uses the real registry.
cat > "$CARGO_HOME/config.toml" <<EOF
[source.crates-io]
replace-with = "vendored-sources"

[source.vendored-sources]
directory = "$CACHE/vendor"

[net]
offline = true
EOF
rm -f "$ROOT/.cargo/config.toml"
rmdir "$ROOT/.cargo" 2>/dev/null || true

# --------------------------------------------------------------------- node
if [ -d node_modules/playwright-core ] && [ -d node_modules/@sparticuz/chromium ]; then
  say "node modules already present"
else
  say "Installing node dependencies"
  npm ci
fi

# ----------------------------------------------------------------- chromium
if [ -x /tmp/chromium ] && [ -f "$CACHE/al2023/lib/libnspr4.so" ]; then
  say "Chromium already present"
else
  say "Extracting headless Chromium"
  # The package is dual-published: the ESM entry has a `default` export, the
  # CJS entry *is* the class. Accept both so a clean provision works.
  node -e '
    const M = require("@sparticuz/chromium");
    const C = M.default ?? M;
    C.executablePath().then(p => console.log("chromium:", p))
      .catch(e => { console.error(e); process.exit(1); });
  '
  # al2023.tar.br holds the shared libraries Chromium needs. The package only
  # unpacks it on Amazon Linux, so extract it explicitly everywhere else.
  node -e '
    const fs = require("node:fs"), zlib = require("node:zlib"),
          path = require("node:path"), tar = require("tar-fs");
    const out = path.resolve(".cache/al2023");
    fs.mkdirSync(out, { recursive: true });
    if (fs.existsSync(path.join(out, "lib", "libnspr4.so"))) {
      console.log("al2023 libs already present");
    } else {
      fs.createReadStream(path.join("node_modules", "@sparticuz", "chromium",
                                    "bin", "al2023.tar.br"))
        .pipe(zlib.createBrotliDecompress())
        .pipe(tar.extract(out))
        .on("finish", () => console.log("al2023 libs:", out))
        .on("error", e => { console.error(e); process.exit(1); });
    }
  '
fi

# --------------------------------------------------------------- wasm asset
say "Building browser WebAssembly module"
cargo build --locked --offline --release -p odd-view --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/release/odd_view.wasm web/odd_view.wasm

say "Provisioned. Next:"
cat <<'EOF'
  source scripts/env.sh     # PATH, CARGO_HOME, CHROME_BIN, LD_LIBRARY_PATH
  ./scripts/gate.sh         # full acceptance sweep
EOF
