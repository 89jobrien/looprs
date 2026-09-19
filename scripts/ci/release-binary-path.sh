#!/usr/bin/env bash
set -euo pipefail

TARGET="${1:?usage: release-binary-path.sh TARGET RUNNER_OS}"
RUNNER_OS="${2:?usage: release-binary-path.sh TARGET RUNNER_OS}"

case "$RUNNER_OS" in
Linux | macOS) binary="looprs" ;;
Windows) binary="looprs.exe" ;;
*)
	echo "unsupported runner OS: $RUNNER_OS" >&2
	exit 1
	;;
esac

printf 'target/%s/release/%s\n' "$TARGET" "$binary"
