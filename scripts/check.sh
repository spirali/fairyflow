#!/bin/bash

set -e

cd `dirname $0`/..

# Python
uvx ruff format --check
uvx ruff check src tests fairyflow-zensical

# Rust
cargo fmt --check
cargo clippy

# TypeScript (web/)
cd web
npm run format:check
npm run typecheck
npm run lint
