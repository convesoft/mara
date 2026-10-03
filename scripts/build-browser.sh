#!/usr/bin/env bash
# Build only the portable binding; never compile the native host into browser assets.
set -euo pipefail
if [[ $# != 1 ]]; then echo "usage: $0 OUTPUT_DIRECTORY" >&2; exit 2; fi
: "${CARGO_TARGET_DIR:?Set a disposable Cargo target directory}"
: "${TMPDIR:?Set a disposable temporary directory}"
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
output=$(realpath -m -- "$1")
mkdir -p -- "$output"
cd -- "$root"
cargo build --locked --release -p mara-wasm --target wasm32-unknown-unknown
wasm-bindgen --version | grep -qx 'wasm-bindgen 0.2.127'
wasm-bindgen "$CARGO_TARGET_DIR/wasm32-unknown-unknown/release/mara_wasm.wasm" \
  --target web --out-dir "$output" --out-name mara_wasm
cp -- npm/browser/worker.mjs "$output/worker.mjs"
