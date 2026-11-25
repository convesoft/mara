#!/usr/bin/env bash
# Prepare of release.yml (DES-RELEASE-WORKFLOW in docs/distribution.mara.md), run at the root of
# the checked-out commit:
#
#   release-prepare.sh
#
# Environment: DIST_TAG, the dispatch's input as given; DRY_RUN, `true` or `false`; REF, the
# dispatched ref; GITHUB_OUTPUT. Refuses what Prepare refuses, then writes `sha`, `version`, `tag`,
# `dist_tag` and `packages`, the JSON list of {name, native, tarball} of `releasePackages` in
# packages/mara-npm/scripts/packages.mjs, native packages first. npm's registry is read through
# npm-registry.sh, and `origin` for the tag.
set -euo pipefail

here=$(dirname "$(readlink -f "${BASH_SOURCE[0]}")")

dist_tag=$DIST_TAG
sha=$(git rev-parse HEAD)
version=$(cargo metadata --no-deps --format-version 1 --offline --locked |
  jq -r '.packages[] | select(.name == "mara") | .version')
echo "commit $sha, version $version, dist-tag \"$dist_tag\", dry run $DRY_RUN"

semver='^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-(0|[1-9][0-9]*|[0-9]*[A-Za-z-][0-9A-Za-z-]*)(\.(0|[1-9][0-9]*|[0-9]*[A-Za-z-][0-9A-Za-z-]*))*)?$'
if ! [[ $version =~ $semver ]]; then
  echo "::error::the version \"$version\" is not SemVer without build metadata"
  exit 1
fi
if ! [[ $dist_tag =~ ^[a-z][a-z0-9._-]*$ ]]; then
  echo "::error::the dist-tag \"$dist_tag\" does not match ^[a-z][a-z0-9._-]*\$"
  exit 1
fi
if [[ $version == *-* ]] && [ "$dist_tag" = latest ]; then
  echo "::error::$version is a prerelease, which is never published under latest"
  exit 1
fi
if [ "$DRY_RUN" = false ] && [ "$REF" != refs/heads/main ]; then
  echo "::error::a publication runs only from refs/heads/main, not $REF"
  exit 1
fi

packages=$(node --input-type=module -e '
  const [table, version] = process.argv.slice(1);
  const { releasePackages } = await import(table);
  console.log(JSON.stringify(releasePackages(version)));
' "$here/../../packages/mara-npm/scripts/packages.mjs" "$version")
jq . <<<"$packages"

tag="v$version"
if [ -n "$(git ls-remote --tags origin "refs/tags/$tag")" ]; then
  echo "::error::the tag $tag exists; set a new version in Cargo.toml first"
  exit 1
fi
published=()
mapfile -t names < <(jq -r '.[].name' <<<"$packages")
for name in "${names[@]}"; do
  shasum=$("$here/npm-registry.sh" shasum "$name" "$version")
  if [ -n "$shasum" ]; then
    published+=("$name")
  fi
done
if [ "${#published[@]}" -gt 0 ]; then
  echo "::error::npm has $version of ${published[*]}; set a new version in Cargo.toml first"
  exit 1
fi

{
  echo "sha=$sha"
  echo "version=$version"
  echo "tag=$tag"
  echo "dist_tag=$dist_tag"
  echo "packages=$(jq -c . <<<"$packages")"
} >>"$GITHUB_OUTPUT"
