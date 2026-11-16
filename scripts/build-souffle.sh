#!/usr/bin/env bash
# Builds the pinned Soufflé natively, with a static C++ runtime and without
# OpenMP, as DES-SOUFFLE-BUILD in docs/distribution.mara.md specifies.
# Output: target/souffle-<version>/souffle in the repository. Needs curl, tar,
# sha256sum, cmake, a C++17 compiler, flex, bison and readelf; ninja if present.
set -euo pipefail

VERSION=2.5
URL="https://github.com/souffle-lang/souffle/archive/refs/tags/${VERSION}.tar.gz"
SHA256=5d009ad6c74ccec10207d865c059716afac625759bff7c8070e529bd80385067

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
out="$root/target/souffle-$VERSION"
bin="$out/souffle"
work="$out/work"

die() { echo "build-souffle: $*" >&2; exit 1; }

for tool in curl tar sha256sum cmake c++ flex bison readelf; do
    command -v "$tool" >/dev/null || die "missing tool: $tool"
done

# The static C++ runtime and the absence of OpenMP show in the dynamic section.
check_linkage() {
    local needed
    needed=$(readelf -d "$1" | grep NEEDED || true)
    if grep -E 'libstdc\+\+|libc\+\+|libgcc_s|libgomp|libomp' <<<"$needed"; then
        die "$1 links a shared C++ runtime or OpenMP"
    fi
}

# Mara's invocation, with no compiler, mcpp or libfunctors reachable, on JSON
# facts and outputs (DES-RULE-EXECUTION in docs/rules/evaluation.mara.md).
smoke() {
    local dir
    mkdir -p "$out"
    dir=$(mktemp -d "${TMPDIR:-$out}/souffle-smoke.XXXXXX")
    cat >"$dir/wrapper.dl" <<'EOF'
.decl edge(a: symbol, b: symbol)
.decl path(a: symbol, b: symbol)
path(x, y) :- edge(x, y).
path(x, z) :- path(x, y), edge(y, z).
.input edge(IO=jsonfile, format=list, filename="edge.json")
.output path(IO=jsonfile, format=list, filename="path.json")
EOF
    printf '[["a","b\\tc"],["b\\tc","d"]]' >"$dir/edge.json"
    local status=0 got
    (cd "$dir" && env -i PATH=/nonexistent "$1" --no-preprocessor -l "" wrapper.dl) || status=$?
    got=$(tr -d ' \n' <"$dir/path.json" 2>/dev/null || true)
    rm -rf "$dir"
    [[ $status == 0 ]] || die "smoke run of $1 exited with $status"
    [[ $got == '[["a","b\tc"],["a","d"],["b\tc","d"]]' ]] \
        || die "smoke run of $1 wrote unexpected output: $got"
}

if [[ -x $bin ]]; then
    check_linkage "$bin"
    smoke "$bin"
    echo "$bin is built and passes the smoke run"
    exit 0
fi

rm -rf "$work"
mkdir -p "$work"
curl -fsSL --retry 3 -o "$work/souffle.tar.gz" "$URL"
echo "$SHA256  $work/souffle.tar.gz" | sha256sum -c --quiet - \
    || die "checksum mismatch for $URL"
tar -xzf "$work/souffle.tar.gz" -C "$work"

generator=()
command -v ninja >/dev/null && generator=(-G Ninja)
cmake -S "$work/souffle-$VERSION" -B "$work/build" "${generator[@]}" \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_EXE_LINKER_FLAGS="-static-libstdc++ -static-libgcc -s" \
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

check_linkage "$work/build/src/souffle"
smoke "$work/build/src/souffle"
cp "$work/souffle-$VERSION/LICENSE" "$out/LICENSE"
mv "$work/build/src/souffle" "$bin.tmp"
mv "$bin.tmp" "$bin"
rm -rf "$work"
echo "built $bin"
