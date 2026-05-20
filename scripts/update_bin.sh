#!/bin/bash
# Rebuild the Rust server binary and stage it into the Python package tree
# so that Zensical and other local tools pick up the latest version.
#
# Usage:
#   scripts/update_bin.sh           # release build (default)
#   scripts/update_bin.sh --debug   # debug build (faster, larger binary)

set -euo pipefail
cd "$(dirname "$0")/.."

DEBUG=0
for arg in "$@"; do
    [[ "$arg" == "--debug" ]] && DEBUG=1
done

if [[ $DEBUG -eq 1 ]]; then
    cargo build -p server
    SRC="target/debug/server"
else
    cargo build --release -p server
    SRC="target/release/server"
fi

cp "$SRC" src/fairyflow/_bin/server
chmod +x src/fairyflow/_bin/server
echo "updated src/fairyflow/_bin/server from $SRC"

rm -rf .ffpy_cache docs/assets/ffpy .cache
echo "cleared zensical caches"
echo ""
echo "Run 'uv run zensical serve' to rebuild and preview the docs."
