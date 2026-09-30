#!/bin/sh
# Build the WebAssembly player into web/dist/ (static files, serve from any host).
#   ./web/build.sh && python3 -m http.server -d web/dist 8000
set -e
cd "$(dirname "$0")"
cargo build --release --target wasm32-unknown-unknown
rm -rf dist && mkdir -p dist/roms
cp target/wasm32-unknown-unknown/release/nes_web.wasm dist/
cp index.html nes-player.js dist/
cp ../roms/*.nes ../roms/ROMS.md dist/roms/
echo "built web/dist ($(wc -c < dist/nes_web.wasm | tr -d ' ') byte wasm)"
