#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

expect_failure() {
	if "$@"; then
		echo "expected command to fail: $*" >&2
		exit 1
	fi
}

cat >"$TMP_DIR/coverage.lcov" <<'EOF'
TN:
SF:src/lib.rs
LF:20
LH:15
end_of_record
EOF

"$ROOT_DIR/scripts/ci/coverage-summary.sh" \
	"$TMP_DIR/coverage.lcov" "$TMP_DIR/coverage.json" deadbeef refs/heads/test
jq -e '
  .schema_version == 1
  and .commit == "deadbeef"
  and .ref == "refs/heads/test"
  and .lines_found == 20
  and .lines_hit == 15
  and .percentage == 75
' "$TMP_DIR/coverage.json" >/dev/null

cat >"$TMP_DIR/empty.lcov" <<'EOF'
TN:
SF:src/lib.rs
LF:0
LH:0
end_of_record
EOF
expect_failure "$ROOT_DIR/scripts/ci/coverage-summary.sh" \
	"$TMP_DIR/empty.lcov" "$TMP_DIR/empty.json" deadbeef refs/heads/test

mkdir -p "$TMP_DIR/package"
printf '# test\n' >"$TMP_DIR/README.md"
printf 'license\n' >"$TMP_DIR/LICENSE"
printf '#!/usr/bin/env sh\nexit 0\n' >"$TMP_DIR/looprs"
chmod +x "$TMP_DIR/looprs"

python3 "$ROOT_DIR/scripts/ci/package-release-unix.py" \
	--binary "$TMP_DIR/looprs" \
	--readme "$TMP_DIR/README.md" \
	--license "$TMP_DIR/LICENSE" \
	--output "$TMP_DIR/package/looprs-v0.6.0-linux-x86_64.tar.gz" \
	--epoch 1700000000
test -s "$TMP_DIR/package/looprs-v0.6.0-linux-x86_64.tar.gz"
test -s "$TMP_DIR/package/looprs-v0.6.0-linux-x86_64.tar.gz.sha256"
(cd "$TMP_DIR/package" && shasum -a 256 -c looprs-v0.6.0-linux-x86_64.tar.gz.sha256)
expect_failure python3 "$ROOT_DIR/scripts/ci/package-release-unix.py" \
	--binary "$TMP_DIR/missing" \
	--readme "$TMP_DIR/README.md" \
	--license "$TMP_DIR/LICENSE" \
	--output "$TMP_DIR/package/missing.tar.gz" \
	--epoch 1700000000

test "$("$ROOT_DIR/scripts/ci/release-binary-path.sh" x86_64-unknown-linux-gnu Linux)" = \
	"target/x86_64-unknown-linux-gnu/release/looprs"
test "$("$ROOT_DIR/scripts/ci/release-binary-path.sh" x86_64-pc-windows-msvc Windows)" = \
	"target/x86_64-pc-windows-msvc/release/looprs.exe"
test "$("$ROOT_DIR/scripts/ci/release-binary-path.sh" aarch64-apple-darwin macOS)" = \
	"target/aarch64-apple-darwin/release/looprs"
expect_failure "$ROOT_DIR/scripts/ci/release-binary-path.sh" target Plan9

ASSETS="$TMP_DIR/assets"
mkdir -p "$ASSETS"
for asset in linux-x86_64 macos-aarch64; do
	printf 'archive\n' >"$ASSETS/looprs-v0.6.0-${asset}.tar.gz"
	printf '{}\n' >"$ASSETS/looprs-v0.6.0-${asset}.spdx.json"
done
printf 'archive\n' >"$ASSETS/looprs-v0.6.0-windows-x86_64.zip"
printf '{}\n' >"$ASSETS/looprs-v0.6.0-windows-x86_64.spdx.json"
printf '{}\n' >"$ASSETS/release-health.json"
printf '{}\n' >"$ASSETS/release-health-history.jsonl"
printf '# release\n' >"$ASSETS/RELEASE_NOTES.md"
for archive in "$ASSETS"/*.tar.gz "$ASSETS"/*.zip; do
	(cd "$ASSETS" && shasum -a 256 "${archive##*/}" >"${archive##*/}.sha256")
done

"$ROOT_DIR/scripts/ci/validate-release-assets.sh" "$ASSETS" 0.6.0 >"$TMP_DIR/assets.txt"
test "$(wc -l <"$TMP_DIR/assets.txt" | tr -d ' ')" -eq 11
rm "$ASSETS/looprs-v0.6.0-linux-x86_64.tar.gz.sha256"
expect_failure "$ROOT_DIR/scripts/ci/validate-release-assets.sh" "$ASSETS" 0.6.0

if command -v pwsh >/dev/null 2>&1; then
	pwsh -NoProfile -File "$ROOT_DIR/scripts/ci/package-release-windows.ps1" \
		-Binary "$TMP_DIR/looprs" \
		-Readme "$TMP_DIR/README.md" \
		-License "$TMP_DIR/LICENSE" \
		-Output "$TMP_DIR/package/looprs-v0.6.0-windows-x86_64.zip"
	test -s "$TMP_DIR/package/looprs-v0.6.0-windows-x86_64.zip"
	test -s "$TMP_DIR/package/looprs-v0.6.0-windows-x86_64.zip.sha256"
	expect_failure pwsh -NoProfile -File "$ROOT_DIR/scripts/ci/package-release-windows.ps1" \
		-Binary "$TMP_DIR/missing.exe" \
		-Readme "$TMP_DIR/README.md" \
		-License "$TMP_DIR/LICENSE" \
		-Output "$TMP_DIR/package/missing.zip"
fi

echo "workflow contract tests passed"
