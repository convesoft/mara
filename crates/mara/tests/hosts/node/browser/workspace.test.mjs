// VER-BROWSER-WORKSPACE (docs/hosts/verification.mara.md): a page of the test works on the files
// of a disposable project in a browser workspace; a copy of the project receives the same
// requests and writes in CLI direct mode, and its files are the expected bytes. The program part
// of the caller's writes comes with Soufflé's module (mara-xsm8w6).

import assert from "node:assert/strict";
import { after, before, describe, test } from "node:test";

import {
  encode,
  isPackageFile,
  launchBrowser,
  openPage,
  pairMids,
  replaceMids,
  startServer,
} from "../support/hosts.mjs";
import { Compared } from "../support/workspace.mjs";

const SCHEMA_WITHOUT_REQUIREMENT = `format_version = 1
statuses = ["draft", "accepted", "retired"]

[types.need]
id_prefix = "NEED-"
description = "A stakeholder capability."
`;

/** An item in the form that \`item create\` writes, with the MID and ID that the caller chose. */
const writtenItem = `
---

<a id="01M7JQ00000000000000000099"></a>

## Written by the caller

| Type | Requirement |
| --- | --- |
| ID | REQ-WRITTEN |
| Status | Draft |

Bytes that the caller wrote.
`;

/** `changes` with each MID that `pairs` holds replaced in their contents. */
function paired(changes, pairs) {
  return changes.map(({ path, base, contents }) => ({
    path,
    base,
    contents:
      contents === null ? null : Buffer.from(replaceMids(Buffer.from(contents, "base64").toString("utf8"), pairs)).toString("base64"),
  }));
}

describe("a browser workspace works on supplied files as CLI direct mode on a copy", { timeout: 300_000 }, () => {
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

  test("the engine runs as WebAssembly in a dedicated worker that the package started", async () => {
    const workspace = await Compared.create(page, "workspace");
    await workspace.request("item_list");
    await workspace.request("get", { node: "REQ-COLD-READ-TIME" });
    assert.deepEqual(page.workers, [`${server.url}/package/worker.js`]);
    assert.deepEqual(await page.harness("wasmCalls"), {
      compile: 0,
      compileStreaming: 0,
      instantiate: 0,
      instantiateStreaming: 0,
      Module: 0,
      Instance: 0,
    });
  });

  test("the change set holds the files that the copy's writes changed, with their supplied bytes' hashes", async () => {
    const workspace = await Compared.create(page, "workspace");
    const draft = ["status=draft"];
    await workspace.request("item_create", {
      type: "requirement",
      path: "docs/reqs.mara.md",
      title: "Search time",
      field: draft,
      body: "A search takes at most 50 ms.",
    });
    await workspace.request("item_create", {
      type: "need",
      path: "docs/more.mara.md",
      title: "Shared work",
      field: draft,
      body: "Teams share their work.",
    });
    await workspace.request("item_update", { item: "REQ-COLD-READ-TIME", title: "Cold read latency" });
    await workspace.request("item_move", { "id-or-mid": "NEED-OFFLINE-WORK", path: "docs/more.mara.md" });
    await workspace.request("relation_add", {
      source: "REQ-WARM-READ-TIME",
      relation: "satisfies",
      target: "NEED-FAST-ANSWERS",
    });
    const changes = await workspace.export();
    const pairs = pairMids(workspace.supplied.files, workspace.expectedFiles(), await workspace.files());
    assert.deepEqual(paired(changes, pairs), workspace.expectedChanges());
    assert.deepEqual(
      changes.map(({ path, base }) => [path, base]),
      [
        ["docs/more.mara.md", null],
        ["docs/needs.mara.md", workspace.supplied.hashes["docs/needs.mara.md"]],
        ["docs/reqs.mara.md", workspace.supplied.hashes["docs/reqs.mara.md"]],
      ],
    );
  });

  test("a document that two writes changed has one change: the second write's bytes, on the supplied bytes", async () => {
    const workspace = await Compared.create(page, "workspace");
    await workspace.request("item_update", { item: "REQ-COLD-READ-TIME", title: "Cold read latency" });
    await workspace.request("item_update", { item: "REQ-COLD-READ-TIME", body: "A cold read takes at most 150 ms." });
    const changes = await workspace.export();
    assert.deepEqual(changes, workspace.expectedChanges());
    assert.deepEqual(
      changes.map(({ path, base }) => [path, base]),
      [["docs/reqs.mara.md", workspace.supplied.hashes["docs/reqs.mara.md"]]],
    );
  });

  test("the caller's removal is a change; a file written back or added and removed is none", async () => {
    const workspace = await Compared.create(page, "workspace");
    const reqs = workspace.supplied.files.get("docs/reqs.mara.md");
    await workspace.remove("docs/needs.mara.md");
    await workspace.write("docs/reqs.mara.md", "# Other bytes\n");
    await workspace.write("docs/reqs.mara.md", reqs);
    await workspace.write("docs/added.mara.md", "# Added\n");
    await workspace.remove("docs/added.mara.md");
    assert.deepEqual(await workspace.export(), [
      { path: "docs/needs.mara.md", base: workspace.supplied.hashes["docs/needs.mara.md"], contents: null },
    ]);
  });

  test("an item that the caller wrote reads as its bytes hold it", async () => {
    const workspace = await Compared.create(page, "workspace");
    const reqs = workspace.supplied.files.get("docs/reqs.mara.md").toString("utf8");
    await workspace.write("docs/reqs.mara.md", reqs + writtenItem);
    const got = await workspace.request("get", { node: "REQ-WRITTEN" });
    assert.equal(got.content, "Bytes that the caller wrote.");
  });

  test("the caller's configuration, schema and .gitignore apply as in CLI direct mode", async () => {
    const workspace = await Compared.create(page, "workspace");
    const config = workspace.supplied.files.get(".mara/config.toml").toString("utf8");
    const narrowed = config.replace('include = ["docs/**/*.mara.md"]', 'include = ["docs/reqs.mara.md"]');
    assert.notEqual(narrowed, config);
    for (const [path, contents] of [
      [".mara/config.toml", narrowed],
      [".mara/schema.toml", SCHEMA_WITHOUT_REQUIREMENT],
      [".gitignore", "docs/reqs.mara.md\n"],
    ]) {
      await workspace.write(path, contents);
      await workspace.request("project_validate");
      await workspace.request("item_list");
    }
  });

  test("an item created inside an item fails as in CLI direct mode and changes nothing", async () => {
    const workspace = await Compared.create(page, "workspace");
    await workspace.request("item_update", { item: "REQ-COLD-READ-TIME", title: "Cold read latency" });
    const before = await workspace.export();
    const { node } = await workspace.request("get", { node: "REQ-WARM-READ-TIME" });
    const failed = await workspace.request("item_create", {
      type: "requirement",
      path: node.path,
      title: "Inside",
      field: ["status=draft"],
      body: "An item inside another.",
      line: node.line + 4,
    });
    assert.equal(failed.error.code, "invalid-position");
    assert.deepEqual(await workspace.export(), before);
  });

  test("two exports give one change set and keep the workspace as it is", async () => {
    const workspace = await Compared.create(page, "workspace");
    for (const title of ["Export time", "Import time"]) {
      await workspace.request("item_create", {
        type: "requirement",
        path: "docs/reqs.mara.md",
        title,
        field: ["status=draft"],
        body: `${title} is short.`,
      });
    }
    const first = await workspace.export();
    assert.deepEqual(await workspace.export(), first);
    const listed = await workspace.request("item_list");
    const ids = JSON.stringify(listed);
    assert.ok(ids.includes("REQ-EXPORT-TIME") && ids.includes("REQ-IMPORT-TIME"), ids);
  });

  test("a directory goes with its last file, so that a file can take its path", async () => {
    const workspace = await Compared.create(page, "workspace");
    await workspace.remove("docs/needs.mara.md");
    await workspace.remove("docs/reqs.mara.md");
    await workspace.write("docs", "No longer a directory.\n");
    assert.deepEqual(await workspace.export(), [
      { path: "docs", base: null, contents: Buffer.from("No longer a directory.\n").toString("base64") },
      { path: "docs/needs.mara.md", base: workspace.supplied.hashes["docs/needs.mara.md"], contents: null },
      { path: "docs/reqs.mara.md", base: workspace.supplied.hashes["docs/reqs.mara.md"], contents: null },
    ]);
  });

  test("arguments that cannot be sent are refused, and the workspace goes on", async () => {
    const workspace = await Compared.create(page, "workspace");
    await workspace.request("item_update", { item: "REQ-COLD-READ-TIME", title: "Cold read latency" });
    const refused = await page.harness("runWithBigInt", "item_list");
    assert.equal(refused.error?.code, "invalid-argument", JSON.stringify(refused));
    await workspace.request("item_list");
    assert.deepEqual(await workspace.export(), workspace.expectedChanges());
  });

  test("bytes from another realm are taken; a path with a NUL is refused", async () => {
    const workspace = await Compared.create(page, "workspace");
    const contents = Buffer.from("# From a frame\n");
    assert.deepEqual(await page.harness("writeFromFrame", "docs/framed.md", contents.toString("base64")), {});
    assert.deepEqual(await workspace.export(), [
      { path: "docs/framed.md", base: null, contents: contents.toString("base64") },
      { path: "docs/framed.md.copy", base: null, contents: contents.toString("base64") },
    ]);
    for (const path of ["docs/a\u0000b.mara.md", "docs/reqs.mara.md\u0000"]) {
      const written = await page.harness("write", path, contents.toString("base64"));
      assert.equal(written.error?.code, "invalid-argument", JSON.stringify(written));
      const removed = await page.harness("remove", path);
      assert.equal(removed.error?.code, "invalid-argument", JSON.stringify(removed));
    }
  });

  test("the module's instance holds one workspace: a second creation in one worker is refused", async () => {
    const files = encode(new Map([["docs/a.mara.md", Buffer.from("# A\n")]]));
    const [first, second] = await page.harness("secondCreate", files);
    assert.deepEqual(first, { failed: false });
    assert.equal(second.failed, true, JSON.stringify(second));
    assert.equal(second.reply.error.code, "invalid-argument");
    assert.match(second.reply.error.message, /holds a workspace already/);
  });

  test("the page kept its buffers, stored nothing and fetched only the package's files", async () => {
    await page.harness("close");
    assert.equal(await page.harness("suppliedIntact"), true);
    assert.deepEqual(await page.harness("storage"), {
      localStorage: 0,
      sessionStorage: 0,
      indexedDB: [],
      caches: [],
      opfs: [],
    });
    const notPackage = (path) => path !== "/" && !isPackageFile(path);
    const origin = new URL(server.url).origin;
    assert.deepEqual(
      page.requests.filter((url) => new URL(url).origin !== origin || notPackage(new URL(url).pathname)),
      [],
      "requests that Playwright reported",
    );
    assert.deepEqual(server.requests.filter(notPackage), [], "requests that the server received");
    assert.ok(server.requests.includes("/package/mara.wasm"), JSON.stringify(server.requests));
  });
});
