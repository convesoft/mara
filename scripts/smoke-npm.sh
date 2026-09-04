#!/usr/bin/env bash

set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "usage: scripts/smoke-npm.sh <mara-binary>" >&2
  exit 2
fi

binary=$(realpath "$1")
marketplace=.agents/plugins/marketplace.json
test "$(node -p 'require("./" + process.argv[1]).name' "$marketplace")" = convesoft
test "$(node -p 'require("./" + process.argv[1]).plugins[0].name' "$marketplace")" = mara
test "$(node -p 'require("./" + process.argv[1]).plugins[0].source.package' "$marketplace")" = \
  @convesoft/mara
test "$(node -p 'require("./" + process.argv[1]).plugins[0].source.version' "$marketplace")" = next
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
plugin_cache="$temporary/plugin-cache"
project="$temporary/project"
shim="$temporary/shim"
mkdir -p "$packages" "$tarballs" "$install" "$plugin_cache" "$project" "$shim"

platform_package=$(node scripts/package-npm.mjs platform "$target" "$binary" "$packages")
main_package=$(node scripts/package-npm.mjs main "$packages")
for file in plugin.json mcp.json skills/mara/SKILL.md; do
  test -f "$main_package/$file"
done
test "$(node -p 'require(process.argv[1]).version' "$main_package/plugin.json")" = \
  "$(node -p 'require(process.argv[1]).version' "$main_package/package.json")"
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
node_binary=$(command -v node)
PATH="$temporary/empty" "$node_binary" \
  "$install/node_modules/@convesoft/mara/bin/mara-plugin.cjs" --version \
  | grep -F "mara $version"
(
  cd "$project"
  "$mara" project init >/dev/null
)
"$mara" --project "$project" --format json project validate | grep -F '"valid":true'

printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"mara-npm-smoke","version":"1"}}}' \
  | "$mara" mcp --project "$project" \
  | grep -F '"protocolVersion":"2025-06-18"'

printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"mara-npm-smoke","version":"1"}}}' \
  '{"jsonrpc":"2.0","method":"notifications/initialized"}' \
  "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\"params\":{\"name\":\"project_validate\",\"arguments\":{\"project\":\"$project\"}}}" \
  | (cd "$install" && "$mara" mcp) \
  | grep -F '"valid":true'

tar -xzf "$main_tarball" -C "$plugin_cache"
plugin="$plugin_cache/package"
test -f "$plugin/bin/mara-plugin.cjs"
test ! -e "$plugin/node_modules"
test "$(node -p 'require(process.argv[1]).mcpServers.mara.args[0]' "$plugin/mcp.json")" = \
  '${PLUGIN_ROOT}/bin/mara-plugin.cjs'

printf '%s\n' \
  '#!/usr/bin/env bash' \
  'set -euo pipefail' \
  'test "$PWD" = "${MARA_SMOKE_EXPECTED_CWD:?}"' \
  'test "$1" = --yes' \
  'shift' \
  'test "$1" = "${MARA_SMOKE_PACKAGE_SPEC:?}"' \
  'shift' \
  'exec "${MARA_SMOKE_EXECUTABLE:?}" "$@"' \
  > "$shim/npx"
chmod +x "$shim/npx"

printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"mara-codex-plugin-smoke","version":"1"}}}' \
  '{"jsonrpc":"2.0","method":"notifications/initialized"}' \
  "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\"params\":{\"name\":\"project_validate\",\"arguments\":{\"project\":\"$project\"}}}" \
  | (
      cd "$plugin"
      PATH="$shim:$PATH" \
        MARA_SMOKE_EXECUTABLE="$mara" \
        MARA_SMOKE_EXPECTED_CWD="$(node -p \
          'require("node:path").parse(process.argv[1]).root' "$plugin")" \
        MARA_SMOKE_PACKAGE_SPEC="@convesoft/mara@$version" \
        node bin/mara-plugin.cjs mcp
    ) \
  | grep -F '"valid":true'
