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

# Cache-busting: calculate SHA256 content hash of the compiled WASM binary
if command -v shasum >/dev/null 2>&1; then
    BUILD_ID=$(shasum -a 256 dist/action-runner-web_bg.wasm | cut -c1-10)
elif command -v sha256sum >/dev/null 2>&1; then
    BUILD_ID=$(sha256sum dist/action-runner-web_bg.wasm | cut -c1-10)
else
    BUILD_ID=$(date +%s)
fi

echo "Injecting cache-busting version (?v=${BUILD_ID}) into dist/index.html..."
sed -e "s|href=\"\./style\.css\"|href=\"./style.css?v=${BUILD_ID}\"|g" \
    -e "s|from '\./action-runner-web\.js'|from './action-runner-web.js?v=${BUILD_ID}'|g" \
    -e "s|init('\./action-runner-web_bg\.wasm')|init('./action-runner-web_bg.wasm?v=${BUILD_ID}')|g" \
    index.html > dist/index.html

echo "Build complete: dist/ is ready for Nginx deployment (Build ID: ${BUILD_ID})."
ls -lh dist/
