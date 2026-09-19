#!/usr/bin/env bash
set -euo pipefail

VERSION="${1:?usage: verify-release-health.sh VERSION [BASELINE] [HISTORY]}"
BASELINE="${2:-.health-baseline.json}"
HISTORY="${3:-.health-history.jsonl}"

if [[ ! "$VERSION" =~ ^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$ ]]; then
	echo "Invalid release version: $VERSION" >&2
	exit 1
fi

if [[ ! -s "$BASELINE" || ! -s "$HISTORY" ]]; then
	echo "Release health baseline and history must both be non-empty." >&2
	exit 1
fi

latest="$(jq -s 'last' "$HISTORY")"
if ! jq -e --arg version "$VERSION" '
  .version == $version
  and .versions_consistent == true
  and .tests.total > 0
  and .tests.passed >= 0
  and .tests.failed == 0
  and .tests.skipped >= 0
  and .tests.total == (.tests.passed + .tests.failed + .tests.skipped)
  and .clippy.warnings == 0
  and .clippy.errors == 0
  and .coverage >= 0
  and .coverage <= 100
' <<<"$latest" >/dev/null; then
	echo "Latest health history entry is not valid for release ${VERSION}." >&2
	exit 1
fi

baseline_normalized="$(mktemp)"
history_normalized="$(mktemp)"
trap 'rm -f "$baseline_normalized" "$history_normalized"' EXIT

jq -S . "$BASELINE" >"$baseline_normalized"
jq -S . <<<"$latest" >"$history_normalized"
if ! cmp -s "$baseline_normalized" "$history_normalized"; then
	echo "Health baseline does not match the latest persisted history entry." >&2
	exit 1
fi

echo "Release health verified for ${VERSION}."
