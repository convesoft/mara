#!/usr/bin/env bash
# Reads npm's public registry for release.yml (DES-RELEASE-WORKFLOW in docs/distribution.mara.md):
#
#   npm-registry.sh shasum <package> <version>   the version's `dist.shasum`, or nothing when npm
#                                                has no such version or no such package
#   npm-registry.sh dist-tags <package>          the package's dist-tags as a JSON object, `{}`
#                                                when npm has no such package
#
# A package is absent only when the registry answers 404 with its "Not found" document, and a
# version only when the package's document lacks it. Any other answer, a present version without
# a SHA-1 included, is retried 5 times 10 s apart and then fails with exit status 1, never read
# as absent. Each request stops after 30 s. For tests, NPM_REGISTRY names another registry and
# NPM_REGISTRY_RETRY_SECONDS another pause between tries.
set -euo pipefail

registry=${NPM_REGISTRY:-https://registry.npmjs.org}
pause=${NPM_REGISTRY_RETRY_SECONDS:-10}

# Fetches the package's abbreviated document once and prints the answer for the command, or fails
# with a reason on stderr.
answer() {
  local command=$1 package=$2 version=${3:-} body status
  body=$(mktemp)
  trap 'rm -f "$body"' RETURN
  status=$(curl -sS --connect-timeout 10 --max-time 30 -o "$body" -w '%{http_code}' \
    -H 'Accept: application/vnd.npm.install-v1+json' -H 'Cache-Control: no-cache' \
    "$registry/${package/\//%2f}") || {
    echo "no answer from $registry" >&2
    return 1
  }
  case "$status" in
    200)
      if ! jq -e --arg name "$package" \
        'type == "object" and .name == $name and (.versions | type == "object")
          and ((."dist-tags" // {}) | type == "object")' "$body" >/dev/null 2>&1; then
        echo "HTTP 200 without the document of $package" >&2
        return 1
      fi
      if [ "$command" = dist-tags ]; then
        jq -c '."dist-tags" // {}' "$body"
      elif jq -e --arg version "$version" '.versions | has($version) | not' "$body" >/dev/null; then
        : # The version is absent.
      else
        jq -er --arg version "$version" \
          '.versions[$version].dist.shasum | select(type == "string" and test("^[0-9a-f]{40}$"))' \
          "$body" || {
          echo "$package@$version is listed without a SHA-1 in dist.shasum" >&2
          return 1
        }
      fi
      ;;
    404)
      if ! jq -e '. == {"error": "Not found"}' "$body" >/dev/null 2>&1; then
        echo "HTTP 404 without npm's \"Not found\" document" >&2
        return 1
      fi
      if [ "$command" = dist-tags ]; then
        echo '{}'
      fi
      ;;
    *)
      echo "HTTP $status" >&2
      return 1
      ;;
  esac
}

case "${1:-} $#" in
  "shasum 3" | "dist-tags 2") ;;
  *)
    echo "usage: npm-registry.sh shasum <package> <version> | dist-tags <package>" >&2
    exit 2
    ;;
esac
for try in 1 2 3 4 5; do
  if output=$(answer "$@"); then
    if [ -n "$output" ]; then
      echo "$output"
    fi
    exit 0
  fi
  echo "npm's registry gave no usable answer for $2 (try $try of 5)" >&2
  if [ "$try" -lt 5 ]; then
    sleep "$pause"
  fi
done
exit 1
