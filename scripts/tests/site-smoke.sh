#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TMP_DIR="$(mktemp -d)"
SERVER_PID=""

cleanup() {
	if [[ -n "$SERVER_PID" ]]; then
		kill "$SERVER_PID" 2>/dev/null || true
		wait "$SERVER_PID" 2>/dev/null || true
	fi
	rm -rf "$TMP_DIR"
}
trap cleanup EXIT

if "$ROOT_DIR/scripts/serve-site.sh" nope >"$TMP_DIR/invalid.log" 2>&1; then
	echo "expected invalid port to fail" >&2
	exit 1
fi
if "$ROOT_DIR/scripts/serve-site.sh" 70000 >"$TMP_DIR/range.log" 2>&1; then
	echo "expected out-of-range port to fail" >&2
	exit 1
fi

if LOOPRS_SITE_DIR="$TMP_DIR/missing" "$ROOT_DIR/scripts/serve-site.sh" 0 \
	>"$TMP_DIR/missing.log" 2>&1; then
	echo "expected missing site directory to fail" >&2
	exit 1
fi

"$ROOT_DIR/scripts/serve-site.sh" 0 >"$TMP_DIR/server.log" 2>&1 &
SERVER_PID=$!

BASE_URL=""
for _ in $(seq 1 100); do
	while IFS= read -r line; do
		case "$line" in
		URL:\ *) BASE_URL="${line#URL: }" ;;
		esac
	done <"$TMP_DIR/server.log"
	if [[ -n "$BASE_URL" ]]; then
		break
	fi
	if ! kill -0 "$SERVER_PID" 2>/dev/null; then
		echo "site server exited before becoming ready" >&2
		exit 1
	fi
	sleep 0.05
done

if [[ -z "$BASE_URL" ]]; then
	echo "site server did not report its ephemeral port" >&2
	exit 1
fi

python3 "$ROOT_DIR/scripts/check-site.py" "$ROOT_DIR/site" "$BASE_URL"

echo "site smoke tests passed"
