#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="$HOME/.local/rust/bin:$HOME/.cargo/bin:$PATH"

if ! command -v cargo >/dev/null 2>&1; then
  echo "[Odderon] Bootstrapping Rust 1.88 + wasm32-unknown-unknown to ~/.local/rust..."
  mkdir -p /tmp/rustbin "$HOME/.local/rust"
  (
    cd /tmp/rustbin
    npm pack --quiet \
      @rustbin/rustc-1.88.0-x86_64-unknown-linux-gnu \
      @rustbin/cargo-1.88.0-x86_64-unknown-linux-gnu \
      @rustbin/rust-std-1.88.0-x86_64-unknown-linux-gnu \
      @rustbin/rust-std-1.88.0-wasm32-unknown-unknown
    for tgz in *.tgz; do
      mkdir -p sub && tar -xzf "$tgz" -C sub
      for comp in sub/package/*/; do
        if [ -d "$comp/bin" ] || [ -d "$comp/lib" ]; then
          cp -a "$comp"/* "$HOME/.local/rust/"
        fi
      done
      rm -rf sub
    done
  )
fi

echo "[Odderon] Running workspace deterministic physics & geometry tests..."
cargo test --workspace

echo "[Odderon] Compiling odderon-wasm -> wasm32-unknown-unknown (release)..."
cargo build -p odderon-wasm --target wasm32-unknown-unknown --release
mkdir -p web/wasm
cp target/wasm32-unknown-unknown/release/odderon_wasm.wasm web/wasm/odderon_wasm.wasm
ls -lh web/wasm/odderon_wasm.wasm
echo "[Odderon] Build complete."
