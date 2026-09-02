#!/usr/bin/env bash

set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "usage: scripts/smoke-npm.sh <mara-binary>" >&2
  exit 2
fi

binary=$(realpath "$1")
case "$(uname -s):$(uname -m)" in
  Linux:x86_64) target=x86_64-unknown-linux-gnu ;;
  Linux:aarch64) target=aarch64-unknown-linux-gnu ;;
  Darwin:x86_64) target=x86_64-apple-darwin ;;
  Darwin:arm64) target=aarch64-apple-darwin ;;
  *)
    echo "unsupported smoke-test host: $(uname -s)/$(uname -m)" >&2
    exit 1
    ;;
esac

temporary=$(mktemp -d)
trap 'rm -rf -- "$temporary"' EXIT

packages="$temporary/packages"
tarballs="$temporary/tarballs"
install="$temporary/install"
project="$temporary/project"
mkdir -p "$packages" "$tarballs" "$install" "$project"

platform_package=$(node scripts/package-npm.mjs platform "$target" "$binary" "$packages")
main_package=$(node scripts/package-npm.mjs main "$packages")
npm pack "$platform_package" --pack-destination "$tarballs" >/dev/null
npm pack "$main_package" --pack-destination "$tarballs" >/dev/null

platform_filename=$(node -e \
  'const p=require(process.argv[1]); process.stdout.write(`${p.name.slice(1).replace("/", "-")}-${p.version}.tgz`)' \
  "$platform_package/package.json")
main_filename=$(node -e \
  'const p=require(process.argv[1]); process.stdout.write(`${p.name.slice(1).replace("/", "-")}-${p.version}.tgz`)' \
  "$main_package/package.json")
version=$(node -p 'require(process.argv[1]).version' "$main_package/package.json")
platform_tarball="$tarballs/$platform_filename"
main_tarball="$tarballs/$main_filename"
test -f "$platform_tarball"
test -f "$main_tarball"

export npm_config_cache="$temporary/npm-cache"
npm install \
  --prefix "$install" \
  --ignore-scripts \
  --no-audit \
  --no-fund \
  --package-lock=false \
  "$platform_tarball" \
  "$main_tarball" >/dev/null

mara="$install/node_modules/.bin/mara"
"$mara" --version | grep -F "mara $version"
(
  cd "$project"
  "$mara" project init >/dev/null
)
"$mara" --project "$project" --format json project validate | grep -F '"valid":true'

printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"mara-npm-smoke","version":"1"}}}' \
  | "$mara" --project "$project" mcp \
  | grep -F '"protocolVersion":"2025-06-18"'
