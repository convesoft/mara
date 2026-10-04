#!/usr/bin/env bash
# Build a local addon only. Host/package certification is a downstream gate.
set -euo pipefail
if [[ $# != 1 ]]; then echo "usage: $0 OUTPUT_DIRECTORY" >&2; exit 2; fi
: "${CARGO_TARGET_DIR:?Set a disposable Cargo target directory}"
: "${TMPDIR:?Set a disposable temporary directory}"
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
mkdir -p -- "$1"
output=$(cd -- "$1" && pwd -P)
cd -- "$root"
cargo build --locked -p mara-node
case "$(uname -s)" in
  Linux) library=libmara_node.so ;;
  Darwin) library=libmara_node.dylib ;;
  *) echo "Unsupported native host" >&2; exit 2 ;;
esac
cp -- "$CARGO_TARGET_DIR/debug/$library" "$output/mara.node"
cp -- crates/mara-node/loader.mjs crates/mara-node/loader.d.mts "$output/"
