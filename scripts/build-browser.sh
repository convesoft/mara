#!/usr/bin/env bash
# Build only the portable binding; never compile the native host into browser assets.
set -euo pipefail
if [[ $# != 1 ]]; then echo "usage: $0 OUTPUT_DIRECTORY" >&2; exit 2; fi
: "${CARGO_TARGET_DIR:?Set a disposable Cargo target directory}"
: "${TMPDIR:?Set a disposable temporary directory}"
build_jobs=${CARGO_BUILD_JOBS:-2}
if [[ ! $build_jobs =~ ^[1-9][0-9]*$ ]]; then
  echo "CARGO_BUILD_JOBS must be a positive integer" >&2; exit 2
fi
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
mkdir -p -- "$1"
output=$(cd -- "$1" && pwd -P)
mkdir -p -- "$output/browser" "$output/internal"
cd -- "$root"
cargo build --locked --release -p mara-wasm --target wasm32-unknown-unknown -j "$build_jobs"
wasm-bindgen --version | grep -qx 'wasm-bindgen 0.2.127'
wasm-bindgen "$CARGO_TARGET_DIR/wasm32-unknown-unknown/release/mara_wasm.wasm" \
  --target web --out-dir "$output/browser" --out-name mara_wasm
cp -- npm/browser/worker.mjs "$output/browser/worker.mjs"
cp -- npm/client.mjs npm/client.d.mts npm/browser.mjs npm/browser.d.mts "$output/"
cp -- npm/internal/json.mjs "$output/internal/"
