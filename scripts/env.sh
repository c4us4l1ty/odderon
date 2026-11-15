#!/usr/bin/env bash
# Source this after scripts/provision.sh: `source scripts/env.sh`
# Exports the sandbox-local toolchain and headless browser paths.
_ODD_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export PATH="$_ODD_ROOT/.cache/rust/prefix/bin:$PATH"
export CARGO_HOME="$_ODD_ROOT/.cache/cargo"
export LD_LIBRARY_PATH="$_ODD_ROOT/.cache/al2023/lib:/tmp${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export CHROME_BIN="/tmp/chromium"
unset _ODD_ROOT
