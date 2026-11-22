#!/usr/bin/env bash
# Run a command; if it fails, surface its diagnostics as GitHub check annotations.
#
# Why this exists: job logs are served from blob storage, which a restricted
# environment (or an agent working without browser access) may not be able to
# fetch. Annotations are served by the GitHub API itself, so a failure reported
# this way is readable from anywhere that can talk to api.github.com:
#
#   gh api repos/{owner}/{repo}/check-runs/{id}/annotations \
#     --jq '.[] | "\(.path):\(.start_line) \(.message)"'
#
# Usage: ci/annotate.sh <label> <command...>
set -uo pipefail

label="$1"
shift
log="$(mktemp)"

set +e
"$@" 2>&1 | tee "$log"
status="${PIPESTATUS[0]}"
set -e

if [ "$status" -eq 0 ]; then
  exit 0
fi

echo "::error::${label} failed (exit ${status})"

# Emit each diagnostic as its own annotation, capped so a catastrophic failure
# cannot spam thousands of them.
n=0
while IFS= read -r line; do
  case "$line" in
    error\[*|error:*|warning:*|"  -->"*|"note: "*)
      # Escape the workflow-command delimiters that would otherwise truncate a line.
      safe="${line//'%'/%25}"
      safe="${safe//$'\r'/}"
      echo "::error::${safe}"
      n=$((n + 1))
      [ "$n" -ge 60 ] && { echo "::error::(truncated at 60 annotations; see the job log for the rest)"; break; }
      ;;
  esac
done < "$log"

# Always end with the tail of the output, so a failure whose message does not
# match the patterns above is still visible.
echo "::error::---- last lines of ${label} ----"
tail -n 25 "$log" | while IFS= read -r line; do
  safe="${line//'%'/%25}"
  safe="${safe//$'\r'/}"
  echo "::error::${safe}"
done

exit "$status"
