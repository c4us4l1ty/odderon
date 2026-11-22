#!/usr/bin/env bash
# Build the no-op NSS/NSPR shim libraries used to run a prebuilt Chromium inside a
# container that has no libnss3. See README.md.  Usage: build-shim.sh [outdir]
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUT="${1:-$HERE/build}"
mkdir -p "$OUT"

cc -shared -fPIC -O1 -o "$OUT/libnss3.so" \
  -Wl,-soname,libnss3.so -Wl,--version-script="$HERE/nss3.map" "$HERE/nss3.c"
cc -shared -fPIC -O1 -o "$OUT/libnssutil3.so" \
  -Wl,-soname,libnssutil3.so -Wl,--version-script="$HERE/nssutil3.map" "$HERE/nssutil3.c"
cc -shared -fPIC -O1 -o "$OUT/libnspr4.so" -Wl,-soname,libnspr4.so "$HERE/nspr4.c"
echo "chromium NSS shim -> $OUT"
