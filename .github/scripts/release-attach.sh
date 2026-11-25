#!/usr/bin/env bash
# The assets of the GitHub release in release.yml's Tag job (DES-RELEASE-WORKFLOW in
# docs/distribution.mara.md):
#
#   release-attach.sh <directory>
#
# Attaches each file of <directory>, the tarballs and SHA256SUMS, to the release of $TAG, which
# must exist: the release is looked up by the API's status, and 404 is no release of the tag. Each
# file is attached unless the release holds an asset of its name in the state `uploaded`, whose
# SHA-256 (its `digest`, else its download) must equal the file's; an asset in another state is an
# interrupted upload, which is deleted and attached again. Environment: GH_TOKEN, GH_REPO,
# GITHUB_API_URL, TAG, RUNNER_TEMP.
set -euo pipefail

release="$RUNNER_TEMP/release.json"
status=$(curl -sS --connect-timeout 10 --max-time 60 -o "$release" -w '%{http_code}' \
  -H "Authorization: Bearer $GH_TOKEN" -H 'Accept: application/vnd.github+json' \
  -H 'X-GitHub-Api-Version: 2022-11-28' \
  "$GITHUB_API_URL/repos/$GH_REPO/releases/tags/$TAG")
if [ "$status" != 200 ]; then
  echo "::error::GitHub answered $status for the release of $TAG"
  exit 1
fi
download=$(mktemp -d "$RUNNER_TEMP/attached.XXXXXX")
cd "$1"
for file in *; do
  asset=$(jq -c --arg name "$file" '[.assets[] | select(.name == $name)] | first // empty' "$release")
  state=$(jq -r '.state // ""' <<<"${asset:-null}")
  if [ -n "$asset" ] && [ "$state" != uploaded ]; then
    echo "the release's $file is in the state $state; deleting it"
    gh api -X DELETE "repos/$GH_REPO/releases/assets/$(jq -r .id <<<"$asset")"
    asset=
  fi
  if [ -z "$asset" ]; then
    gh release upload "$TAG" "$file"
    continue
  fi
  expected="sha256:$(sha256sum "$file" | cut -d ' ' -f 1)"
  digest=$(jq -r '.digest // ""' <<<"$asset")
  if [ -z "$digest" ]; then
    gh release download "$TAG" --pattern "$file" --dir "$download"
    digest="sha256:$(sha256sum "$download/$file" | cut -d ' ' -f 1)"
  fi
  if [ "$digest" != "$expected" ]; then
    echo "::error::the release's $file has $digest, not this run's $expected"
    exit 1
  fi
  echo "the release has this run's $file"
done
