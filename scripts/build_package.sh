#!/bin/bash
# Build a standalone Python wheel that bundles the Rust backend and web frontend.
# The installed 'fairyflow' command launches the Rust server directly.
#
# Usage:
#   scripts/build_package.sh
#
# Output: dist/fairyflow-*.whl

set -euo pipefail

cd "$(dirname "$0")/.."

# ---------------------------------------------------------------------------
# Prerequisite checks
# ---------------------------------------------------------------------------

command -v cargo >/dev/null 2>&1 || { echo "error: cargo not found"; exit 1; }
command -v npm   >/dev/null 2>&1 || { echo "error: npm not found";   exit 1; }
command -v uv    >/dev/null 2>&1 || { echo "error: uv not found";    exit 1; }

# ---------------------------------------------------------------------------
# 1. Build Rust backend (release)
# ---------------------------------------------------------------------------

echo "==> Building Rust backend (release)..."
cargo build --release -p server

RUST_BINARY="target/release/server"
if [[ ! -f "$RUST_BINARY" ]]; then
    echo "error: expected binary not found: $RUST_BINARY"
    exit 1
fi

# ---------------------------------------------------------------------------
# 2. Build web frontend
# ---------------------------------------------------------------------------

echo "==> Building web frontend..."
(cd web && npm run build)

WEB_DIST="web/dist"
if [[ ! -d "$WEB_DIST" ]]; then
    echo "error: expected web dist not found: $WEB_DIST"
    exit 1
fi

# ---------------------------------------------------------------------------
# 3. Copy artifacts into the Python package tree
#
# Layout inside the installed package:
#   fairyflow/_bin/server   — Rust binary
#   fairyflow/_web/         — static web files (served via FAIRYFLOW_WEB_DIST)
# ---------------------------------------------------------------------------

echo "==> Staging artifacts into src/fairyflow/..."

BIN_DEST="src/fairyflow/_bin"
WEB_DEST="src/fairyflow/_web"

rm -rf "$BIN_DEST" "$WEB_DEST"
mkdir -p "$BIN_DEST"
mkdir -p "$WEB_DEST"

cp "$RUST_BINARY" "$BIN_DEST/server"
chmod +x "$BIN_DEST/server"

cp -r "$WEB_DIST/." "$WEB_DEST/"

# ---------------------------------------------------------------------------
# 4. Build the Python wheel
# ---------------------------------------------------------------------------

echo "==> Building Python wheel..."
uv build --wheel

# ---------------------------------------------------------------------------
# Done
# ---------------------------------------------------------------------------

echo ""
echo "Done. Wheel is in dist/:"
ls -lh dist/*.whl
echo ""
echo "Install with:  pip install dist/fairyflow-*.whl"
echo "Then run:      fairyflow open <project-directory>"
