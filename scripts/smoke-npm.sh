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
"$mara" project init "$temporary/empty" --template empty >/dev/null
"$mara" --project "$temporary/empty" --format json schema validate | grep -F '"valid":true'

node - "$mara" "$temporary/engineering" "$version" <<'NODE'
const assert = require("node:assert/strict");
const { spawnSync } = require("node:child_process");
const { mkdirSync, readdirSync, readFileSync, symlinkSync, writeFileSync } = require("node:fs");
const path = require("node:path");
const [mara, engineering, version] = process.argv.slice(2);
let project = engineering;
// Only Node is discoverable: the installed dispatcher must launch its bundled binary.
const runtime = path.join(path.dirname(project), "runtime");
mkdirSync(runtime);
symlinkSync(process.execPath, path.join(runtime, "node"));
const env = { ...process.env, PATH: runtime };
for (const command of ["cargo", "rustc", "rustup", "mara"]) {
  assert.equal(spawnSync(command, ["--version"], { env }).error?.code, "ENOENT");
}
mkdirSync(project);
for (const args of [
  ["project", "init", "--template", "engineering"],
  ["item", "create", "requirement", "REQ-ACCESS", "knowledge.mara.md", "--title", "Permit access", "--body", "An authorized user can access the service."],
]) {
  const output = spawnSync(mara, ["--project", project, ...args], { cwd: project, env, encoding: "utf8" });
  assert.equal(output.status, 0, output.stderr || output.stdout);
  if (args[0] === "project") {
    assert.deepEqual(readdirSync(project), [".mara"]);
    assert.deepEqual(readdirSync(path.join(project, ".mara")).sort(), ["project.toml", "schema.yaml"]);
  }
}
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
const output = spawnSync(mara, ["mcp", "--project", project], { cwd: project, env, input: requests.map(JSON.stringify).join("\n") + "\n", encoding: "utf8" });
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
  const output = spawnSync(mara, ["--project", project, "--format", "json", ...args], { cwd: project, env, encoding: "utf8" });
  assert.equal(output.status, 0, output.stderr || output.stdout);
  return JSON.parse(output.stdout);
};
const mcp = request => {
  const output = spawnSync(mara, ["mcp", "--project", project], {
    cwd: project,
    env,
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
console.log("PASS packaged guidance and narrative → requirement → verification CLI/MCP parity (Node-only PATH)");

// Extend the same real project with a design and cross-document narrative context.
cli(["item", "create", "design", "DES-ACCESS", "knowledge.mara.md", "--title", "Access gate",
  "--body", "Check authorization before granting access.", "--relation", "satisfies=REQ-ACCESS"]);
mkdirSync(path.join(project, "docs"));
writeFileSync(path.join(project, "docs/policy.mara.md"), "# Access policy\n\nApply the authorization gate.\n\n## Audit\n\nRetain access decisions.\n");
writeFileSync(path.join(project, "context.mara.md"), "# Access context\n\nStart access here: [[REQ-ACCESS]].\n\n[Policy](./docs/policy.mara.md#access-policy) and [document](./docs/policy.mara.md).\n");
const linked = parity(["search", "Policy", "--path", "context.mara.md"], "search", { query: "Policy", paths: ["context.mara.md"] }).results[0].node;
const links = parity(["related", linked.reference, "--relation", "builtin:mentions", "--direction", "outgoing"], "related", { reference: linked.reference, relations: ["builtin:mentions"], direction: "outgoing" });
assert.deepEqual(new Set(links.connections.map(edge => edge.neighbour.kind)), new Set(["section", "document"]));
for (const edge of links.connections) {
  assert.equal(edge.source.path, "context.mara.md");
  assert.ok(read(edge.neighbour.reference).content.includes("Retain access decisions."));
  const incoming = parity(["related", edge.neighbour.reference, "--relation", "builtin:mentions", "--direction", "incoming"], "related", { reference: edge.neighbour.reference, relations: ["builtin:mentions"], direction: "incoming" });
  assert.ok(incoming.connections.some(backlink => backlink.neighbour.reference === linked.reference));
}
const parents = parity(["related", linked.reference, "--relation", "builtin:contains", "--direction", "incoming"], "related", { reference: linked.reference, relations: ["builtin:contains"], direction: "incoming" });
assert.equal(parents.connections.length, 1);
assert.equal(parents.connections[0].neighbour.reference, linked.context.parent);
const parent = parents.connections[0].neighbour.reference;
const children = parity(["related", parent, "--relation", "builtin:contains", "--direction", "outgoing"], "related", { reference: parent, relations: ["builtin:contains"], direction: "outgoing" });
assert.equal(children.connections.length, 2);
const sibling = children.connections.find(edge => edge.neighbour.reference !== linked.reference).neighbour;
assert.ok(read(sibling.reference).content.includes("[[REQ-ACCESS]]"));

// Force real search/related continuation and compare every page across transports.
const pagedSearch = [];
let cursor = null;
do {
  const page = parity(["search", "access", "--limit", "1", ...(cursor ? ["--cursor", cursor] : [])], "search", { query: "access", limit: 1, ...(cursor ? { cursor } : {}) });
  pagedSearch.push(...page.results);
  assert.equal(page.has_more, page.next_cursor !== null);
  cursor = page.next_cursor;
} while (cursor);
assert.deepEqual(pagedSearch, parity(["search", "access"], "search", { query: "access" }).results);
const pagedChildren = [];
do {
  const page = parity(["related", parent, "--relation", "builtin:contains", "--direction", "outgoing", "--limit", "1", ...(cursor ? ["--cursor", cursor] : [])], "related", { reference: parent, relations: ["builtin:contains"], direction: "outgoing", limit: 1, ...(cursor ? { cursor } : {}) });
  pagedChildren.push(...page.connections);
  assert.equal(page.has_more, page.next_cursor !== null);
  cursor = page.next_cursor;
} while (cursor);
assert.deepEqual(pagedChildren, children.connections);
console.log("PASS cross-document section/document links, backlinks, sibling navigation, search/related continuation");

// Reconstruct an oversized Unicode block, its section, and its document from bounded reads.
const largeSource = "# Large context\n\n" + 'Unicode e\u0301 "quoted" \\ text 🦀. '.repeat(4000) + "\n";
writeFileSync(path.join(project, "large.mara.md"), largeSource);
const large = parity(["search", "Unicode", "--path", "large.mara.md"], "search", { query: "Unicode", paths: ["large.mara.md"] });
assert.equal(large.results.length, 1);
assert.equal(large.results[0].excerpt.partial, true);
let reference = large.results[0].node.reference;
const largeReference = reference;
let readCursor;
const readKinds = [];
while (reference) {
  let content = "";
  let pages = 0;
  let node;
  do {
    const page = parity(["get", reference, ...(cursor ? ["--cursor", cursor] : [])], "get", { reference, ...(cursor ? { cursor } : {}) });
    node = page.node;
    assert.equal(page.format_version, 1);
    assert.ok(Buffer.byteLength(JSON.stringify(page)) <= 65536);
    assert.equal(page.content_range.start_byte, Buffer.byteLength(content));
    content += page.content;
    assert.equal(page.content_range.end_byte, Buffer.byteLength(content));
    assert.deepEqual(page.metadata, []);
    assert.equal(page.has_more, page.next_cursor !== null);
    cursor = page.next_cursor;
    if (node.kind === "block" && pages === 0) readCursor = cursor;
    pages++;
  } while (cursor);
  assert.ok(pages > 1);
  assert.equal(content, Buffer.from(largeSource).subarray(node.source.start_byte, node.source.end_byte).toString());
  readKinds.push(node.kind);
  reference = node.context.parent;
}
assert.deepEqual(readKinds, ["block", "section", "document"]);
console.log("PASS oversized Unicode block/section/document reads reconstruct exact source within 65536-byte pages");

const rejects = (command, name, args, diagnostic) => {
  const output = spawnSync(mara, ["--project", project, "--format", "json", ...command], { cwd: project, env, encoding: "utf8" });
  assert.notEqual(output.status, 0);
  assert.match(output.stderr + output.stdout, diagnostic);
  const response = mcp(call(2, name, args));
  assert.ok(response.error || response.result.isError, JSON.stringify(response));
  assert.match(JSON.stringify(response), diagnostic);
};
const searchCursor = parity(["search", "access", "--limit", "1"], "search", { query: "access", limit: 1 }).next_cursor;
const relatedCursor = parity(["related", parent, "--limit", "1"], "related", { reference: parent, limit: 1 }).next_cursor;
assert.ok(searchCursor && relatedCursor && readCursor);
writeFileSync(path.join(project, "other.mara.md"), "Unrelated new narrative.\n");
assert.equal(read(largeReference).node.reference, largeReference);
rejects(["get", largeReference, "--cursor", readCursor], "get", { reference: largeReference, cursor: readCursor }, /stale get cursor/);
rejects(["search", "access", "--limit", "1", "--cursor", searchCursor], "search", { query: "access", limit: 1, cursor: searchCursor }, /stale cursor/);
rejects(["related", parent, "--limit", "1", "--cursor", relatedCursor], "related", { reference: parent, limit: 1, cursor: relatedCursor }, /stale.*cursor/);
writeFileSync(path.join(project, "large.mara.md"), largeSource + "\nChanged document.\n");
rejects(["get", largeReference], "get", { reference: largeReference }, /search again/);
rejects(["related", largeReference], "related", { reference: largeReference }, /rediscover/);
const fresh = parity(["search", "Unicode", "--path", "large.mara.md"], "search", { query: "Unicode", paths: ["large.mara.md"] }).results[0].node;
assert.notEqual(fresh.reference, largeReference);
assert.ok(read(fresh.reference).content.startsWith("Unicode"));
assert.equal(parity(["project", "validate"], "project_validate", {}).valid, true);
console.log("PASS stale search/get/related cursors and structural handles reject; rediscovery succeeds");

// Use the guide's customized schema, generating genuine item identities before
// staging its format-1 state. This fixture does not require an old executable.
const examples = [...readFileSync("docs/migration-0.2.mara.md", "utf8").matchAll(/```yaml\n([\s\S]*?)```/g)].map(match => match[1]);
assert.equal(examples.length, 2);
const [before, after] = examples;
project = path.join(path.dirname(engineering), "customized");
mkdirSync(project);
cli(["project", "init", "--template", "empty"]);
const schemaPath = path.join(project, ".mara/schema.yaml");
writeFileSync(schemaPath, after);
cli(["item", "create", "term", "TERM-BASE", "terms.mara.md", "--title", "Base term", "--body", "Project-specific base meaning.", "--field", "alias=base"]);
cli(["item", "create", "term", "TERM-CUSTOM", "terms.mara.md", "--title", "Custom term", "--body", "Clarifies [[TERM-BASE]].", "--field", "alias=custom", "--field", "alias=second", "--relation", "clarifies=TERM-BASE"]);
const originalItems = cli(["item", "list"]);
const originalSchema = cli(["schema", "get"]);
const originalDocument = readFileSync(path.join(project, "terms.mara.md"));
const originalConfig = readFileSync(path.join(project, ".mara/project.toml"));
writeFileSync(schemaPath, before);
rejects(["schema", "validate"], "schema_validate", {}, /migrate/);
assert.equal(readFileSync(schemaPath, "utf8"), before);
writeFileSync(schemaPath, before.replace("format_version: 1", "format_version: 2"));
rejects(["schema", "validate"], "schema_validate", {}, /use_when/);
// Migrate in place by changing the version and adding guidance only.
const guidance = after.slice(after.indexOf("    use_when:"), after.indexOf("    id_prefix:"));
const migrated = before.replace("format_version: 1", "format_version: 2").replace("    id_prefix:", guidance + "    id_prefix:");
assert.equal(migrated, after);
writeFileSync(schemaPath, migrated);
assert.deepEqual(parity(["schema", "get"], "schema_get", {}), originalSchema);
assert.deepEqual(parity(["item", "list"], "item_list", {}), originalItems);
assert.deepEqual(readFileSync(path.join(project, "terms.mara.md")), originalDocument);
assert.deepEqual(readFileSync(path.join(project, ".mara/project.toml")), originalConfig);
assert.deepEqual(readdirSync(project).sort(), [".mara", "terms.mara.md"]);
assert.equal(parity(["schema", "validate"], "schema_validate", {}).valid, true);
assert.equal(parity(["project", "validate"], "project_validate", {}).valid, true);
assert.ok(read("TERM-CUSTOM").content.includes("[[TERM-BASE]]"));
const customRelations = parity(["related", "TERM-CUSTOM", "--relation", "clarifies", "--direction", "outgoing"], "related", { reference: "TERM-CUSTOM", relations: ["clarifies"], direction: "outgoing" });
assert.equal(customRelations.connections[0].neighbour.id, "TERM-BASE");
const created = mcp(call(2, "item_create", { flavour: "term", id: "TERM-NEW", file: "terms.mara.md", title: "New term", body: "New project-specific meaning.", fields: [{ key: "alias", value: "new" }], relations: [{ relation: "clarifies", target: "TERM-CUSTOM" }] }));
assert.ok(!created.error && !created.result.isError, JSON.stringify(created));
assert.ok(created.result.structuredContent.mid);
assert.equal(parity(["related", "TERM-CUSTOM", "--relation", "clarifies", "--direction", "incoming"], "related", { reference: "TERM-CUSTOM", relations: ["clarifies"], direction: "incoming" }).connections[0].neighbour.id, "TERM-NEW");
assert.equal(parity(["project", "validate"], "project_validate", {}).valid, true);
console.log("PASS customized format-1 migration preserves schema declarations, repeated fields, relations, IDs/MIDs and source bytes; new MCP authoring succeeds");
NODE
