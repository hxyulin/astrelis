#!/bin/sh
set -eu
cd "$(dirname "$0")/../.."
if command -v mbx >/dev/null 2>&1; then
    mbx build -p astrelis-web-example --target wasm32-unknown-unknown --release --locked
else
    echo "mbx unavailable; building with Cargo without shared compilation caching" >&2
    cargo build -p astrelis-web-example --target wasm32-unknown-unknown --release --locked
fi
build_dir=$(cargo metadata --no-deps --format-version 1 | python3 -c 'import json, sys; print(json.load(sys.stdin)["target_directory"])')
wasm-bindgen --target web --out-dir examples/web/pkg \
    "$build_dir/wasm32-unknown-unknown/release/astrelis_web_example.wasm"
cp crates/astrelis/tests/fonts/SourceSans3-OFL.md examples/web/SourceSans3-OFL.md
