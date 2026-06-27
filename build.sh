#!/usr/bin/env bash
# Build the Career Poker cdfy_next plugin to wasm.
set -euo pipefail

cd "$(dirname "$0")"

WASM="target/wasm32-unknown-unknown/release/cdfy_plugin_career_poker.wasm"

echo "building $(basename "$WASM") (wasm32-unknown-unknown, release)..."
cargo build --target wasm32-unknown-unknown --release

echo
echo "wasm: ${WASM}"
