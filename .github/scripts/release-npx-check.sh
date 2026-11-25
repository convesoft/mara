#!/usr/bin/env bash
# The first step of release.yml's Registry check (DES-RELEASE-WORKFLOW in
# docs/distribution.mara.md):
#
#   release-npx-check.sh
#
# Runs `npx --yes --ignore-scripts @convesoft/mara@$VERSION --version` until it prints
# `mara $VERSION`, trying every RELEASE_POLL_SECONDS (30) for at most RELEASE_WAIT_SECONDS (1800).
# Each try has an empty directory and npm cache under RUNNER_TEMP, so that an installation that
# missed a native package which npm did not serve yet is not reused, and stops after 5 minutes.
# Environment: VERSION, RUNNER_TEMP; NPM_REGISTRY, for tests, names the registry that npx is given.
set -euo pipefail

deadline=$((SECONDS + ${RELEASE_WAIT_SECONDS:-1800}))
while :; do
  try=$(mktemp -d "$RUNNER_TEMP/npx.XXXXXX")
  if printed=$(cd "$try" && npm_config_cache="$try/.npm" \
    timeout 300 npx --yes --ignore-scripts ${NPM_REGISTRY:+--registry "$NPM_REGISTRY"} \
    "@convesoft/mara@$VERSION" --version) &&
    [ "$printed" = "mara $VERSION" ]; then
    echo "$printed"
    break
  fi
  echo "printed: ${printed:-nothing}"
  if [ "$SECONDS" -ge "$deadline" ]; then
    echo "::error::npx did not run mara $VERSION from the registry within ${RELEASE_WAIT_SECONDS:-1800} seconds"
    exit 1
  fi
  sleep "${RELEASE_POLL_SECONDS:-30}"
done
