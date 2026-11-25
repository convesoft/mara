#!/usr/bin/env bash
# The publication of release.yml's Publish job (DES-RELEASE-WORKFLOW in
# docs/distribution.mara.md):
#
#   release-publish.sh <directory> [<npm publish option>...]
#
# Requires npm 11.5.1 or later, the first with Trusted Publishing. Publishes the tarballs in
# <directory> in the order of PACKAGES, Prepare's JSON list of {name, native, tarball}: each native
# package, then, once npm shows every native package with its tarball's SHA-1, each package for
# any platform, the dispatcher last. Each is published with `npm publish <tarball> <options>
# --access public --tag $DIST_TAG` from RUNNER_TEMP, so that no project .npmrc applies.
#
# A re-run continues a partial publication: a version that npm has with the tarball's SHA-1 is
# skipped, one with another fails, and only a version that npm reports absent is published. The
# wait polls every RELEASE_POLL_SECONDS (30) for at most RELEASE_WAIT_SECONDS (1800); a registry
# error or another SHA-1 fails it at once. Environment: PACKAGES, VERSION, DIST_TAG, RUNNER_TEMP;
# NPM_REGISTRY, for tests, names the registry that npm-registry.sh reads and `npm publish` is given.
set -euo pipefail

script=$(readlink -f "${BASH_SOURCE[0]}")
here=$(dirname "$script")
registry="$here/npm-registry.sh"
directory=$(readlink -f "$1")
shift
options=("$@")
poll=${RELEASE_POLL_SECONDS:-30}
wait=${RELEASE_WAIT_SECONDS:-1800}

npm_version=$(npm --version)
# `sed -n 1p` reads all of its input, so that `sort` never meets a closed pipe under pipefail.
lowest=$(printf '%s\n' 11.5.1 "$npm_version" | sort -V | sed -n 1p)
if [ "$lowest" != 11.5.1 ]; then
  echo "::error::npm $npm_version is older than 11.5.1, the first with Trusted Publishing"
  exit 1
fi
echo "npm $npm_version"

publish() {
  local name=$1 tarball="$directory/$2" expected published
  expected=$(sha1sum "$tarball" | cut -d ' ' -f 1)
  published=$("$registry" shasum "$name" "$VERSION")
  if [ -z "$published" ]; then
    (cd "$RUNNER_TEMP" && npm publish "$tarball" "${options[@]}" --access public --tag "$DIST_TAG" \
      ${NPM_REGISTRY:+--registry "$NPM_REGISTRY"})
  elif [ "$published" = "$expected" ]; then
    echo "npm has $name@$VERSION with this tarball's SHA-1 $expected; skipped"
  else
    echo "::error::npm has $name@$VERSION with the SHA-1 $published, not this tarball's $expected"
    exit 1
  fi
}

list=$(jq -r '.[] | select(.native) | "\(.name) \(.tarball)"' <<<"$PACKAGES")
natives=()
[ -z "$list" ] || mapfile -t natives <<<"$list"
list=$(jq -r '.[] | select(.native | not) | "\(.name) \(.tarball)"' <<<"$PACKAGES")
others=()
[ -z "$list" ] || mapfile -t others <<<"$list"
for entry in "${natives[@]}"; do
  read -r name tarball <<<"$entry"
  publish "$name" "$tarball"
done

deadline=$((SECONDS + wait))
for entry in "${natives[@]}"; do
  read -r name tarball <<<"$entry"
  expected=$(sha1sum "$directory/$tarball" | cut -d ' ' -f 1)
  while :; do
    published=$("$registry" shasum "$name" "$VERSION")
    if [ "$published" = "$expected" ]; then
      echo "npm shows $name@$VERSION"
      break
    fi
    if [ -n "$published" ]; then
      echo "::error::npm shows $name@$VERSION with the SHA-1 $published, not this tarball's $expected"
      exit 1
    fi
    if [ "$SECONDS" -ge "$deadline" ]; then
      echo "::error::npm did not show $name@$VERSION within $wait seconds"
      exit 1
    fi
    echo "npm does not show $name@$VERSION yet"
    sleep "$poll"
  done
done

for entry in "${others[@]}"; do
  read -r name tarball <<<"$entry"
  publish "$name" "$tarball"
done
