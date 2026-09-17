#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SITE_DIR="$ROOT_DIR/site"
PORT="${1:-4173}"

if [[ ! -d "$SITE_DIR" ]]; then
	echo "site directory not found: $SITE_DIR" >&2
	exit 1
fi

echo "Serving looprs static site from: $SITE_DIR"
echo "URL: http://127.0.0.1:$PORT"

python3 -m http.server "$PORT" --directory "$SITE_DIR"
