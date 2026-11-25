#!/usr/bin/env bash
# The tag and GitHub release of release.yml's Tag job (DES-RELEASE-WORKFLOW in
# docs/distribution.mara.md), run at the root of a checkout of the recorded commit with every tag:
#
#   release-tag.sh
#
# `nx release changelog` tags HEAD, the recorded commit, pushes that tag alone (Nx puts
# `--follow-tags` before the given arguments, which `--no-follow-tags` overrides), and creates the
# GitHub release of the tag with its notes, a prerelease for a version with a prerelease part. It
# fails on a tag that exists and updates a release that exists, so a re-run makes only what is
# missing, after looking both up: an existing release and its notes are kept, and one without its
# tag on origin fails; an existing tag must name the recorded commit; without its release, Nx
# creates the release alone, after the local copy of the tag is deleted, so that the notes start at
# the release before, as they would have. Standard input is empty, so that a prompt of Nx after a
# failed API call fails. Environment: GH_TOKEN, GH_REPO, GITHUB_API_URL, SHA, VERSION, TAG.
set -euo pipefail

remote=$(git ls-remote origin "refs/tags/$TAG" "refs/tags/$TAG^{}")
status=$(curl -sS --connect-timeout 10 --max-time 60 -o /dev/null -w '%{http_code}' \
  -H "Authorization: Bearer $GH_TOKEN" -H 'Accept: application/vnd.github+json' \
  -H 'X-GitHub-Api-Version: 2022-11-28' \
  "$GITHUB_API_URL/repos/$GH_REPO/releases/tags/$TAG")
if [ "$status" != 200 ] && [ "$status" != 404 ]; then
  echo "::error::GitHub answered $status for the release of $TAG"
  exit 1
fi
if [ -z "$remote" ]; then
  if [ "$status" = 200 ]; then
    echo "::error::the release of $TAG exists, but origin has no tag $TAG; restore the tag on $SHA or delete the release, then re-run"
    exit 1
  fi
  git config user.name "github-actions[bot]"
  git config user.email "41898282+github-actions[bot]@users.noreply.github.com"
  pnpm nx release changelog "$VERSION" --verbose \
    --git-push-args="--no-follow-tags refs/tags/$TAG" </dev/null
  exit 0
fi
# The commit is the peeled line of an annotated tag, else the only line.
commit=$(awk -v peeled="refs/tags/$TAG^{}" '$2 == peeled { print $1 }' <<<"$remote")
commit=${commit:-$(awk '{ print $1; exit }' <<<"$remote")}
if [ "$commit" != "$SHA" ]; then
  echo "::error::the tag $TAG names $commit, not the recorded commit $SHA"
  exit 1
fi
echo "the tag $TAG names $SHA"
if [ "$status" = 200 ]; then
  echo "the release of $TAG exists; it and its notes are kept"
  exit 0
fi
if git rev-parse -q --verify "refs/tags/$TAG" >/dev/null; then
  git tag -d "$TAG"
fi
pnpm nx release changelog "$VERSION" --verbose --git-tag=false --git-push=false </dev/null
