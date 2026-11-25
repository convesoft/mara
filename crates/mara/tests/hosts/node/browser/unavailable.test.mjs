// The browser workspace's part of VER-UNAVAILABLE-NOT-PARTIAL (docs/verification.mara.md): what
// the browser cannot do is reported as unavailable, never as a partial result. The daemon's part
// of the first row comes with the browser's daemon client (mara-vfkgbs).

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { after, before, describe, test } from "node:test";

import { launchBrowser, openPage, requireSouffle, startServer } from "../support/hosts.mjs";
import { PROGRAMS_LINE, admitted, bothSeverities, declare } from "../support/programs.mjs";
import { Compared } from "../support/workspace.mjs";

/** A program that reads code links, through which an operation reads code only for it. */
const CODE_PROGRAM = `.include "mara.dl"
.decl implemented(m: Mid, path: Path, line: number)
implemented(m, p, l) :- code_link("implements", m, p, l, _, _).
`;

const CODE_ENTRIES =
  '\n[rules]\nprograms = [".mara/rules/*.dl"]\n\n[queries.implemented]\nprogram = ".mara/rules/code.dl"\n' +
  'relation = "implemented"\ndescription = "d"\n\n[views.implemented-view]\nprogram = ".mara/rules/code.dl"\n' +
  'relation = "implemented"\ndescription = "d"\n\n[gates.implemented-gate]\nprogram = ".mara/rules/code.dl"\n' +
  'description = "d"\nsubjects = ["implemented"]\nconditions = ["implemented"]\n\n' +
  '[invariants.unimplemented]\nprogram = ".mara/rules/code.dl"\n' +
  'relation = "implemented"\nseverity = "warning"\nmessage = "implemented"\n';

/**
 * `value` with the message of each `unavailable-grammar` diagnostic left out: the workspace's
 * says that the browser cannot load grammars.
 */
function grammarMessages(value, browser) {
  if (Array.isArray(value)) {
    return value.map((element) => grammarMessages(element, browser));
  }
  if (value && typeof value === "object") {
    const unavailable = value.code === "unavailable-grammar";
    if (unavailable && browser) {
      assert.match(value.message, /the browser cannot load grammars/);
    }
    return Object.fromEntries(
      Object.entries(value)
        .filter(([key]) => !(unavailable && key === "message"))
        .map(([key, member]) => [key, grammarMessages(member, browser)]),
    );
  }
  return value;
}

/** The `unavailable-grammar` diagnostics that `value` holds. */
function unavailableGrammars(value) {
  if (Array.isArray(value)) {
    return value.flatMap(unavailableGrammars);
  }
  if (value && typeof value === "object") {
    const own = value.code === "unavailable-grammar" ? [value] : [];
    return own.concat(Object.values(value).flatMap(unavailableGrammars));
  }
  return [];
}

describe("a browser workspace reports what it cannot do as unavailable", { timeout: 300_000 }, () => {
  let server;
  let browser;
  let page;

  before(async () => {
    server = await startServer();
    browser = await launchBrowser();
    page = await openPage(browser, server);
  });

  after(async () => {
    await page?.close();
    await browser?.close();
    await server?.close();
  });

  for (const [fixture, name, args] of [
    ["workspace", "project_init", {}],
    ["workspace", "project_init", [1]],
    ["workspace", "project_init", { template: "engineering" }],
    ["workspace", "project_init", { template: "empty" }],
    ["legacy", "project_convert", {}],
  ]) {
    test(`${name} ${JSON.stringify(args)} fails with unavailable-operation and changes nothing`, async () => {
      const workspace = await Compared.create(page, fixture);
      const { error } = await workspace.browser(name, args);
      assert.equal(error?.code, "unavailable-operation", JSON.stringify(error));
      assert.match(error.message, new RegExp(`\`${name.replace("_", " ")}\``));
      assert.deepEqual(await workspace.export(), []);
    });
  }

  test("each operation that reads code gives CLI direct mode's result without the grammars", async () => {
    const workspace = await Compared.create(page, "grammars");
    const requests = [
      ["relation_list", { node: "REQ-LOGIN", relation: ["implemented_by"] }],
      ["relation_list", { node: "src/login.rs:12" }],
      ["get", { node: "src/login.rs:12" }],
      ["get", { node: "scripts/tool.py" }],
      ["project_validate", {}],
      ["item_rename", { "id-or-mid": "REQ-AUDIT", "new-id": "REQ-AUDITED" }],
      ["item_delete", { item: "REQ-LOGIN-AUDIT" }],
      ["project_validate", {}],
    ];
    let reported = 0;
    for (const [name, args] of requests) {
      const expected = await workspace.request(name, args, grammarMessages);
      reported += unavailableGrammars(expected).length;
      if (name === "project_validate") {
        assert.equal(expected.valid, false, JSON.stringify(expected));
      }
    }
    assert.ok(reported > 0, "the requests reported no unavailable grammar");
    await workspace.sameFiles();
  });

  test("each operation that reads code only for a program gives CLI direct mode's result without the grammars", async () => {
    requireSouffle();
    const workspace = await Compared.create(page, "grammars");
    const config = workspace.supplied.files.get(".mara/config.toml").toString("utf8");
    await workspace.write(".mara/rules/code.dl", CODE_PROGRAM);
    await workspace.write(".mara/config.toml", config + CODE_ENTRIES);
    let reported = 0;
    let rows = 0;
    for (const [name, args] of [
      ["project_validate", {}],
      ["query_execute", { name: "implemented" }],
      ["query_view", { name: "implemented-view" }],
      ["query_gate", { name: "implemented-gate", id: ["REQ-LOGIN", "REQ-AUDIT"] }],
      ["item_validate", { "id-or-mid": "REQ-LOGIN" }],
      ["query_report", { name: "unimplemented", id: "REQ-LOGIN" }],
    ]) {
      const expected = await workspace.request(name, args, grammarMessages);
      reported += unavailableGrammars(expected).length;
      rows += (expected.rows ?? []).length;
    }
    assert.ok(reported > 0, "the requests reported no unavailable grammar");
    assert.ok(rows > 0, "the program found no code link");
  });

  for (const [what, answer] of [
    ["answered with status 404", (response) => response.writeHead(404).end()],
    [
      "truncated",
      (response) => {
        const module = readFileSync(join(process.env.MARA_BROWSER_PACKAGE, "souffle.wasm"));
        response.writeHead(200, { "content-type": "application/wasm" });
        response.end(module.subarray(0, module.length >> 1));
      },
    ],
  ]) {
    test(`a Soufflé module ${what} is a fault without a line for each declared program`, async () => {
      requireSouffle();
      server.answer("/package/souffle.wasm", answer);
      // A page of its own, in a new context, which has fetched no module before.
      const fresh = await openPage(browser, server);
      try {
        const workspace = await Compared.create(fresh, "programs");
        await workspace.write(".mara/rules/q.dl", '.include "mara.dl"\n.decl other(m: Mid)\nother(m) :- item(m, _, _, _, _, _).\n');
        await declare(workspace, admitted(""), bothSeverities("p.dl", "bad"));
        const before = await workspace.export();
        const validated = (await workspace.browser("project_validate")).result;
        assert.equal(validated?.valid, false, JSON.stringify(validated));
        assert.deepEqual(
          validated.diagnostics.map((d) => [d.code, d.path, d.line, d.message.split(":")[0]]),
          [
            ["program-fault", ".mara/config.toml", PROGRAMS_LINE, ".mara/rules/p.dl"],
            ["program-fault", ".mara/config.toml", PROGRAMS_LINE, ".mara/rules/q.dl"],
          ],
          JSON.stringify(validated),
        );
        for (const diagnostic of validated.diagnostics) {
          assert.match(diagnostic.message, /Soufflé's WebAssembly module/);
        }
        for (const [name, entry] of [
          ["query_execute", "as-query"],
          ["query_view", "as-view"],
        ]) {
          const failed = await workspace.browser(name, { name: entry });
          assert.equal(failed.error?.code, "program-failed", JSON.stringify(failed));
          assert.deepEqual(failed.error.reply.error.diagnostics, [validated.diagnostics[0]]);
        }
        const gate = (await workspace.browser("query_gate", { name: "as-gate", id: ["REQ-A", "NEED-ONE"] })).result;
        assert.equal(gate?.gate.outcome, "unavailable", JSON.stringify(gate));
        assert.deepEqual(gate.items.map((item) => item.outcome), ["unavailable", "unavailable"]);
        assert.deepEqual(await workspace.export(), before);
      } finally {
        server.answer("/package/souffle.wasm", null);
        await fresh.close();
      }
    });
  }
});
