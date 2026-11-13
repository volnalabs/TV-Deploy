#!/usr/bin/env bash
# Re-vendors the pinned Samsung tizen-sdk-skills CLI into resources/samsung-cli.
# Usage: scripts/vendor-samsung-cli.sh [<commit-sha>]   (default: the commit in PINNED.json)
# Updating is a deliberate act: review the diff, run `cargo test`, then commit PINNED.json.
set -euo pipefail
cd "$(dirname "$0")/.."
SHA="${1:-$(node -p "require('./resources/samsung-cli/PINNED.json').commit")}"
[[ "$SHA" =~ ^[0-9a-f]{40}$ ]] || { echo "need a full 40-char commit sha" >&2; exit 1; }
TMP="$(mktemp -d)"; trap 'rm -rf "$TMP"' EXIT
curl -fsSL "https://codeload.github.com/Samsung/tizen-agent-skills/tar.gz/$SHA" | tar -xz -C "$TMP"
SRC="$TMP/tizen-agent-skills-$SHA/tizen-sdk-skills"
rm -rf resources/samsung-cli/lib resources/samsung-cli/assets
mkdir -p resources/samsung-cli/assets
cp -r "$SRC/common/lib" resources/samsung-cli/lib
rm -rf resources/samsung-cli/lib/tests
cp -r "$SRC/common/assets/samsung-tv-ca" resources/samsung-cli/assets/
cp "$SRC/LICENSE" "$SRC/NOTICE" resources/samsung-cli/
TREE=$(cd resources/samsung-cli && find . -type f ! -name PINNED.json ! -name package.json | sort | xargs sha256sum | sha256sum | cut -d' ' -f1)
echo "vendored $SHA ; tree sha256 (excluding PINNED.json): $TREE"
echo "Update commit/tree_sha256 in resources/samsung-cli/PINNED.json by hand after review."
