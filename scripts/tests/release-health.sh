#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
FIXTURES="$ROOT_DIR/scripts/tests/fixtures"
VERIFY="$ROOT_DIR/scripts/verify-release-health.sh"

"$VERIFY" 0.6.0 \
	"$FIXTURES/health-baseline-matching.json" \
	"$FIXTURES/health-history-matching.jsonl"

if "$VERIFY" 0.6.0 \
	"$FIXTURES/health-baseline-matching.json" \
	"$FIXTURES/health-history-version-mismatch.jsonl"; then
	echo "expected release version mismatch to fail" >&2
	exit 1
fi

if "$VERIFY" 0.6.0 \
	"$FIXTURES/health-baseline-matching.json" \
	"$FIXTURES/health-history-baseline-mismatch.jsonl"; then
	echo "expected baseline/history mismatch to fail" >&2
	exit 1
fi

if "$VERIFY" 0.6.0 \
	"$FIXTURES/health-baseline-matching.json" \
	"$FIXTURES/health-history-empty-tests.jsonl"; then
	echo "expected an empty test measurement to fail" >&2
	exit 1
fi

echo "release health fixture tests passed"
