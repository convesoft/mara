#!/usr/bin/env bash
# The dist-tags baseline of release.yml (DES-RELEASE-WORKFLOW in docs/distribution.mara.md):
#
#   release-dist-tags.sh record <file>
#       Publish, before its first publication: keeps <file> when an earlier attempt of the run
#       recorded it, else writes every package's dist-tags to it as one JSON object by package
#       name and writes `recorded=true` to GITHUB_OUTPUT, so that the job uploads it.
#   release-dist-tags.sh check <file>
#       Registry check, after Publish: for each package, waits until $DIST_TAG names $VERSION,
#       polling every RELEASE_POLL_SECONDS (30) for at most RELEASE_WAIT_SECONDS (1800), then
#       fails unless every other dist-tag names what <file> recorded, compared as JSON values. A
#       registry error fails at once.
#
# Environment: PACKAGES, Prepare's JSON list of {name, native, tarball}; VERSION and DIST_TAG for
# `check`; NPM_REGISTRY, for tests, as npm-registry.sh reads it.
set -euo pipefail

here=$(dirname "$(readlink -f "${BASH_SOURCE[0]}")")
registry="$here/npm-registry.sh"

record() {
  local file=$1 tags='{}' name value
  if [ -f "$file" ]; then
    echo "an earlier attempt of this run recorded:"
    jq . "$file"
    return
  fi
  mapfile -t names < <(jq -r '.[].name' <<<"$PACKAGES")
  for name in "${names[@]}"; do
    value=$("$registry" dist-tags "$name")
    tags=$(jq -c --arg name "$name" --argjson value "$value" '.[$name] = $value' <<<"$tags")
  done
  mkdir -p "$(dirname "$file")"
  jq . <<<"$tags" | tee "$file"
  echo "recorded=true" >>"$GITHUB_OUTPUT"
}

check() {
  local before_all name tags before after deadline
  before_all=$(cat "$1")
  deadline=$((SECONDS + ${RELEASE_WAIT_SECONDS:-1800}))
  mapfile -t names < <(jq -r '.[].name' <<<"$PACKAGES")
  for name in "${names[@]}"; do
    while :; do
      tags=$("$registry" dist-tags "$name")
      if [ "$(jq -r --arg tag "$DIST_TAG" '.[$tag] // ""' <<<"$tags")" = "$VERSION" ]; then
        break
      fi
      if [ "$SECONDS" -ge "$deadline" ]; then
        echo "::error::$DIST_TAG of $name does not name $VERSION within ${RELEASE_WAIT_SECONDS:-1800} seconds: $tags"
        exit 1
      fi
      sleep "${RELEASE_POLL_SECONDS:-30}"
    done
    before=$(jq -c --arg name "$name" --arg tag "$DIST_TAG" '.[$name] | del(.[$tag])' <<<"$before_all")
    after=$(jq -c --arg tag "$DIST_TAG" 'del(.[$tag])' <<<"$tags")
    if ! jq -e -n --argjson before "$before" --argjson after "$after" '$before == $after' >/dev/null; then
      echo "::error::the other dist-tags of $name changed: $before before, $after after"
      exit 1
    fi
    echo "$name: $tags"
  done
}

case "${1:-} $#" in
  "record 2") record "$2" ;;
  "check 2") check "$2" ;;
  *)
    echo "usage: release-dist-tags.sh record <file> | check <file>" >&2
    exit 2
    ;;
esac
