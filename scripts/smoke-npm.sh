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
  | "$mara" mcp --project "$project" \
  | grep -F '"protocolVersion":"2025-06-18"'

printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"mara-npm-smoke","version":"1"}}}' \
  '{"jsonrpc":"2.0","method":"notifications/initialized"}' \
  "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\"params\":{\"name\":\"project_validate\",\"arguments\":{\"project\":\"$project\"}}}" \
  | (cd "$install" && "$mara" mcp) \
  | grep -F '"valid":true'

# Exercise embedded templates from the installed package outside the source tree.
for template in empty engineering; do
  "$mara" project init "$temporary/$template" --template "$template" >/dev/null
  "$mara" --project "$temporary/$template" --format json schema validate | grep -F '"valid":true'
done

"$mara" --project "$temporary/engineering" item create requirement REQ-ACCESS knowledge.mara.md \
  --title "Permit access" --body "An authorized user can access the service." >/dev/null

node - "$mara" "$temporary/engineering" "$version" <<'NODE'
const assert = require("node:assert/strict");
const { spawnSync } = require("node:child_process");
const [mara, project, version] = process.argv.slice(2);
const call = (id, name, args) => ({ jsonrpc: "2.0", id, method: "tools/call", params: { name, arguments: args } });
const requests = [
  { jsonrpc: "2.0", id: 1, method: "initialize", params: { protocolVersion: "2025-06-18", capabilities: {}, clientInfo: { name: "mara-npm-smoke", version: "1" } } },
  { jsonrpc: "2.0", method: "notifications/initialized" },
  call(2, "schema_get", {}),
  call(3, "item_create", { flavour: "verification", id: "VER-ACCESS", file: "knowledge.mara.md", title: "Check access", body: "Demonstrate that an authorized user can access the service." }),
  call(4, "relation_add", { source: "VER-ACCESS", relation: "verifies", target: "REQ-ACCESS" }),
  call(5, "item_related", { id: "REQ-ACCESS", direction: "incoming" }),
  call(6, "project_validate", {}),
];
const output = spawnSync(mara, ["mcp", "--project", project], { cwd: project, input: requests.map(JSON.stringify).join("\n") + "\n", encoding: "utf8" });
assert.equal(output.status, 0, output.stderr);
const responses = output.stdout.trim().split("\n").map(JSON.parse);
const result = id => {
  const response = responses.find(response => response.id === id);
  assert.ok(response && !response.error, JSON.stringify(response));
  assert.notEqual(response.result.isError, true, JSON.stringify(response));
  return response.result;
};
assert.equal(result(1).serverInfo.version, version);
const schema = result(2).structuredContent.schema;
assert.equal(schema.format_version, 2);
assert.equal(Object.keys(schema.flavours).length, 11);
assert.deepEqual(schema.relations.verifies.target, ["requirement", "design"]);
assert.ok(result(3).structuredContent.mid);
assert.equal(result(4).structuredContent.action, "added");
assert.equal(result(5).structuredContent.items[0].item.id, "VER-ACCESS");
assert.equal(result(6).structuredContent.valid, true);
console.log("Installed engineering CLI/MCP workflow passed");
NODE
