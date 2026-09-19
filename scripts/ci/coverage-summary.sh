#!/usr/bin/env bash
set -euo pipefail

LCOV_PATH="${1:?usage: coverage-summary.sh LCOV OUTPUT COMMIT REF [SUMMARY]}"
OUTPUT_PATH="${2:?usage: coverage-summary.sh LCOV OUTPUT COMMIT REF [SUMMARY]}"
COMMIT="${3:?usage: coverage-summary.sh LCOV OUTPUT COMMIT REF [SUMMARY]}"
REF="${4:?usage: coverage-summary.sh LCOV OUTPUT COMMIT REF [SUMMARY]}"
SUMMARY_PATH="${5:-}"

if [[ ! -s "$LCOV_PATH" ]]; then
	echo "coverage input is missing or empty: $LCOV_PATH" >&2
	exit 1
fi

read -r lines_found lines_hit < <(
	awk -F: '
    /^LF:/ { found += $2 }
    /^LH:/ { hit += $2 }
    END { print found + 0, hit + 0 }
  ' "$LCOV_PATH"
)

if ((lines_found == 0)); then
	echo "$LCOV_PATH contains no instrumented lines" >&2
	exit 1
fi
if ((lines_hit < 0 || lines_hit > lines_found)); then
	echo "$LCOV_PATH contains invalid line totals: $lines_hit/$lines_found" >&2
	exit 1
fi

percentage="$(awk -v hit="$lines_hit" -v found="$lines_found" 'BEGIN {printf "%.4f", hit * 100 / found}')"
jq -n \
	--arg commit "$COMMIT" \
	--arg ref "$REF" \
	--argjson lines_found "$lines_found" \
	--argjson lines_hit "$lines_hit" \
	--argjson percentage "$percentage" \
	'{schema_version: 1, commit: $commit, ref: $ref, lines_found: $lines_found, lines_hit: $lines_hit, percentage: $percentage}' \
	>"$OUTPUT_PATH"
jq -e '.schema_version == 1 and .lines_found > 0 and .lines_hit >= 0 and .lines_hit <= .lines_found and .percentage >= 0 and .percentage <= 100' "$OUTPUT_PATH" >/dev/null

if [[ -n "$SUMMARY_PATH" ]]; then
	printf '### Workspace coverage: %s%% (%s/%s lines)\n' \
		"$percentage" "$lines_hit" "$lines_found" >>"$SUMMARY_PATH"
fi
