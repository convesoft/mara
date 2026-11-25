#!/usr/bin/env bash
# The npm part of release.yml's Dry run (DES-RELEASE-WORKFLOW in docs/distribution.mara.md):
#
#   release-dry-run.sh <directory>
#
# Runs `npm publish --dry-run --access public --tag $DIST_TAG` on each tarball in <directory>, in
# the order of PACKAGES, Prepare's JSON list of {name, native, tarball}, native packages first.
# npm refuses even a dry run of a version it has, which a version published after Prepare's check
# meets: the registry is read right before each package's dry run, and again when npm refuses it,
# and a version that npm has by then is left out with a warning. Any other refusal fails.
# Environment: PACKAGES, VERSION, DIST_TAG; NPM_REGISTRY, for tests, names the registry that
# npm-registry.sh reads and `npm publish` is given.
set -euo pipefail

here=$(dirname "$(readlink -f "${BASH_SOURCE[0]}")")
registry="$here/npm-registry.sh"

cd "$1"
npm --version
mapfile -t entries < <(jq -r '.[] | "\(.name) \(.tarball)"' <<<"$PACKAGES")
for entry in "${entries[@]}"; do
  read -r name tarball <<<"$entry"
  published=$("$registry" shasum "$name" "$VERSION")
  if [ -z "$published" ]; then
    if npm publish "./$tarball" --dry-run --access public --tag "$DIST_TAG" \
      ${NPM_REGISTRY:+--registry "$NPM_REGISTRY"} </dev/null; then
      continue
    fi
    published=$("$registry" shasum "$name" "$VERSION")
    if [ -z "$published" ]; then
      echo "::error::npm publish --dry-run of $name failed, and npm has no $VERSION of it"
      exit 1
    fi
  fi
  echo "::warning::npm has $name@$VERSION, so its dry run is left out"
done
