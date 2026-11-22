#!/usr/bin/env bash
# Run a command; if it fails, publish a diagnosable summary as a check annotation.
#
# Why this exists: job logs are served from blob storage, which a restricted client may not be able to
# fetch. That makes a failed build report "failure" with no cause, which is the least useful possible
# outcome. Annotations are served by the GitHub API itself:
#
#   gh api repos/{owner}/{repo}/check-runs/{id}/annotations --jq '.[].message'
#
# Two GitHub limits shape the implementation:
#   * a step emits at most 10 annotations, so emitting one per line loses the important ones;
#   * a message may contain newlines only if they are encoded as %0A.
#
# So: exactly ONE annotation, containing the tail of the output, newline-encoded.
#
# Usage: ci/annotate.sh <label> <command...>
set -uo pipefail

label="$1"
shift
log="$(mktemp)"

set +e
"$@" >"$log" 2>&1
status=$?
set -e

# Always show the output in the job log, for anyone who can read it.
cat "$log"

if [ "$status" -eq 0 ]; then
  rm -f "$log"
  exit 0
fi

# Build one encoded message: percent-escape first, then encode newlines, so the escaping cannot
# corrupt the separator. ANSI colour codes are stripped because they render as noise.
message="$(
  tail -n 80 "$log" \
    | sed -e 's/\x1b\[[0-9;]*m//g' \
    | sed -e 's/%/%25/g' \
    | sed -e 's/\r$//' \
    | awk '{ printf "%s%%0A", $0 }'
)"

# Keep well clear of the 64 KB annotation limit.
if [ "${#message}" -gt 60000 ]; then
  message="${message:0:60000}...[truncated]"
fi

printf '::error title=%s failed (exit %s)::%s\n' "$label" "$status" "$message"

rm -f "$log"
exit "$status"
