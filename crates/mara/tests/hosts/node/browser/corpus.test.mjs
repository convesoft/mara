// The 1× corpus row of VER-HOST-COMPARISON (docs/verification.mara.md): a browser workspace
// supplied with the corpus of the performance harness runs every query and view of the
// `engineering` template, `query gate` with the corpus's gate inputs and `query report` of its
// failing row, and gives the rows of CLI direct mode: Soufflé's WebAssembly and native builds
// give the same outputs.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { after, before, describe, test } from "node:test";

import { cli, launchBrowser, openPage, requireSouffle, startServer } from "../support/hosts.mjs";
import { Compared } from "../support/workspace.mjs";

describe("a browser workspace evaluates the engineering template as CLI direct mode", { timeout: 600_000 }, () => {
  let server;
  let browser;
  let page;
  const described = JSON.parse(readFileSync(join(process.env.MARA_HOSTS_FIXTURES, "corpus.json"), "utf8"));

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

  test("every query and view, the gate with the corpus's inputs and the report of its failing row", async () => {
    const workspace = await Compared.create(page, "corpus");
    const results = JSON.parse(workspace.supplied.files.get(described.results).toString("utf8"));
    const supplied = {
      candidate: described.candidate,
      verification_result: results,
      validation_context: [],
      validation_result: [],
    };
    const entries = cli(workspace.root, "query_list").json.entries;
    let rows = 0;
    for (const entry of entries.filter(({ kind }) => kind === "query" || kind === "view")) {
      const input = Object.fromEntries(entry.inputs.map((name) => [name, supplied[name]]));
      const args = { name: entry.name, ...(entry.inputs.length > 0 ? { input } : {}), limit: 100 };
      const operation = entry.kind === "view" ? "query_view" : "query_execute";
      // Every page, each read with the cursor that the previous page gave.
      for (let cursor = null, first = true; first || cursor !== null; first = false) {
        const expected = await workspace.request(operation, { ...args, cursor });
        assert.equal(expected.error, undefined, `${entry.name}: ${JSON.stringify(expected.error)}`);
        rows += (expected.rows ?? []).length;
        cursor = expected.next_cursor ?? null;
      }
    }
    assert.ok(rows > 0, "the queries and views gave no row");
    const gate = await workspace.request("query_gate", {
      name: "verified",
      id: [described.gate_requirement],
      input: { candidate: described.candidate, verification_result: results },
      complete: ["verification_result"],
    });
    assert.equal(gate.gate.outcome, "passed", JSON.stringify(gate));
    const report = await workspace.request("query_report", {
      name: "implementation-ready",
      id: described.retired_requirement,
    });
    assert.equal(report.report.outcome, "failed", JSON.stringify(report));
  });
});
