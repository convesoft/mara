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
const { readFileSync, writeFileSync } = require("node:fs");
const path = require("node:path");
const [mara, project, version] = process.argv.slice(2);
const call = (id, name, args) => ({ jsonrpc: "2.0", id, method: "tools/call", params: { name, arguments: args } });
const requests = [
  { jsonrpc: "2.0", id: 1, method: "initialize", params: { protocolVersion: "2025-06-18", capabilities: {}, clientInfo: { name: "mara-npm-smoke", version: "1" } } },
  { jsonrpc: "2.0", method: "notifications/initialized" },
  call(2, "schema_get", {}),
  call(3, "item_create", { flavour: "verification", id: "VER-ACCESS", file: "knowledge.mara.md", title: "Check access", body: "Demonstrate that an authorized user can access the service." }),
  call(4, "relation_add", { source: "VER-ACCESS", relation: "verifies", target: "REQ-ACCESS" }),
  call(5, "related", { reference: "REQ-ACCESS", direction: "incoming", relations: ["verifies"] }),
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
assert.equal(result(5).structuredContent.connections[0].neighbour.id, "VER-ACCESS");
assert.equal(result(6).structuredContent.valid, true);

const cli = args => {
  const output = spawnSync(mara, ["--project", project, "--format", "json", ...args], { cwd: project, encoding: "utf8" });
  assert.equal(output.status, 0, output.stderr || output.stdout);
  return JSON.parse(output.stdout);
};
const mcp = request => {
  const output = spawnSync(mara, ["mcp", "--project", project], {
    cwd: project,
    input: [requests[0], requests[1], request].map(JSON.stringify).join("\n") + "\n",
    encoding: "utf8",
  });
  assert.equal(output.status, 0, output.stderr);
  const response = output.stdout.trim().split("\n").map(JSON.parse).find(response => response.id === request.id);
  assert.ok(response, output.stdout);
  return response;
};
const parity = (command, name, args) => {
  const expected = cli(command);
  const response = mcp(call(2, name, args));
  assert.ok(!response.error && !response.result.isError, JSON.stringify(response));
  assert.deepEqual(response.result.structuredContent, expected);
  return expected;
};

assert.deepEqual(cli(["schema", "get"]).schema, schema);
for (const name of Object.keys(schema.flavours)) {
  const declaration = parity(["schema", "get", "flavour", name], "schema_get", { kind: "flavour", name }).definition;
  assert.ok(declaration.description.trim());
  assert.ok(declaration.use_when.length > 0);
  assert.ok(Array.isArray(declaration.avoid_when));
  assert.equal(typeof declaration.distinguish_from, "object");
}

// Read the actual installed skill and compare it with the source packaged for this run.
const installedSkill = path.resolve(path.dirname(mara), "../@convesoft/mara/skills/mara/SKILL.md");
assert.equal(readFileSync(installedSkill, "utf8"), readFileSync("skills/mara/SKILL.md", "utf8"));
const listing = mcp({ jsonrpc: "2.0", id: 2, method: "tools/list", params: {} }).result.tools;
const names = new Set(listing.map(tool => tool.name));
for (const name of ["search", "get", "related", "item_list", "item_create", "schema_get", "project_init", "relation_add"]) {
  assert.ok(names.has(name), name);
}
for (const name of ["item_search", "item_get", "item_related"]) {
  assert.ok(!names.has(name), name);
  assert.ok(mcp(call(2, name, { id: "REQ-ACCESS" })).error, name);
}
for (const [name, property] of [["search", "excerpts"], ["get", "id"], ["get", "limit"], ["related", "id"]]) {
  assert.ok(!(property in listing.find(tool => tool.name === name).inputSchema.properties));
}

// Follow a narrative mention to an item, then its verification, using only discovery references.
writeFileSync(path.join(project, "context.mara.md"), "# Access context\n\nStart access here: [[REQ-ACCESS]].\n\n[Checks](knowledge.mara.md).\n");
const search = parity(["search", "access"], "search", { query: "access" });
assert.equal(search.format_version, 1);
assert.equal(search.has_more, false);
assert.deepEqual(new Set(search.results.map(hit => hit.node.kind)), new Set(["item", "section", "block"]));
const narrative = search.results.find(hit => hit.node.kind === "block").node;
const read = reference => parity(["get", reference], "get", { reference });
assert.ok(read(narrative.reference).content.includes("[[REQ-ACCESS]]"));
const mentions = parity(["related", narrative.reference, "--relation", "builtin:mentions", "--direction", "outgoing"], "related", { reference: narrative.reference, relations: ["builtin:mentions"], direction: "outgoing" });
assert.equal(mentions.connections.length, 1);
const requirement = mentions.connections[0].neighbour;
assert.equal(requirement.id, "REQ-ACCESS");
assert.ok(read(requirement.reference).content.includes("authorized user"));
const checks = parity(["related", requirement.reference, "--relation", "verifies", "--direction", "incoming"], "related", { reference: requirement.reference, relations: ["verifies"], direction: "incoming" });
assert.equal(checks.connections[0].neighbour.id, "VER-ACCESS");
assert.ok(read(checks.connections[0].neighbour.reference).content.includes("Demonstrate"));
const section = read(narrative.context.parent);
assert.equal(section.node.kind, "section");
assert.equal(read(section.node.context.parent).node.kind, "document");
assert.equal(parity(["schema", "validate"], "schema_validate", {}).valid, true);
assert.equal(parity(["project", "validate"], "project_validate", {}).valid, true);
console.log("Installed 0.2 schema guidance and narrative → item → verification CLI/MCP workflow passed");
NODE
