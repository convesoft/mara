// The browser workspace's part of the first two rows of VER-UNAVAILABLE-NOT-PARTIAL
// (docs/verification.mara.md): what the browser cannot do is reported as unavailable, never as
// a partial result. The daemon's part of the first row, and the Soufflé module's row, come with
// the browser's daemon client (mara-vfkgbs) and Soufflé's module (mara-xsm8w6).

import assert from "node:assert/strict";
import { after, before, describe, test } from "node:test";

import { launchBrowser, openPage, startServer } from "../support/hosts.mjs";
import { Compared } from "../support/workspace.mjs";

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
});
