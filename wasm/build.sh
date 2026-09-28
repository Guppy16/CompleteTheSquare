#!/usr/bin/env bash
# Build the AI to WebAssembly and place it where the page loads it from.
set -euo pipefail
cd "$(dirname "$0")"
cargo build --release --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/release/complete_the_square_ai.wasm ../square-game/ai.wasm
ls -l ../square-game/ai.wasm
