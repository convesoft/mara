# Reads npm's public registry for release.yml (DES-RELEASE-WORKFLOW in docs/distribution.mara.md).
# Sourced by its steps, which run bash with -e and -o pipefail. Each function fails on any answer
# of the registry other than the package's document or "no such package", after 5 tries 10 s apart,
# so that an error is never read as an absent package or version.

# A failure inside a command substitution fails it, as everywhere else in the step.
shopt -s inherit_errexit

# Prints the abbreviated document of the package $1, which holds its dist-tags and each version's
# `dist.shasum`, or `{}` when npm has no package of that name.
registry_document() {
  local url="https://registry.npmjs.org/${1/\//%2f}" body status attempt
  body=$(mktemp)
  for attempt in 1 2 3 4 5; do
    status=$(curl -sS -o "$body" -w '%{http_code}' \
      -H 'Accept: application/vnd.npm.install-v1+json' -H 'Cache-Control: no-cache' "$url") ||
      status="no answer"
    case "$status" in
      200) jq -e 'type == "object"' "$body" >/dev/null && cat "$body" && rm -f "$body" && return 0 ;;
      404) echo '{}' && rm -f "$body" && return 0 ;;
    esac
    echo "npm's registry answered $status for $1 (try $attempt of 5)" >&2
    sleep 10
  done
  rm -f "$body"
  return 1
}

# Prints the `dist.shasum` of version $2 of the package $1, or nothing when npm has no such version.
registry_shasum() {
  local document
  document=$(registry_document "$1") || return 1
  jq -r --arg version "$2" '.versions[$version].dist.shasum // ""' <<<"$document"
}

# Prints the dist-tags of the package $1 as a JSON object, `{}` when it has none.
registry_dist_tags() {
  local document
  document=$(registry_document "$1") || return 1
  jq -c '."dist-tags" // {}' <<<"$document"
}
