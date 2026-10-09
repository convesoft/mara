#!/usr/bin/env bash
# Test selection belongs to cargo-affected/nextest, not this script.
set -euo pipefail
usage() {
  echo 'Usage: scripts/verify.sh {impacted|selected FILTER|refresh|refresh-diff|full|check EVIDENCE_DIR}' >&2
  exit 2
}
[[ $# -gt 0 ]] || usage
mode=$1; shift
case "$mode" in
  selected|check) [[ $# == 1 ]] || usage ;;
  impacted|refresh|refresh-diff|full) [[ $# == 0 ]] || usage ;;
  *) usage ;;
esac
cd "$(git rev-parse --show-toplevel)"
root=$PWD
: "${CARGO_TARGET_DIR:?Set an absolute reusable build directory}"
: "${TMPDIR:?Set an absolute disposable fixture directory}"
: "${MARA_TEST_RUNTIME_DIR:?Set a short private IPC directory}"
for directory in "$CARGO_TARGET_DIR" "$TMPDIR" "$MARA_TEST_RUNTIME_DIR"; do
  [[ "$directory" == /* && -d "$directory" ]] || { echo "Missing absolute directory: $directory" >&2; exit 2; }
done
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}"

# Hash environment values, not secrets. Storage paths are recorded separately:
# moving disposable storage alone is not a change to the test environment.
context() {
  printf 'checkout=%s\n' "$root"
  uname -sm
  rustc -vV
  cargo --version
  git --version
  for tool in cargo-nextest cargo-affected; do
    if command -v "$tool" >/dev/null; then
      "$tool" --version
      printf '%s=' "$tool"
      git hash-object -- "$(command -v "$tool")"
    else
      printf '%s=absent\n' "$tool"
    fi
  done
  for name in $(compgen -e | LC_ALL=C sort); do
    case "$name" in
      CARGO_TARGET_DIR|CARGO_BUILD_JOBS|MARA_TEST_RUNTIME_DIR) continue ;;
      CARGO_*|RUST*|NEXTEST_*|MARA_*|NODE*|LLVM_*|PATH|LANG|LC_*|TZ|HOME)
        printf '%s=' "$name"
        printf '%s' "${!name}" | git hash-object --stdin ;;
    esac
  done
  # Upstream misses build.rs, Cargo/nextest config and toolchain-file changes.
  # Include ancestor and user Cargo configuration, including absent files.
  directory=$root
  while :; do
    for file in "$directory/.cargo/config" "$directory/.cargo/config.toml"; do
      printf '%s=' "$file"
      if [[ -f "$file" ]]; then git hash-object -- "$file"; else echo absent; fi
    done
    [[ "$directory" != / ]] || break
    directory=$(dirname "$directory")
  done
  for file in "${CARGO_HOME:-$HOME/.cargo}/config" "${CARGO_HOME:-$HOME/.cargo}/config.toml"; do
    printf '%s=' "$file"
    if [[ -f "$file" ]]; then git hash-object -- "$file"; else echo absent; fi
  done
  git ls-files --cached --others --exclude-standard -z -- \
    '**/build.rs' build.rs '**/Cargo.toml' Cargo.toml Cargo.lock \
    rust-toolchain rust-toolchain.toml .config/nextest.toml scripts/verify.sh |
    file_hashes
}
# Filesystem snapshot includes untracked inputs and explicit deleted-file markers.
# Git's NUL-delimited traversal keeps spaces/newlines in file names unambiguous.
file_hashes() {
  while IFS= read -r -d '' file; do
    printf '%s\0' "$file"
    if [[ -e "$file" || -L "$file" ]]; then git hash-object -- "$file"; else echo deleted; fi
  done
}
inputs() { git ls-files --cached --others --exclude-standard -z | file_hashes; }

if [[ "$mode" == check ]]; then
  receipt=$1
  [[ -f "$receipt/passed" && -f "$receipt/exit-code" && "$(< "$receipt/exit-code")" == 0 &&
     -f "$receipt/inputs" && -f "$receipt/context" ]] || {
    echo 'No completed passing receipt' >&2; exit 1;
  }
  # cmp process substitutions would hide snapshot command failures.
  current=$(mktemp -d "$TMPDIR/verification-check.XXXXXX")
  trap 'rm -rf -- "$current"' EXIT
  inputs > "$current/inputs"
  context > "$current/context"
  cmp "$receipt/inputs" "$current/inputs" && cmp "$receipt/context" "$current/context" || {
    echo 'STALE: source/configuration or execution environment differs' >&2; exit 1;
  }
  echo "Reusable passing evidence: $receipt (scope and original revision remain unchanged)"
  exit 0
fi

: "${VERIFY_EVIDENCE_DIR:?Set a retained evidence directory outside the checkout}"
[[ "$VERIFY_EVIDENCE_DIR" == /* ]] || { echo 'Evidence directory must be absolute' >&2; exit 2; }
case "$VERIFY_EVIDENCE_DIR/" in "$root/"*) echo 'Evidence must be outside the checkout' >&2; exit 2 ;; esac
mkdir -p "$VERIFY_EVIDENCE_DIR"
evidence=$(mktemp -d "$VERIFY_EVIDENCE_DIR/$mode.XXXXXX")
echo "Verification evidence: $evidence"
printf 'mode=%s\nrevision=%s\ncheckout=%s\ntarget=%s\ntmp=%s\nipc=%s\n' \
  "$mode" "$(git rev-parse HEAD)" "$root" "$CARGO_TARGET_DIR" "$TMPDIR" "$MARA_TEST_RUNTIME_DIR" > "$evidence/identity"
git status --porcelain=v1 > "$evidence/status"
inputs > "$evidence/inputs"
context > "$evidence/context"
links=""
finish() {
  rc=$?
  trap - EXIT
  # Names are fixed below, not user paths; remove only links created here.
  for directory in $links; do
    if [[ -L "target/$directory" && "$(readlink "target/$directory")" == "$CARGO_TARGET_DIR/$directory" ]]; then
      rm -- "target/$directory"
    else
      echo 'Verification storage link changed during execution' >&2; rc=1
    fi
  done
  if [[ "$rc" != 0 ]]; then rm -f -- "$evidence/passed"; fi
  printf '%s\n' "$rc" > "$evidence/exit-code"
  echo "Verification exit=$rc; evidence=$evidence"
  exit "$rc"
}
trap finish EXIT
run() {
  printf '%q ' "$@" >> "$evidence/commands"
  printf '\n' >> "$evidence/commands"
  "$@" 2>&1 | tee -a "$evidence/output.log"
}
capture() {
  destination=$1; shift
  printf '%q ' "$@" >> "$evidence/commands"
  printf '\n' >> "$evidence/commands"
  rc=0
  "$@" > "$destination" 2> "$destination.stderr" || rc=$?
  tee -a "$evidence/output.log" < "$destination.stderr" >&2
  return "$rc"
}
# Both tools keep metadata under workspace-root/target despite CARGO_TARGET_DIR.
# Route just their directories to caller-owned storage; never replace user data.
for directory in nextest affected; do
  if [[ "$directory" == affected && "$mode" != impacted && "$mode" != refresh* ]]; then continue; fi
  mkdir -p "$CARGO_TARGET_DIR/$directory" target
  if [[ ! -e "target/$directory" && ! -L "target/$directory" ]]; then
    ln -s "$CARGO_TARGET_DIR/$directory" "target/$directory"
    links="$links $directory"
  fi
  [[ "$(cd "target/$directory" && pwd -P)" == "$(cd "$CARGO_TARGET_DIR/$directory" && pwd -P)" ]] || {
    echo "target/$directory already belongs to another cache; choose its CARGO_TARGET_DIR or a disposable checkout" >&2
    exit 2
  }
done
# Never attach a JUnit result left by an earlier run (including a zero selection).
rm -f -- "$CARGO_TARGET_DIR/nextest/default/junit.xml"
stamp="$CARGO_TARGET_DIR/affected/mara-context"
case "$mode" in
  impacted)
    impact=(cargo affected run)
    if [[ ! -f "$stamp" ]] || ! cmp -s "$stamp" "$evidence/context"; then
      echo 'Impact context missing or stale: conservative full selection' | tee "$evidence/fallback"
      impact+=(--all)
    fi
    run "${impact[@]}" --report-json "$evidence/selection.json" --report-detail full -- \
      --locked --workspace --exclude mara-wasm --exclude mara-node --all-targets
    if [[ -f "$CARGO_TARGET_DIR/nextest/default/junit.xml" ]]; then
      cp "$CARGO_TARGET_DIR/nextest/default/junit.xml" "$evidence/native-junit.xml"
      rm -- "$CARGO_TARGET_DIR/nextest/default/junit.xml"
    fi
    # Upstream cannot map cdylib/rlib test targets. Never silently omit them.
    run cargo nextest run --locked -p mara-wasm -p mara-node --all-targets --ignore-default-filter ;;
  selected)
    printf '%s\n' "$1" > "$evidence/filter"
    capture "$evidence/selection.json" cargo nextest list --locked --workspace --all-targets --ignore-default-filter -E "$1" --message-format json
    run cargo nextest run --locked --workspace --all-targets --ignore-default-filter -E "$1" ;;
  refresh|refresh-diff)
    collection=(cargo affected collect)
    if [[ "$mode" == refresh-diff ]]; then
      [[ -f "$stamp" ]] && cmp -s "$stamp" "$evidence/context" || {
        echo 'Impact context missing or stale: use refresh, not refresh-diff' >&2; exit 2;
      }
      collection+=(--diff)
    fi
    # Never use --allow-dirty: LLVM ranges must describe the committed baseline.
    rm -f -- "$stamp"
    run "${collection[@]}" -- \
      --locked --workspace --exclude mara-wasm --exclude mara-node --all-targets
    ;;
  full)
    run cargo fmt --all -- --check
    run cargo build --locked --workspace --all-targets -j "$CARGO_BUILD_JOBS"
    run cargo clippy --locked --workspace --all-targets --all-features -j "$CARGO_BUILD_JOBS" -- -D warnings
    capture "$evidence/selection.json" cargo nextest list --locked --workspace --all-targets --ignore-default-filter --message-format json
    run cargo nextest run --locked --workspace --all-targets --ignore-default-filter
    # Keep Cargo for doctests: nextest does not execute them.
    run cargo test --locked --workspace --doc -j "$CARGO_BUILD_JOBS"
    # Validation can exit zero with valid:false: check the structured result.
    for target in schema project; do
      capture "$evidence/$target.json" cargo run --locked --quiet -j "$CARGO_BUILD_JOBS" -- --format json "$target" validate
      grep -Eq '"valid"[[:space:]]*:[[:space:]]*true' "$evidence/$target.json"
      grep -Eq '"evaluation_complete"[[:space:]]*:[[:space:]]*true' "$evidence/$target.json"
    done ;;
esac
inputs > "$evidence/inputs-after"
context > "$evidence/context-after"
cmp "$evidence/inputs" "$evidence/inputs-after"
cmp "$evidence/context" "$evidence/context-after"
if [[ "$mode" == refresh* ]]; then cp "$evidence/context" "$stamp"; fi
# Copy JUnit separately; it is execution evidence, unlike selection.json.
if [[ -f "$CARGO_TARGET_DIR/nextest/default/junit.xml" && "$mode" != refresh* ]]; then
  cp "$CARGO_TARGET_DIR/nextest/default/junit.xml" "$evidence/junit.xml"
fi
touch "$evidence/passed"
