#!/usr/bin/env bash
set -euo pipefail

ASSET_DIR="${1:?usage: validate-release-assets.sh DIRECTORY VERSION}"
VERSION="${2:?usage: validate-release-assets.sh DIRECTORY VERSION}"

if [[ ! "$VERSION" =~ ^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-[0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$ ]]; then
	echo "invalid semantic version: $VERSION" >&2
	exit 1
fi
if [[ ! -d "$ASSET_DIR" ]]; then
	echo "release asset directory not found: $ASSET_DIR" >&2
	exit 1
fi

upload_assets=(
	"looprs-v${VERSION}-linux-x86_64.tar.gz"
	"looprs-v${VERSION}-linux-x86_64.tar.gz.sha256"
	"looprs-v${VERSION}-linux-x86_64.spdx.json"
	"looprs-v${VERSION}-macos-aarch64.tar.gz"
	"looprs-v${VERSION}-macos-aarch64.tar.gz.sha256"
	"looprs-v${VERSION}-macos-aarch64.spdx.json"
	"looprs-v${VERSION}-windows-x86_64.zip"
	"looprs-v${VERSION}-windows-x86_64.zip.sha256"
	"looprs-v${VERSION}-windows-x86_64.spdx.json"
	"release-health.json"
	"release-health-history.jsonl"
)
required_assets=("${upload_assets[@]}" "RELEASE_NOTES.md")

for asset in "${required_assets[@]}"; do
	path="$ASSET_DIR/$asset"
	if [[ ! -s "$path" ]]; then
		echo "required release asset not found or empty: $path" >&2
		exit 1
	fi
done

for checksum in "$ASSET_DIR"/*.sha256; do
	(cd "$ASSET_DIR" && sha256sum --check "${checksum##*/}" >/dev/null)
done

for asset in "${upload_assets[@]}"; do
	path="$ASSET_DIR/$asset"
	printf '%s\n' "$path"
done
