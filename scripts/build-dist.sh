#!/usr/bin/env bash
# scripts/build-dist.sh: Compile Leptos WASM release bundle and package into dist/
set -euo pipefail

cd "$(dirname "$0")/.."

echo "Building mu-ci-web release WASM binary..."
cargo build --target wasm32-unknown-unknown --release

echo "Packaging web distribution with wasm-bindgen..."
mkdir -p dist
wasm-bindgen --target web --out-dir dist --out-name mu-ci-web target/wasm32-unknown-unknown/release/mu-ci-web.wasm

cp style.css dist/

# Cache-busting: calculate SHA256 content hash of the compiled WASM binary
if command -v shasum >/dev/null 2>&1; then
    BUILD_ID=$(shasum -a 256 dist/mu-ci-web_bg.wasm | cut -c1-10)
elif command -v sha256sum >/dev/null 2>&1; then
    BUILD_ID=$(sha256sum dist/mu-ci-web_bg.wasm | cut -c1-10)
else
    BUILD_ID=$(date +%s)
fi

echo "Injecting cache-busting version (?v=${BUILD_ID}) into dist/index.html..."
sed -e "s|href=\"\./style\.css\"|href=\"./style.css?v=${BUILD_ID}\"|g" \
    -e "s|from '\./mu-ci-web\.js'|from './mu-ci-web.js?v=${BUILD_ID}'|g" \
    -e "s|init('\./mu-ci-web_bg\.wasm')|init('./mu-ci-web_bg.wasm?v=${BUILD_ID}')|g" \
    index.html > dist/index.html

echo "Build complete: dist/ is ready for Nginx deployment (Build ID: ${BUILD_ID})."
ls -lh dist/
