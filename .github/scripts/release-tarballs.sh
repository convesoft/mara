#!/usr/bin/env bash
# The verified tarballs of release.yml (DES-RELEASE-WORKFLOW in docs/distribution.mara.md):
#
#   release-tarballs.sh record <directory> <out> <label>
#       Build, after its install verification: <directory> must hold, as `*.tgz`, the tarball of
#       every package for any platform and of exactly one native package. Writes their SHA-256 to
#       <out>/verified-<label>.sha256 and copies the native tarball to <out>, a new directory.
#   release-tarballs.sh collect <directory>
#       .github/actions/release-tarballs, on every Build job's artifact downloaded into
#       <directory>: fails unless it holds exactly one tarball per package, each with the SHA-256
#       that a Build job recorded for it, and the records, and no other visible file. Then writes
#       SHA256SUMS in the order of the packages, removes the records and writes
#       `directory=<directory>` to GITHUB_OUTPUT.
#
# PACKAGES is Prepare's JSON list of {name, native, tarball}.
set -euo pipefail

record() {
  local directory=$1 out=$2 label=$3 tarball list
  local -a any natives tarballs found=() unexpected=()
  list=$(jq -r '.[] | select(.native | not) | .tarball' <<<"$PACKAGES")
  mapfile -t any <<<"$list"
  list=$(jq -r '.[] | select(.native) | .tarball' <<<"$PACKAGES")
  mapfile -t natives <<<"$list"
  case $out in
    /*) ;;
    *) out="$PWD/$out" ;;
  esac
  cd "$directory"
  shopt -s nullglob
  tarballs=(*.tgz)
  for tarball in "${tarballs[@]}"; do
    if printf '%s\n' "${natives[@]}" | grep -Fxq -- "$tarball"; then
      found+=("$tarball")
    elif ! printf '%s\n' "${any[@]}" | grep -Fxq -- "$tarball"; then
      unexpected+=("$tarball")
    fi
  done
  for tarball in "${any[@]}"; do
    if [ ! -f "$tarball" ]; then
      unexpected+=("no $tarball")
    fi
  done
  if [ "${#found[@]}" -ne 1 ] || [ "${#unexpected[@]}" -gt 0 ]; then
    echo "::error::expected the tarballs of the packages for any platform and of one native package in $directory: ${tarballs[*]}"
    exit 1
  fi
  mkdir "$out"
  sha256sum "${tarballs[@]}" | tee "$out/verified-$label.sha256"
  cp "${found[0]}" "$out/"
}

collect() {
  local directory=$1 record line sum name tarball
  local -a records expected
  local -A verified=()
  cd "$directory"
  shopt -s nullglob
  records=(verified-*.sha256)
  if [ "${#records[@]}" -eq 0 ]; then
    echo "::error::no Build job recorded the tarballs that its install verification ran on"
    exit 1
  fi
  # Each Build job recorded its native tarball and those for any platform. Two checksums for one
  # name would mean that the jobs verified different files under it. A line of sha256sum is 64
  # hexadecimal digits, a space, a space or `*`, and the name.
  for record in "${records[@]}"; do
    while IFS= read -r line; do
      if ! [[ $line =~ ^([0-9a-f]{64})\ [\ *](.+)$ ]]; then
        echo "::error::$record has a line that is not sha256sum's: $line"
        exit 1
      fi
      sum=${BASH_REMATCH[1]}
      name=${BASH_REMATCH[2]}
      if [ -n "${verified[$name]:-}" ] && [ "${verified[$name]}" != "$sum" ]; then
        echo "::error::the Build jobs verified different tarballs named $name"
        exit 1
      fi
      verified[$name]=$sum
    done <"$record"
  done
  # Tag attaches every visible file of the directory, so it may hold nothing else.
  for name in *; do
    case $name in
      *.tgz | verified-*.sha256) ;;
      *)
        echo "::error::$directory holds $name, which is neither a tarball nor a Build job's record"
        exit 1
        ;;
    esac
  done
  local list sorted recorded present
  list=$(jq -r '.[].tarball' <<<"$PACKAGES")
  mapfile -t expected <<<"$list"
  sorted=$(printf '%s\n' "${expected[@]}" | sort)
  recorded=$(printf '%s\n' "${!verified[@]}" | sort)
  present=$(printf '%s\n' *.tgz | sort)
  if [ "$recorded" != "$sorted" ] || [ "$present" != "$sorted" ]; then
    echo "::error::expected exactly the tarballs of every package, each verified by a Build job"
    echo "expected:"
    echo "$sorted"
    echo "verified:"
    echo "$recorded"
    echo "downloaded:"
    echo "$present"
    exit 1
  fi
  # SHA256SUMS in the order of the packages, which the release attaches.
  for tarball in "${expected[@]}"; do
    echo "${verified[$tarball]}  $tarball"
  done >SHA256SUMS
  sha256sum --check --strict SHA256SUMS
  rm -- "${records[@]}"
  ls -l
  echo "directory=$PWD" >>"$GITHUB_OUTPUT"
}

case "${1:-} $#" in
  "record 4") record "$2" "$3" "$4" ;;
  "collect 2") collect "$2" ;;
  *)
    echo "usage: release-tarballs.sh record <directory> <out> <label> | collect <directory>" >&2
    exit 2
    ;;
esac
