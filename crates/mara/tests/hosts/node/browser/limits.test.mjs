// The browser row of VER-EVALUATION-TIME-LIMIT (docs/rules/verification.mara.md): at the time
// limit the workspace's worker terminates Soufflé's worker, which holds nothing of the workspace
// (DES-RULE-EXECUTION). The workspace keeps every unexported edit, and the next request works,
// with a program too; so after a module that aborts, whose worker goes on.

import assert from "node:assert/strict";
import { setTimeout as sleep } from "node:timers/promises";
import { after, before, describe, test } from "node:test";

import { launchBrowser, openPage, requireSouffle, startServer } from "../support/hosts.mjs";
import { PROGRAMS_LINE, admitted, bothSeverities, declare } from "../support/programs.mjs";
import { Compared } from "../support/workspace.mjs";

/**
 * VER-EVALUATION-TIME-LIMIT's long program: a `count` over the four-fold product of a `range` of
 * 1,000 numbers, hours of work in little memory.
 */
const LONG =
  '.include "mara.dl"\n.decl n(x: number)\nn(x) :- x = range(0, 1000).\n.decl bad(m: Mid, c: number)\n' +
  "bad(m, c) :- item(m, _, _, _, _, _), c = count : { n(w), n(x), n(y), n(z), w + x + y + z < 0 }.\n";

/** Waits until `holds()` is true, at most `ms`. */
async function until(holds, ms = 10_000) {
  const end = Date.now() + ms;
  while (!holds() && Date.now() < end) {
    await sleep(20);
  }
  return holds();
}

/** Unexported edits of `workspace`: an item that Mara writes and a document that the caller writes. */
async function edit(workspace) {
  await workspace.request("item_create", {
    type: "requirement",
    path: "docs/a.mara.md",
    title: "Kept",
    field: ["status=draft"],
    body: "An edit that no evaluation may lose.",
  });
  await workspace.write("docs/notes.mara.md", "# Notes\n\nWritten by the caller.\n");
}

/** The only program fault of `validated`, a `project validate` of the workspace that failed. */
function onlyFault(validated) {
  assert.equal(validated.result?.valid, false, JSON.stringify(validated));
  const faults = validated.result.diagnostics.filter((d) => d.code === "program-fault");
  assert.equal(faults.length, 1, JSON.stringify(validated));
  return faults[0];
}

describe("an evaluation stopped in Soufflé's worker loses nothing of the workspace", { timeout: 300_000 }, () => {
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

  test("at the time limit Soufflé's worker is terminated, the edits are kept and the next requests work", async () => {
    const workspace = await Compared.create(page, "programs");
    await edit(workspace);
    await declare(workspace, LONG, bothSeverities("p.dl", "bad"), { timeLimit: 1 });
    const before = await workspace.export();
    const started = Date.now();
    const fault = onlyFault(await workspace.browser("project_validate"));
    const elapsed = Date.now() - started;
    assert.ok(elapsed >= 1000 && elapsed < 10_000, `${elapsed} ms`);
    assert.deepEqual([fault.path, fault.line], [".mara/config.toml", PROGRAMS_LINE]);
    assert.match(fault.message, /time limit of 1 s/);
    const souffle = page.souffleWorkers();
    assert.equal(souffle.length, 1, JSON.stringify(souffle));
    assert.ok(await until(() => souffle[0].closed), "Soufflé's worker was not terminated");
    assert.deepEqual(await workspace.export(), before);
    await workspace.request("item_list");
    // A program that ends: another Soufflé worker evaluates it.
    await declare(workspace, admitted(""), bothSeverities("p.dl", "bad"), { timeLimit: 1 });
    const validated = await workspace.request("project_validate");
    assert.equal(validated.diagnostics.filter((d) => d.code === "as-error").length, 2, JSON.stringify(validated));
    assert.equal(page.souffleWorkers().length, 2);
    await workspace.sameFiles();
  });

  test("a module that aborts is a fault; its worker goes on, the edits are kept and the next requests work", async () => {
    const workspace = await Compared.create(page, "programs");
    await edit(workspace);
    const program = '.include "mara.dl"\n.decl bad(m: Mid)\nbad(m) :- field(m, "owner", v), to_number(v) > 0.\n';
    await declare(workspace, program, bothSeverities("p.dl", "bad"));
    const before = await workspace.export();
    const workers = page.souffleWorkers().length;
    const fault = onlyFault(await workspace.browser("project_validate"));
    assert.deepEqual([fault.path, fault.line], [".mara/config.toml", PROGRAMS_LINE]);
    assert.match(fault.message, /Aborted/);
    assert.deepEqual(await workspace.export(), before);
    await workspace.request("item_list");
    await declare(workspace, admitted(""), bothSeverities("p.dl", "bad"));
    await workspace.request("project_validate");
    const started = page.souffleWorkers().slice(workers);
    assert.deepEqual(
      started.map((state) => state.closed),
      [false],
      "one Soufflé worker ran both evaluations",
    );
    await workspace.sameFiles();
  });
});
