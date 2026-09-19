#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SITE_DIR="${LOOPRS_SITE_DIR:-$ROOT_DIR/site}"
PORT="${1:-4173}"

if [[ ! -d "$SITE_DIR" ]]; then
	echo "site directory not found: $SITE_DIR" >&2
	exit 1
fi

if [[ ! "$PORT" =~ ^[0-9]+$ ]] || ((PORT < 0 || PORT > 65535)); then
	echo "invalid port: $PORT" >&2
	exit 1
fi

exec python3 -u "$ROOT_DIR/scripts/serve-site.py" "$SITE_DIR" "$PORT"
