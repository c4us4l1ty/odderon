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
FULL_TOOLING=0
if [ "${1:-}" = "--full" ]; then FULL_TOOLING=1; shift; fi
if [ "$#" -ne 0 ]; then echo "usage: $0 [--full]" >&2; exit 2; fi

say() { printf '\n==> %s\n' "$*"; }

# ---------------------------------------------------------------- toolchain
# Prefer a normal rustup installation on developer machines. The PyPI bundle is
# only a restricted-sandbox fallback; using it on a brand-new Python release
# can force source builds of zstandard and incorrectly require a C compiler.
if [ -x "$CACHE/rust/prefix/bin/cargo" ]; then
  say "Sandbox Rust toolchain already present"
  export PATH="$CACHE/rust/prefix/bin:$PATH"
elif command -v cargo >/dev/null 2>&1; then
  say "Using installed Rust toolchain"
elif command -v rustup >/dev/null 2>&1; then
  say "Installing stable Rust with rustup"
  rustup toolchain install stable --profile minimal
elif command -v curl >/dev/null 2>&1 && ! python3 -c 'import sys; raise SystemExit(0 if sys.version_info < (3, 14) else 1)'; then
  say "Installing Rust with rustup"
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | \
    sh -s -- -y --profile minimal
  export PATH="$HOME/.cargo/bin:$PATH"
else
  say "Installing sandbox Rust toolchain from PyPI"
  [ -d .venv ] || python3 -m venv .venv
  .venv/bin/pip install --quiet --disable-pip-version-check --only-binary=:all: \
    "arena-rust-toolchain[all]" zstandard || {
      echo "ERROR: Rust is not installed and no compatible prebuilt Python toolchain exists." >&2
      echo "Install rustup from https://rustup.rs, then run this script again." >&2
      exit 1
    }
  .venv/bin/python - <<'PY_TOOLCHAIN'
import importlib.util, pathlib, tarfile, zstandard as zstd
parts = []
for package in (f"arena_rust_toolchain_data{i}" for i in (1, 2, 3, 4)):
    spec = importlib.util.find_spec(package)
    if spec and spec.submodule_search_locations:
        directory = pathlib.Path(next(iter(spec.submodule_search_locations)))
        parts += sorted(p for p in directory.iterdir()
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
                if n > 0: n -= len(chunk)
                if n == 0 or len(buf) > 64 * 2**20: break
            else:
                self.f.close(); self.i += 1
                if self.i >= len(self.paths): break
                self.f = open(self.paths[self.i], "rb")
        return buf
target = pathlib.Path(".cache/rust")
target.mkdir(parents=True, exist_ok=True)
with zstd.ZstdDecompressor().stream_reader(Reader(parts)) as sr:
    with tarfile.open(fileobj=sr, mode="r|") as tf: tf.extractall(target)
print("toolchain extracted to", target)
PY_TOOLCHAIN
  export PATH="$CACHE/rust/prefix/bin:$PATH"
fi
export CARGO_HOME="$CACHE/cargo"
mkdir -p "$CARGO_HOME"
rustc --version
if ! rustc --print target-libdir --target wasm32-unknown-unknown >/dev/null 2>&1; then
  if command -v rustup >/dev/null 2>&1; then
    say "Installing the WebAssembly Rust target"
    rustup target add wasm32-unknown-unknown
  else
    echo "ERROR: wasm32-unknown-unknown is missing; install it with rustup target add wasm32-unknown-unknown" >&2
    exit 1
  fi
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

if [ "$FULL_TOOLING" -eq 1 ]; then
# --------------------------------------------------------------------- node
if [ -d node_modules/playwright-core ] && [ -d node_modules/@sparticuz/chromium ] && [ -d node_modules/miniflare ]; then
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

else
  say "Skipping browser test tooling (use --full to install it)"
fi

# --------------------------------------------------------------- wasm asset
say "Building browser WebAssembly module"
cargo build --locked --offline --release -p odd-view --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/release/odd_view.wasm web/odd_view.wasm

say "Odderon browser build is ready"
cat <<'EOF'
  bun run dev               # launch the game
  ./scripts/provision.sh --full && ./scripts/gate.sh  # full acceptance sweep
EOF
