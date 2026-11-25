// The daemon case's requests of VER-HOST-COMPARISON (`crates/mara/tests/daemon/parity.rs`) as
// MCP tool calls, for the Node and browser hosts, on the 1× corpus of DES-PERFORMANCE-HARNESS: an
// agent creates, relates, updates, reads, moves and renames items, writes code that implements
// them, repairs hand edits with fix, runs the queries, and meets failures, up to a schema broken
// by hand. The corpus declares Rust at the basic level, so the operations that read code read the
// markers of `src/`.

/** An invariant over code links, so that an item validation reads code (DES-CODE-READERS). */
const IMPLEMENTED =
  '\n[invariants.requirement-implemented]\nprogram = ".mara/rules/readiness.dl"\n' +
  'relation = "not_implemented"\nseverity = "warning"\n' +
  'message = "No code implements this requirement"\n';

/** The program of the queries that the agent writes by hand. */
const GATE_PROGRAM = `.include "mara.dl"
.decl listed(m: Mid, id: Id, status: Status)
listed(m, i, s) :- item(m, _, i, s, _, _).
.decl candidate(c: symbol)
.decl result(m: Mid, outcome: symbol, run: number)
.decl subject(m: Mid)
subject(m) :- item(m, _, _, _, _, _), candidate(_).
.decl has(m: Mid)
has(m) :- result(m, _, _).
.decl failed(m: Mid, outcome: symbol)
failed(m, o) :- subject(m), result(m, o, _), o = "fail".
failed(m, "missing") :- subject(m), !has(m), source_complete("result").
.decl missing(m: Mid)
missing(m) :- subject(m), !has(m), !source_complete("result").
`;

const QUERY_ENTRIES = `
[rules]
programs = [".mara/rules/*.dl"]

[queries.listed]
program = ".mara/rules/gate.dl"
relation = "listed"
description = "Every item with its ID and status"

[views.listed-view]
program = ".mara/rules/gate.dl"
relation = "listed"
description = "The same rows as a table"

[gates.passed]
program = ".mara/rules/gate.dl"
description = "Named items pass"
subjects = ["subject"]
conditions = ["failed"]
unavailable = ["missing"]
inputs = ["candidate", "result"]

[queries.broken]
program = ".mara/rules/broken.dl"
relation = "bad"
description = "A program that does not compile"

[gates.broken-gate]
program = ".mara/rules/broken.dl"
description = "A gate over it"
subjects = ["bad"]
conditions = ["bad"]
`;

/** The `[rules]` table and what follows it of a configuration replaced by `tail`. */
function rulesReplaced(config, tail) {
  const at = config.indexOf("\n[rules]\n");
  if (at === -1) {
    throw new Error(`the corpus's configuration has no [rules]: ${config}`);
  }
  return config.slice(0, at + 1) + tail;
}

/** The daemon case's requests on the project of `s`, a `Lockstep`. */
export async function agentWork(s) {
  await s.write(".mara/config.toml", s.read(".mara/config.toml") + IMPLEMENTED);
  const need = await s.run("item_create", {
    type: "need",
    path: "docs/agent/needs.mara.md",
    title: "Fast answers",
    field: ["status=draft"],
    body: "Agents get answers quickly.",
  });
  const mid = need.item.mid;
  const results = [[mid, "pass", -2_147_483_648]];
  const wrong = [[mid, "pass", 1.5]];
  const verified = [];

  await s.run("project_validate");
  await s.run("schema_list");
  await s.run("schema_get", { name: "requirement" });
  await s.run("schema_validate");
  await s.write("docs/agent/reqs.mara.md", "# Requirements\n\nPerformance obligations.\n");
  await s.run("item_create", {
    type: "requirement",
    path: "docs/agent/reqs.mara.md",
    title: "Cold read time",
    field: ["status=draft"],
    body: "A cold read takes at most 200 ms.",
  });
  await s.run("item_create", {
    type: "requirement",
    path: "docs/agent/reqs.mara.md",
    id: "REQ-WARM-READ",
    title: "Warm read time",
    field: ["status=draft"],
    body: "A warm read takes at most 20 ms.",
    line: 3,
  });
  await s.run("relation_add", {
    source: "REQ-COLD-READ-TIME",
    relation: "satisfies",
    target: "NEED-FAST-ANSWERS",
  });
  await s.run("relation_add", {
    source: "NEED-FAST-ANSWERS",
    relation: "satisfied_by",
    target: "REQ-WARM-READ",
  });

  await s.write(
    "src/agent/read.rs",
    "// @mara implements REQ-COLD-READ-TIME\npub fn cold() {\n    warm(); // @mara implements REQ-WARM-READ\n}\n",
  );
  await s.write("src/agent/typo.rs", "// @mara implements REQ-COLD-READ\nfn typo() {}\n");
  await s.run("project_validate");
  await s.run("relation_list", { node: "REQ-COLD-READ-TIME" });
  await s.run("relation_list", { node: "REQ-WARM-READ", relation: ["implemented_by"] });
  await s.run("relation_list", { node: "src/agent/read.rs:3" });
  await s.run("relation_list", { node: "src/agent/read.rs" });
  await s.run("relation_list", { node: "src/agent/read.rs:99" });
  await s.run("get", { node: "src/agent/read.rs:2" });
  await s.run("get", { node: "src/agent/read.rs" });
  await s.run("get", { node: "src/agent/none.rs" });
  await s.run("query_execute", { name: "implemented" });
  await s.run("query_view", {
    name: "requirement-verification",
    input: { candidate: "abc", verification_result: verified },
  });
  await s.run("item_delete", { item: "REQ-WARM-READ" });
  await s.write("src/agent/typo.rs", "// @mara implements REQ-COLD-READ-TIME\nfn typo() {}\n");
  await s.run("item_update", {
    item: "REQ-COLD-READ-TIME",
    title: "Cold read budget",
    body: "A cold read takes at most 200 ms at 16×.",
  });
  await s.run("get", { node: "REQ-COLD-READ-TIME" });
  await s.run("get", { node: "docs/agent/reqs.mara.md" });
  await s.run("relation_list", { node: "NEED-FAST-ANSWERS" });
  await s.run("search", { query: "cold read", limit: 5 });
  await s.run("item_list", { field: ["status=draft"], path: ["docs/agent"] });
  const page = await s.run("item_list", { limit: 1 });
  await s.run("item_list", { limit: 1, cursor: page.next_cursor });
  await s.run("item_move", { "id-or-mid": "REQ-COLD-READ-TIME", path: "docs/agent/perf/reqs.mara.md" });
  await s.run("item_move", { "id-or-mid": "REQ-WARM-READ", path: "docs/agent/reqs.mara.md", line: 1 });
  await s.run("item_rename", { "id-or-mid": "REQ-COLD-READ-TIME", "new-id": "REQ-COLD-READ-BUDGET" });
  await s.run("relation_list", { node: "src/agent/read.rs:1" });
  await s.run("item_rename", { "id-or-mid": "REQ-COLD-READ-BUDGET", "new-id": "NEED-WRONG" });

  // Hand edits: a document moved by hand, and an item written without an anchor whose relation
  // names its target by ID.
  await s.rename("docs/agent/needs.mara.md", "docs/agent/product/needs.mara.md");
  await s.write(
    "docs/agent/hand.mara.md",
    "# Hand notes\n\n---\n\n## Hand written\n\n| Type | Requirement |\n| --- | --- |\n" +
      "| ID | REQ-HAND-WRITTEN |\n| Status | Draft |\n| Satisfies | NEED-FAST-ANSWERS |\n\n" +
      "Written without an anchor.\n",
  );
  await s.run("project_validate");
  await s.run("project_fix");
  await s.run("project_validate", { path: ["docs/agent"], limit: 10 });
  await s.run("item_validate", { "id-or-mid": "REQ-HAND-WRITTEN" });
  const inputs = { candidate: "abc", verification_result: verified };
  await s.run("query_gate", { name: "verified", id: ["REQ-HAND-WRITTEN"], input: inputs });
  await s.run("query_report", { name: "verified", id: "REQ-HAND-WRITTEN", input: inputs });
  await s.run("relation_list", { node: "docs/agent/hand.mara.md" });

  // Requests that fail.
  await s.run("get", { node: "REQ-NOPE" });
  await s.run("get", { node: "docs/agent/none.mara.md:3" });
  await s.run("schema_get", { name: "nothing" });
  await s.run("item_validate", { "id-or-mid": "REQ-NOPE" });
  await s.run("relation_list", { node: "REQ-NOPE" });
  await s.run("item_delete", { item: "NEED-FAST-ANSWERS" });
  await s.run("item_update", { item: "REQ-WARM-READ" });
  await s.run("item_list", { limit: 0 }, { rejected: true });
  await s.run(
    "item_move",
    { "id-or-mid": "REQ-WARM-READ", path: "docs/agent/reqs.mara.md", line: 0 },
    { rejected: true },
  );
  // Required options left out: refused before anything is sent (DES-MCP-SERVER).
  await s.run(
    "item_create",
    { type: "requirement", path: "docs/agent/reqs.mara.md", body: "Text." },
    { rejected: true },
  );
  await s.run("query_gate", { name: "verified" }, { rejected: true });
  await s.run("query_gate", { name: "verified", id: [] }, { rejected: true });
  await s.run("query_report", { name: "verified" }, { rejected: true });
  await s.run("item_create", {
    type: "requirement",
    path: "notes/x.md",
    title: "Outside",
    field: ["status=draft"],
    body: "Text.",
  });
  await s.run("item_move", {
    "id-or-mid": "REQ-WARM-READ",
    path: "docs/agent/perf/reqs.mara.md",
    line: 999,
  });
  const relation = { source: "REQ-HAND-WRITTEN", relation: "satisfies", target: "NEED-FAST-ANSWERS" };
  await s.run("relation_remove", relation);
  await s.run("relation_remove", relation);
  await s.run("item_delete", { item: "REQ-HAND-WRITTEN" });

  // The queries, written by hand into the configuration in place of its own entries, with the
  // inputs as rows.
  await s.write(".mara/config.toml", rulesReplaced(s.read(".mara/config.toml"), QUERY_ENTRIES));
  await s.write(".mara/rules/gate.dl", GATE_PROGRAM);
  await s.write(".mara/rules/broken.dl", '.include "mara.dl"\n.decl bad(m: Mid)\nbad(m) :- missing(m).\n');
  const gate = (extra = {}) => ({
    name: "passed",
    id: ["NEED-FAST-ANSWERS", "REQ-COLD-READ-BUDGET"],
    input: { candidate: "abc", result: results },
    ...extra,
  });
  await s.run("query_list");
  await s.run("query_execute", { name: "listed" });
  await s.run("query_view", { name: "listed-view" });
  await s.run("query_execute", { name: "listed", id: ["REQ-COLD-READ-BUDGET"], limit: 1 });
  await s.run("query_gate", gate());
  await s.run("query_gate", gate({ complete: ["result"] }));
  await s.run("query_execute", { name: "nothing" });
  await s.run("query_view", { name: "listed" });
  await s.run("query_execute", { name: "listed", id: ["REQ-NONE"] });
  await s.run("query_gate", { name: "passed", id: ["NEED-FAST-ANSWERS"], input: { candidate: "abc" } });
  await s.run("query_gate", gate({ input: { candidate: "abc", result: results, other: "1" } }));
  await s.run("query_gate", gate({ input: { candidate: "abc", result: wrong } }));
  await s.run("query_execute", { name: "broken" });
  await s.run("query_gate", { name: "broken-gate", id: ["NEED-FAST-ANSWERS"] });
  await s.run("query_report", {
    name: "passed",
    id: "REQ-COLD-READ-BUDGET",
    input: { candidate: "abc", result: results },
    complete: ["result"],
  });
  await s.run("query_report", { name: "broken-gate", id: "NEED-FAST-ANSWERS" });
  await s.run("query_report", { name: "listed", id: "REQ-COLD-READ-BUDGET" });

  // A schema broken by hand: every operation but validation refuses the project.
  const schema = s.read(".mara/schema.toml");
  await s.write(".mara/schema.toml", "format_version = 1\nstatuses = [\n");
  await s.run("item_list");
  await s.run("get", { node: "REQ-WARM-READ" });
  await s.run("project_fix");
  await s.run("relation_add", { source: "REQ-WARM-READ", relation: "satisfies", target: "NEED-FAST-ANSWERS" });
  await s.run("query_execute", { name: "listed" });
  await s.run("project_validate");
  await s.run("schema_validate");
  await s.write(".mara/schema.toml", schema);
}
