#!/usr/bin/env bash
# scripts/build-dist.sh: Compile Leptos WASM release bundle and package into dist/
set -euo pipefail

cd "$(dirname "$0")/.."

echo "Building action-runner-web release WASM binary..."
cargo build --target wasm32-unknown-unknown --release

echo "Packaging web distribution with wasm-bindgen..."
mkdir -p dist
wasm-bindgen --target web --out-dir dist --out-name action-runner-web target/wasm32-unknown-unknown/release/action-runner-web.wasm

cp style.css dist/
cp index.html dist/

echo "Build complete: dist/ is ready for Nginx deployment."
ls -lh dist/
