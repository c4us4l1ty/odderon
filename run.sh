#!/usr/bin/env sh
# Odderon — one command to play.
#   ./run.sh          start the dev server on http://localhost:8080
#   ./run.sh test     run the test suite
set -e
cd "$(dirname "$0")"
case "${1:-serve}" in
  test|tests) node test/run.mjs ;;
  *)          exec python3 server.py "$@" ;;
esac
