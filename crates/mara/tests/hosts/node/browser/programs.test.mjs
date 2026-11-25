// The browser rows of the rules Verification items (docs/rules/verification.mara.md): programs
// evaluated by Soufflé's WebAssembly module in a browser workspace. VER-FACT-ROWS,
// VER-EXPLANATIONS and the arithmetic of VER-PROGRAM-REACH-CASES give the rows and proofs of CLI
// direct mode on a copy, which the native tests check against `get` and `relation list`; each
// runtime fault of VER-PROGRAM-FAULTS makes the outcome unavailable, a fault without a line at the
// configuration line that declares the program.

import assert from "node:assert/strict";
import { after, before, describe, test } from "node:test";

import { launchBrowser, openPage, requireSouffle, startServer } from "../support/hosts.mjs";
import { PROGRAMS_LINE, admitted, bothSeverities, declare, invariant, mid } from "../support/programs.mjs";
import { Compared } from "../support/workspace.mjs";

/** VER-FACT-ROWS's program: each fact relation copied into one that starts with an item's MID. */
const FACTS = `.include "mara.dl"
.decl note(text: symbol)

.decl r_item(mid: Mid, type: Type, id: Id, status: Status, path: Path, line: number)
r_item(m, t, i, s, p, l) :- item(m, t, i, s, p, l).
.decl r_title(mid: Mid, text: symbol)
r_title(m, t) :- title(m, t).
.decl r_revision(mid: Mid, hash: symbol)
r_revision(m, h) :- revision(m, h).
.decl r_field(mid: Mid, key: symbol, value: symbol)
r_field(m, k, v) :- field(m, k, v).
.decl r_edge(src: Mid, rel: Rel, dst: Mid, path: Path, line: number)
r_edge(s, r, d, p, l) :- edge(r, s, d, p, l).
.decl r_folded(mid: Mid, text: symbol, folded: symbol)
r_folded(m, t, f) :- title(m, t), folded(t, f).
r_folded(m, v, f) :- field(m, _, v), folded(v, f).
.decl r_complete(mid: Mid, source: symbol)
r_complete(m, s) :- item(m, _, _, _, _, _), source_complete(s).
.decl r_note(mid: Mid, text: symbol)
r_note(m, t) :- item(m, _, _, _, _, _), note(t).
`;

const FACT_RELATIONS = ["r_item", "r_title", "r_revision", "r_field", "r_edge", "r_folded", "r_complete", "r_note"];

/** The configuration of the fixture `facts` with a query on each relation of [`FACT_RELATIONS`]. */
function factsConfig() {
  let config =
    'format_version = 1\nschema = ".mara/schema.toml"\n\n[documents]\nformat_version = 1\n' +
    'include = ["**/*.mara.md"]\n\n[rules]\nprograms = [".mara/rules/*.dl"]\n';
  for (const relation of FACT_RELATIONS) {
    config +=
      `\n[queries.${relation}]\nprogram = ".mara/rules/facts.dl"\nrelation = "${relation}"\n` +
      `description = "The rows of ${relation}"\ninputs = ["note"]\n`;
  }
  return config;
}

/** VER-EXPLANATIONS's join of `item`, `field` and `edge`, and a gate on a supplied input. */
const JOIN = `.include "mara.dl"
.decl joined(m: Mid, n: Mid, o: symbol)
joined(m, n, o) :- item(m, "requirement", _, "accepted", _, _), field(m, "owner", o),
    edge("satisfies", m, n, _, _).
.decl result(m: Mid, r: symbol)
.decl subject(m: Mid)
subject(m) :- item(m, "requirement", _, _, _, _).
.decl condition(m: Mid, r: symbol)
condition(m, r) :- subject(m), result(m, r), r != "pass".
`;

const JOIN_ENTRIES =
  invariant("joined", "p.dl", "joined", "error") +
  '\n[gates.release]\nprogram = ".mara/rules/p.dl"\ndescription = "d"\n' +
  'subjects = ["subject"]\nconditions = ["condition"]\ninputs = ["result"]\n';

const NEGATION = `.include "mara.dl"
.decl incoming(m: Mid)
incoming(m) :- edge(_, _, m, _, _).
.decl lonely(m: Mid)
lonely(m) :- item(m, "requirement", _, "accepted", _, _), !incoming(m).
.decl unlinked(m: Mid)
unlinked(m) :- item(m, "requirement", _, "draft", _, _), !edge(_, m, _, _, _).
.type At = [m: symbol, line: number]
.decl at(a: At)
at([m, l]) :- item(m, _, _, _, _, l).
.decl placed(m: Mid)
placed(m) :- at([m, _]), item(m, "need", _, _, _, _).
.comp Typed<T> {
    .decl of(m: T)
    of(m) :- item(m, "need", _, _, _, _).
}
.init needs = Typed<Mid>
.decl component(m: Mid)
component(m) :- needs.of(m).
.decl linked(a: Mid, b: Mid) eqrel
linked(a, b) :- edge("satisfies", a, b, _, _).
.decl connected(m: Mid)
connected(m) :- linked(m, x), x != m, item(x, "requirement", _, _, _, _).
`;

const UNWRITABLE = `.include "mara.dl"
.decl result(m: Mid, r: symbol)
.decl score(m: Mid, n: number)
.decl subject(m: Mid)
subject(m) :- item(m, "requirement", _, _, _, _).
.decl condition(m: Mid, r: symbol)
condition(m, r) :- subject(m), result(m, r).
.decl low(m: Mid, n: number)
low(m, n) :- subject(m), score(m, n).
.decl titled(m: Mid, t: symbol)
titled(m, t) :- title(m, t), item(m, "requirement", _, "draft", _, _).
`;

const AGGREGATE = `.include "mara.dl"
.decl counted(m: Mid, c: number)
counted(m, c) :- item(m, "need", _, _, _, _), c = count : item(_, _, _, _, _, _).
.decl total(c: number)
total(c) :- c = count : item(_, _, _, _, _, _).
.decl crowded(m: Mid)
crowded(m) :- item(m, "need", _, _, _, _), total(c), c > 1.
`;

/** The proof steps of `proof`, depth first. */
function nodes(proof) {
  return [proof, ...(proof.children ?? []).flatMap(nodes)];
}

/** Every row of a report has a proof. */
function explained(report) {
  assert.ok(report.rows.length > 0, JSON.stringify(report));
  for (const row of report.rows) {
    assert.ok(row.proof, JSON.stringify(row));
  }
}

describe("programs in a browser workspace give the rows and proofs of CLI direct mode", { timeout: 600_000 }, () => {
  let server;
  let browser;
  let page;

  before(async () => {
    requireSouffle();
    server = await startServer();
    browser = await launchBrowser();
    page = await openPage(browser, server);
  });

  after(async () => {
    await page?.close();
    await browser?.close();
    await server?.close();
  });

  test("VER-FACT-ROWS: each fact relation and a supplied input read back through a query", async () => {
    const workspace = await Compared.create(page, "facts");
    await workspace.write(".mara/rules/facts.dl", FACTS);
    await workspace.write(".mara/config.toml", factsConfig());
    const note = [["line\nbreak\ttab"]];
    for (const complete of [[], ["note"]]) {
      for (const relation of FACT_RELATIONS) {
        const args = { name: relation, input: { note }, complete, limit: 100 };
        const expected = await workspace.request("query_execute", args);
        assert.equal(expected.error, undefined, JSON.stringify(expected));
        assert.equal(expected.rows.length > 0, relation !== "r_complete" || complete.length > 0, relation);
      }
    }
    // A rule that compares a `Mid` with an `Id` does not compile.
    await workspace.write(".mara/rules/facts.dl", `${FACTS}r_note(m, i) :- item(m, _, i, _, _, _), m = i.\n`);
    const failed = await workspace.request("query_execute", { name: "r_note", input: { note: "x" } });
    assert.equal(failed.error.code, "program-failed", JSON.stringify(failed));
    assert.equal(failed.error.diagnostics[0].code, "program-error", JSON.stringify(failed));
  });

  test("VER-EXPLANATIONS: rows of invariants and gates are explained with the proofs of CLI direct mode", async () => {
    const workspace = await Compared.create(page, "programs");
    await declare(workspace, JOIN, JOIN_ENTRIES);
    explained(await workspace.request("query_report", { name: "joined", id: "REQ-A" }));
    const failing = { name: "release", id: "REQ-A", input: { result: [[mid(1), "fail"]] } };
    explained(await workspace.request("query_report", failing));
    await workspace.request("query_gate", { ...failing, id: ["REQ-A"] });
    await workspace.request("project_validate");

    const entries = ["lonely", "unlinked", "placed", "component", "connected"];
    await declare(workspace, NEGATION, entries.map((name) => invariant(name, "p.dl", name, "warning")).join(""));
    for (const [name, id] of [
      ["lonely", "REQ-A"],
      ["unlinked", "REQ-B"],
      ["component", "NEED-ONE"],
      ["connected", "NEED-ONE"],
    ]) {
      explained(await workspace.request("query_report", { name, id }));
    }
    // Soufflé's two builds number the record of this proof differently, #3 natively and #2 in
    // WebAssembly, on the same files and commands: the number is masked until mara-ecnpg6
    // decides.
    const records = (value) => JSON.parse(JSON.stringify(value).replace(/record #\d+/g, "record #…"));
    explained(await workspace.request("query_report", { name: "placed", id: "NEED-ONE" }, records));

    await declare(workspace, `.include "mara.dl"\n.decl early(m: Mid)\nearly(m) :- item(m, _, i, _, _, _), i < "Z".\n`, invariant("early", "p.dl", "early", "warning"));
    explained(await workspace.request("query_report", { name: "early", id: "REQ-A" }));

    const release =
      '\n[gates.release]\nprogram = ".mara/rules/p.dl"\ndescription = "d"\n' +
      'subjects = ["subject"]\nconditions = ["condition", "low"]\ninputs = ["result", "score"]\n';
    await declare(workspace, UNWRITABLE, release + invariant("titled", "p.dl", "titled", "warning"));
    const results = [
      [mid(1), "a\nb"],
      [mid(1), "a\\nb"],
      [mid(1), 'q"x'],
      [mid(1), "plain"],
    ];
    const input = { result: results, score: [[mid(1), -1]] };
    const unwritable = await workspace.request("query_report", { name: "release", id: "REQ-A", input });
    assert.equal(unwritable.rows.filter((row) => row.proof).length, 1, JSON.stringify(unwritable));
    const document = workspace.supplied.files.get("docs/a.mara.md").toString("utf8");
    await workspace.write("docs/a.mara.md", document.replace("### Héllo", '### Say "hi"'));
    const quoted = await workspace.request("query_report", { name: "titled", id: "REQ-B" });
    assert.match(quoted.rows[0].unexplained, /quote/);
    await workspace.write("docs/a.mara.md", document);

    const aggregates = invariant("counted", "p.dl", "counted", "warning") + invariant("crowded", "p.dl", "crowded", "warning");
    await declare(workspace, AGGREGATE, aggregates);
    for (const name of ["counted", "crowded"]) {
      const report = await workspace.request("query_report", { name, id: "NEED-ONE" });
      assert.ok(report.rows.length > 0 && report.rows.every((row) => row.unexplained), JSON.stringify(report));
    }
  });

  test("VER-EXPLANATIONS: an ancestry closure over a chain of 8 links is expanded", async () => {
    const workspace = await Compared.create(page, "programs");
    const schema = workspace.supplied.files.get(".mara/schema.toml").toString("utf8");
    await workspace.write(
      ".mara/schema.toml",
      `${schema}\n[relations.decomposes]\ninverse = "decomposed_by"\n` +
        'description = "The source refines the target."\ntargets = { requirement = ["requirement"] }\n',
    );
    const step = (n) => {
      const row = n === 8 ? "" : `| Decomposes | [C${n + 1}](#${mid(20 + n + 1)}) |\n`;
      return (
        `<a id="${mid(20 + n)}"></a>\n\n### Step ${n}\n\n| Type | Requirement |\n| --- | --- |\n` +
        `| ID | C${n} |\n| Status | Draft |\n${row}\nThe body.\n`
      );
    };
    const chain = Array.from({ length: 9 }, (_, n) => step(n));
    await workspace.write("docs/chain.mara.md", `# Chain\n\n---\n\n${chain.join("\n---\n\n")}`);
    const program = `.include "mara.dl"
.decl above(a: Mid, b: Mid)
above(a, b) :- edge("decomposes", a, b, _, _).
above(a, c) :- above(a, b), edge("decomposes", b, c, _, _).
.decl deep(a: Mid, b: Mid)
deep(a, b) :- above(a, b), item(a, _, "C0", _, _, _), item(b, _, "C8", _, _, _).
`;
    await declare(workspace, program, invariant("deep", "p.dl", "deep", "warning"));
    const report = await workspace.request("query_report", { name: "deep", id: "C0" });
    explained(report);
    const leaves = nodes(report.rows[0].proof).filter((node) => node.relation === "edge");
    assert.equal(leaves.length, 8, JSON.stringify(report));
  });

  test("VER-PROGRAM-FAULTS: each runtime fault makes the outcome unavailable", async () => {
    const workspace = await Compared.create(page, "programs");
    const copies = Array(128).fill("t").join(", ");
    for (const [what, program, reason] of [
      ["to_number of text", '.include "mara.dl"\n.decl bad(m: Mid)\nbad(m) :- field(m, "owner", v), to_number(v) > 0.\n', /evaluation failed/],
      ["substr past the end", '.include "mara.dl"\n.decl bad(m: Mid)\nbad(m) :- title(m, t), substr(t, 100, 1) = "x".\n', /evaluation failed/],
      ["substr splitting a character", '.include "mara.dl"\n.decl bad(m: Mid, s: symbol)\nbad(m, substr(t, 0, 2)) :- title(m, t).\n', /UTF-8/],
      [
        "an output past the size limit",
        `.include "mara.dl"\n.decl n(x: number)\nn(x) :- x = range(0, 140000).\n.decl bad(m: Mid, x: number, s: symbol)\nbad(m, x, "${"x".repeat(1000)}") :- item(m, _, _, "accepted", _, _), n(x).\n`,
        /output-size limit of 64 MiB/,
      ],
      [
        "a symbol past the memory limit",
        `.include "mara.dl"\n.decl s(i: number, t: symbol)\ns(0, "x").\ns(i + 1, cat(t, t)) :- s(i, t), i < 24.\n.decl big(t: symbol)\nbig(cat(${copies})) :- s(24, t).\n.decl bad(m: Mid, c: number)\nbad(m, c) :- item(m, _, _, _, _, _), c = count : big(_).\n`,
        /memory limit of 1024 MiB/,
      ],
    ]) {
      await declare(workspace, program, bothSeverities("p.dl", "bad"));
      const validated = await workspace.browser("project_validate");
      const result = validated.result;
      assert.equal(result?.valid, false, `${what}: ${JSON.stringify(validated)}`);
      const faults = result.diagnostics.filter((d) => d.code === "program-fault");
      assert.equal(faults.length, 1, `${what}: ${JSON.stringify(result)}`);
      const [fault] = faults;
      assert.deepEqual([fault.path, fault.line], [".mara/config.toml", PROGRAMS_LINE], `${what}: ${JSON.stringify(fault)}`);
      assert.match(fault.message, reason, what);
      assert.equal(result.diagnostics.filter((d) => d.code.startsWith("as-")).length, 0, what);
      for (const name of ["query_execute", "query_view"]) {
        const failed = await workspace.browser(name, { name: name === "query_view" ? "as-view" : "as-query" });
        assert.equal(failed.error?.code, "program-failed", `${what}: ${JSON.stringify(failed)}`);
        assert.deepEqual(failed.error.reply.error.diagnostics, [fault], what);
      }
      const gate = (await workspace.browser("query_gate", { name: "as-gate", id: ["REQ-A", "NEED-ONE"] })).result;
      assert.equal(gate?.gate.outcome, "unavailable", `${what}: ${JSON.stringify(gate)}`);
      assert.deepEqual(gate.items.map((item) => item.outcome), ["unavailable", "unavailable"], what);
    }
    // Lenient text functions are no fault: the rows of CLI direct mode.
    await declare(
      workspace,
      admitted(', to_number("7junk") = 7, substr("abcdef", 2, -1) = "cdef"'),
      bothSeverities("p.dl", "bad"),
    );
    const lenient = await workspace.request("project_validate");
    assert.equal(lenient.diagnostics.length, 4, JSON.stringify(lenient));
    await workspace.request("query_execute", { name: "as-query" });
    await workspace.request("query_gate", { name: "as-gate", id: ["REQ-A", "NEED-ONE"] });
  });

  test("VER-PROGRAM-REACH-CASES: integer arithmetic at the limits of 32 bits gives the rows of CLI direct mode", async () => {
    const workspace = await Compared.create(page, "programs");
    const checks =
      ", 2147483647 + 2 = -2147483647, -2147483648 - 2 = 2147483646, 2147483647 * 2 = -2, " +
      "-2147483648 * 2 = 0, 1 bshl 31 = -2147483648, -2147483648 bshr 31 = -1, " +
      '-2147483648 bshru 31 = 1, strlen("hé") = 3, to_number("0x10") = 16';
    await declare(workspace, admitted(checks), bothSeverities("p.dl", "bad"));
    const validated = await workspace.request("project_validate");
    assert.equal(validated.diagnostics.filter((d) => d.code === "as-error").length, 2, JSON.stringify(validated));
    await workspace.request("query_execute", { name: "as-query" });
  });
});
