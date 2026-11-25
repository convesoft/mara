#!/usr/bin/env bash
# Builds the pinned Soufflé with Emscripten for the browser workspace, with the
# one-line patch scripts/souffle-emscripten.patch, as DES-SOUFFLE-BUILD in
# docs/distribution.mara.md specifies. Output: target/souffle-<version>-wasm/
# with souffle.js, souffle.wasm and Soufflé's LICENSE. A rerun only checks an
# existing module with the smoke run of scripts/souffle-wasm-smoke.mjs, which
# needs only node. A build also needs curl, tar, sha256sum, patch, cmake, flex,
# bison, and emcc and emcmake of the Emscripten SDK of EMSDK_VERSION on PATH;
# ninja if present.
set -euo pipefail

VERSION=2.5
URL="https://github.com/souffle-lang/souffle/archive/refs/tags/${VERSION}.tar.gz"
SHA256=5d009ad6c74ccec10207d865c059716afac625759bff7c8070e529bd80385067
EMSDK_VERSION=6.0.12

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
patch_file="$root/scripts/souffle-emscripten.patch"
out="$root/target/souffle-$VERSION-wasm"
work="$out/work"

die() { echo "build-souffle-wasm: $*" >&2; exit 1; }

command -v node >/dev/null || die "missing tool: node"

# Mara's invocation of the module in a fresh instance on an in-memory
# filesystem, as the browser workspace's Soufflé worker runs it.
smoke() {
    node --disable-warning=MODULE_TYPELESS_PACKAGE_JSON "$root/scripts/souffle-wasm-smoke.mjs" "$1" </dev/null \
        || die "smoke run of $1/souffle.js failed"
}

if [[ -f $out/souffle.js && -f $out/souffle.wasm ]]; then
    smoke "$out"
    echo "$out/souffle.wasm is built and passes the smoke run"
    exit 0
fi

for tool in curl tar sha256sum patch cmake flex bison; do
    command -v "$tool" >/dev/null || die "missing tool: $tool"
done
for tool in emcc emcmake; do
    command -v "$tool" >/dev/null || die "missing tool: $tool of the Emscripten SDK $EMSDK_VERSION: \
install it with emsdk (https://github.com/emscripten-core/emsdk: ./emsdk install $EMSDK_VERSION \
&& ./emsdk activate $EMSDK_VERSION) and put its upstream/emscripten directory on PATH"
done
emcc_version=$(emcc --version 2>/dev/null | head -n 1)
[[ $emcc_version == *" $EMSDK_VERSION ("* ]] \
    || die "the Emscripten SDK on PATH is not $EMSDK_VERSION: $emcc_version"

rm -rf "$work"
mkdir -p "$work"
curl -fsSL --retry 3 -o "$work/souffle.tar.gz" "$URL"
echo "$SHA256  $work/souffle.tar.gz" | sha256sum -c --quiet - \
    || die "checksum mismatch for $URL"
tar -xzf "$work/souffle.tar.gz" -C "$work"
patch -d "$work/souffle-$VERSION" -p1 --quiet <"$patch_file" \
    || die "cannot apply $patch_file"

# The module: an ES module factory (souffle.js) for workers and Node, a fresh
# instance per call, which runs main on its arguments and reports its exit
# status (EXIT_RUNTIME); its memory grows up to the memory limit of an
# evaluation, 1 GiB, past which it aborts with "OOM" (ABORTING_MALLOC), with a
# stack of 8 MiB, as Linux gives a process.
flags=(
    -sEXPORT_ES6=1
    -sENVIRONMENT=worker,node
    -sEXIT_RUNTIME=1
    -sALLOW_MEMORY_GROWTH=1
    -sABORTING_MALLOC=1
    -sINITIAL_MEMORY=33554432
    -sMAXIMUM_MEMORY=1073741824
    -sSTACK_SIZE=8388608
    -sSTACK_OVERFLOW_CHECK=1
)
generator=()
command -v ninja >/dev/null && generator=(-G Ninja)
emcmake cmake -S "$work/souffle-$VERSION" -B "$work/build" "${generator[@]}" \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_EXE_LINKER_FLAGS="${flags[*]}" \
    -DCMAKE_CROSSCOMPILING_EMULATOR="$(command -v node)" \
    -DPACKAGE_VERSION="$VERSION" \
    -DSOUFFLE_GIT=OFF \
    -DSOUFFLE_USE_OPENMP=OFF \
    -DSOUFFLE_USE_LIBFFI=OFF \
    -DSOUFFLE_USE_SQLITE=OFF \
    -DSOUFFLE_USE_ZLIB=OFF \
    -DSOUFFLE_USE_CURSES=OFF \
    -DSOUFFLE_BASH_COMPLETION=OFF \
    -DSOUFFLE_ENABLE_TESTING=OFF
cmake --build "$work/build" --target souffle --parallel "${JOBS:-$(nproc)}"

built="$work/build/src"
smoke "$built"
cp "$work/souffle-$VERSION/LICENSE" "$out/LICENSE"
cp "$built/souffle.wasm" "$out/souffle.wasm.tmp"
cp "$built/souffle.js" "$out/souffle.js.tmp"
mv "$out/souffle.wasm.tmp" "$out/souffle.wasm"
mv "$out/souffle.js.tmp" "$out/souffle.js"
rm -rf "$work"
echo "built $out/souffle.wasm"
