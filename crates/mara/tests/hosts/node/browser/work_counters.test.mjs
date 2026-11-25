// The browser workspace's warm rows of VER-WORK-COUNTERS (docs/verification.mara.md) on the 1×
// corpus of the performance harness, read through the workspace's debug stats option
// (DES-PERFORMANCE-HARNESS). Contents the caller supplies or writes are hashed by the engine and
// are no file reads. The rows of programs come with Soufflé's module (mara-xsm8w6).

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { after, before, describe, test } from "node:test";

import { encode, fixture, launchBrowser, openPage, startServer } from "../support/hosts.mjs";

/** No work: every counter 0. */
const NONE = {
  format_version: 1,
  file_reads: 0,
  hashes: 0,
  parses: 0,
  snapshot_builds: 0,
  code_scans: 0,
  subprocess_spawns: 0,
  check_steps: 0,
  program_runs: 0,
};

describe("a browser workspace does only the work that a change calls for", { timeout: 300_000 }, () => {
  let server;
  let browser;
  let page;
  const corpus = fixture("corpus");
  const { document, id } = JSON.parse(readFileSync(join(process.env.MARA_HOSTS_FIXTURES, "corpus.json"), "utf8"));

  /** The stats of the call `name` with `args`, whether it succeeds or fails. */
  async function stats(name, args = {}) {
    const answer = await page.harness("run", name, args, { debugStats: true });
    const stats = answer.stats ?? answer.error?.stats;
    assert.ok(stats, `${name}: no stats: ${JSON.stringify(answer)}`);
    return stats;
  }

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

  test("the same read request again reads, hashes, parses, builds, checks and runs nothing", async () => {
    assert.deepEqual(await page.harness("create", encode(corpus.files)), {});
    const reads = [
      ["project_validate", {}],
      ["item_list", {}],
      ["search", { query: "request" }],
      ["get", { node: id }],
      ["relation_list", { node: id }],
      ["item_validate", { "id-or-mid": id }],
      ["schema_list", {}],
    ];
    for (const [name, args] of reads) {
      await stats(name, args);
      assert.deepEqual(await stats(name, args), NONE, `${name} again`);
    }
  });

  test("a document written by the caller, then a read: 1 hash, 1 parse, 1 snapshot build", async () => {
    assert.deepEqual(await page.harness("create", encode(corpus.files)), {});
    await stats("item_list");
    const edited = Buffer.concat([corpus.files.get(document), Buffer.from("\nEdited by the caller.\n")]);
    assert.deepEqual(await page.harness("write", document, edited.toString("base64")), {});
    assert.deepEqual(await stats("item_list"), { ...NONE, hashes: 1, parses: 1, snapshot_builds: 1 });
  });
});
