// The first three rows of the Node and browser table of VER-HOST-COMPARISON
// (docs/verification.mara.md): the daemon case's requests on the 1× corpus through the Node client
// embedded and through the daemon, from Chromium through the daemon's listener, and in a browser
// workspace, each against CLI direct mode, in lockstep. With them, the parity cases of
// VER-NODE-LIBRARY (docs/hosts/verification.mara.md): the embedded client with the package's
// `mara` removed and none on PATH, the client through the daemon with that `mara` made
// non-executable, every call a promise, an undeclared option rejected, and a read after an edit;
// the Chromium case of VER-BROWSER-ACCESS (docs/daemon/verification.mara.md); and the part of the
// first row of VER-UNAVAILABLE-NOT-PARTIAL through the daemon from Chromium.

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmodSync, mkdirSync, mkdtempSync, rmSync, statSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { after, before, describe, test } from "node:test";

import { agentWork } from "../support/agent-work.mjs";
import {
  cli,
  copy,
  encode,
  filesOf,
  fixture,
  launchBrowser,
  openPage,
  requireSouffle,
  startServer,
  testDir,
} from "../support/hosts.mjs";
import {
  clientEnv,
  daemon,
  daemonsOfTest,
  install,
  pathWithoutMara,
  startClient,
  status,
  stopDaemons,
} from "../support/library.mjs";
import {
  Lockstep,
  diskHost,
  printedModule,
  restore,
  toolList,
  workspaceHost,
} from "../support/lockstep.mjs";
import { tools as browserTools } from "../../../../../../packages/mara-browser/daemon.js";

/** The document operations that run in the client's process, never in the daemon. */
const IN_PROCESS = ["project_init", "project_convert"];

/** A new empty directory of the test. */
function emptyDir(name) {
  const parent = join(testDir, "empty");
  mkdirSync(parent, { recursive: true });
  return mkdtempSync(join(parent, `${name}-`));
}

describe("the Node and browser hosts answer as CLI direct mode", { timeout: 1_800_000 }, () => {
  const corpus = fixture("corpus");
  const legacy = fixture("legacy");
  let embedded;
  let through;
  let server;
  let other;
  let browser;
  let page;
  let secondPage;
  let listener;
  let lockstep;
  const roots = {};

  before(async () => {
    requireSouffle();
    roots.baseline = copy(corpus.root);
    roots.embedded = copy(corpus.root);
    roots.daemon = copy(corpus.root);
    roots.http = copy(corpus.root);
    roots.legacy = copy(legacy.root);
    // The embedded client's package holds no `mara`, and none is on PATH.
    embedded = install(emptyDir("embedded"), { binary: false });
    through = install(emptyDir("daemon"));

    server = await startServer();
    other = await startServer();
    browser = await launchBrowser();
    // The daemon of the test, outside every project, listening for the first server's pages.
    listener = daemon(through.mara, [
      "start",
      "--http",
      "--allow-origin",
      server.url,
      "--allow-project",
      roots.http,
      "--allow-project",
      roots.legacy,
    ]);
    assert.equal(typeof listener.token, "string", JSON.stringify(listener));
  });

  after(async () => {
    await page?.close();
    await secondPage?.close();
    await browser?.close();
    await server?.close();
    await other?.close();
    if (through) {
      chmodSync(through.mara, 0o755);
      await stopDaemons([through.mara]);
    }
  });

  test("a page of another origin is refused, and one of the allowed origin is served", async () => {
    // VER-BROWSER-ACCESS's Chromium case: the second page first, then the first.
    const url = `http://127.0.0.1:${listener.port}`;
    secondPage = await openPage(browser, other);
    assert.deepEqual(await secondPage.harness("connect", url, listener.token, roots.http), {});
    const refused = await secondPage.harness("daemonRun", "item_list", { limit: 3 });
    assert.equal(refused.error?.name, "TypeError", JSON.stringify(refused));
    assert.deepEqual(status(through.mara).sessions, [], "a refused request opened a session");

    page = await openPage(browser, server);
    assert.deepEqual(await page.harness("connect", url, listener.token, roots.http), {});
    const served = await page.harness("daemonRun", "item_list", { limit: 3 });
    assert.deepEqual(served, { result: cli(roots.http, "item_list", { limit: 3 }).json });
  });

  test("the daemon case's requests on the 1× corpus give each host direct mode's results", async () => {
    // The session of the daemon client's copy, opened by a CLI read of the package.
    const opened = spawnSync(through.mara, ["--format", "json", "item", "list", "--limit", "1"], {
      cwd: roots.daemon,
      env: clientEnv(),
      encoding: "utf8",
    });
    assert.equal(opened.status, 0, opened.stderr);
    await (async () => {
      const deadline = Date.now() + 10_000;
      while (!status(through.mara).sessions.includes(roots.daemon)) {
        assert.ok(Date.now() < deadline, "the daemon holds no session of the project");
        await new Promise((resolve) => setTimeout(resolve, 20));
      }
    })();
    const pid = status(through.mara).pid;
    const embeddedClient = startClient(embedded, {
      options: { project: roots.embedded, noDaemon: true },
      env: clientEnv({ PATH: pathWithoutMara() }),
    });
    const daemonClient = startClient(through, { options: { project: roots.daemon } });
    try {
      const { mtimeMs, size } = statSync(through.mara);
      // The daemon answered each request: `served` rose by one across it, under the same process
      // (VER-DAEMON-START).
      const served = (executableDuring) => ({
        before: () => {
          const now = status(through.mara);
          assert.equal(now.pid, pid);
          if (!executableDuring) {
            chmodSync(through.mara, 0o644);
          }
          return now.served;
        },
        after: (before, count, shown) => {
          chmodSync(through.mara, 0o755);
          const now = status(through.mara);
          assert.equal(now.pid, pid);
          assert.equal(now.served - before, count, `${shown}: the requests the daemon answered`);
        },
      });
      const hosts = [
        diskHost("embedded", roots.embedded, (name, args) => embeddedClient.run(name, args)),
        diskHost("through the daemon", roots.daemon, (name, args) => daemonClient.run(name, args), {
          // The package's `mara` cannot be executed while the call runs.
          served: served(false),
        }),
        diskHost("Chromium through the daemon", roots.http, (name, args) => page.harness("daemonRun", name, args), {
          served: served(true),
        }),
      ];
      // The browser workspace, supplied with every file of the corpus but `.git`.
      assert.deepEqual(await page.harness("create", encode(filesOf(roots.baseline))), {});
      hosts.push(workspaceHost("a browser workspace", page, filesOf(roots.baseline)));
      lockstep = new Lockstep(roots.baseline, hosts);
      await agentWork(lockstep);

      const tools = toolList(roots.baseline).map((tool) => tool.name);
      const documentOperations = tools.filter((name) => !IN_PROCESS.includes(name));
      assert.deepEqual([...lockstep.sent].sort(), documentOperations.sort(), "the operations sent");

      // Every operation with an undeclared option, in the Node client: rejected before it runs.
      for (const name of tools) {
        for (const client of [embeddedClient, daemonClient]) {
          const before = status(through.mara).served;
          const answer = await client.run(name, { undeclared: "x" });
          assert.equal(answer.promised, true, name);
          assert.equal(answer.error?.code, "invalid-argument", `${name}: ${JSON.stringify(answer)}`);
          assert.equal(answer.error.maraError, true, name);
          assert.equal(status(through.mara).served, before, `${name}: the daemon ran it`);
        }
      }
      const unchanged = statSync(through.mara);
      assert.deepEqual([unchanged.mtimeMs, unchanged.size], [mtimeMs, size]);
    } finally {
      chmodSync(through.mara, 0o755);
      await embeddedClient.close();
      await daemonClient.close();
    }
    assert.deepEqual(daemonsOfTest(), [pid], "the daemon of the test is its only daemon");
  });

  test("the library's types hold what each request of the parity cases printed", () => {
    assert.ok(lockstep?.printed.length > 0, "the parity cases ran no request");
    writeFileSync(join(embedded.dir, "package.json"), JSON.stringify({ type: "module" }));
    writeFileSync(join(embedded.dir, "printed.ts"), printedModule(lockstep.printed));
    const checked = spawnSync(
      process.execPath,
      [process.env.MARA_TSC, "--strict", "--noEmit", "--module", "nodenext", "--moduleResolution", "nodenext", "--target", "es2023", "printed.ts"],
      { cwd: embedded.dir, encoding: "utf8" },
    );
    assert.equal(`${checked.stdout}${checked.stderr}`, "");
    assert.equal(checked.status, 0);
  });

  test("the browser's daemon client declares the parameters of mara mcp's tools", () => {
    const kinds = { string: "string", integer: "integer" };
    for (const tool of toolList(roots.baseline)) {
      const declared = browserTools[tool.name];
      assert.ok(declared, `the browser's client lacks ${tool.name}`);
      const properties = tool.inputSchema.properties;
      const names = [...declared.arguments, ...Object.keys(declared.options)].sort();
      assert.deepEqual(names, Object.keys(properties).sort(), tool.name);
      for (const argument of declared.arguments) {
        assert.equal(properties[argument].type, "string", `${tool.name} ${argument}`);
        assert.ok(tool.inputSchema.required.includes(argument), `${tool.name} ${argument}`);
      }
      for (const [option, kind] of Object.entries(declared.options)) {
        const schema = properties[option];
        const expected =
          schema.type === "array"
            ? `${schema.items.type}s`
            : schema.type === "object"
              ? "inputs"
              : kinds[schema.type];
        assert.equal(kind, expected, `${tool.name} ${option}`);
      }
    }
    assert.equal(Object.keys(browserTools).length, toolList(roots.baseline).length);
  });

  test("project init and convert run in the Node client's process as in direct mode", async () => {
    for (const [installation, noDaemon] of [
      [embedded, true],
      [through, false],
    ]) {
      const before = status(through.mara).served;
      const initialized = emptyDir("init");
      const expectedInit = emptyDir("init");
      const converted = copy(legacy.root);
      const expectedConvert = copy(legacy.root);
      for (const [root, expectedRoot, name] of [
        [initialized, expectedInit, "project_init"],
        [converted, expectedConvert, "project_convert"],
      ]) {
        const client = startClient(installation, { options: { project: root, noDaemon } });
        try {
          const answer = await client.run(name, {});
          const expected = cli(expectedRoot, name, {});
          assert.deepEqual(answer.result ?? answer.error?.reply, expected.json, `${name} (${noDaemon})`);
        } finally {
          await client.close();
        }
        const files = filesOf(root);
        const expectedFiles = filesOf(expectedRoot);
        assert.deepEqual([...files.keys()], [...expectedFiles.keys()], name);
        for (const [path, bytes] of files) {
          assert.ok(bytes.equals(expectedFiles.get(path)), `${name}: ${path}`);
        }
      }
      assert.equal(status(through.mara).served, before, "the daemon ran a request that runs in-process");
    }
  });

  test("Chromium through the daemon reports project init and convert as unavailable", async () => {
    const legacyBefore = filesOf(roots.legacy);
    const httpBefore = filesOf(roots.http);
    for (const [root, name, args] of [
      [roots.http, "project_init", {}],
      [roots.http, "project_init", { template: "engineering" }],
      [roots.http, "project_init", { template: "empty" }],
      [roots.legacy, "project_convert", {}],
    ]) {
      const url = `http://127.0.0.1:${listener.port}`;
      assert.deepEqual(await page.harness("connect", url, listener.token, root), {});
      const before = status(through.mara).served;
      const { error } = await page.harness("daemonRun", name, args);
      assert.equal(error?.code, "unavailable-operation", JSON.stringify(error));
      assert.match(error.message, new RegExp(`\`${name.replace("_", " ")}\``));
      assert.equal(status(through.mara).served, before);
    }
    for (const [root, before] of [
      [roots.legacy, legacyBefore],
      [roots.http, httpBefore],
    ]) {
      const after = filesOf(root);
      assert.deepEqual([...after.keys()], [...before.keys()]);
      for (const [path, bytes] of after) {
        assert.ok(bytes.equals(before.get(path)), path);
      }
    }
  });

  test("the embedded client's read after an edit returns direct mode's result on the edited files", async () => {
    const root = copy(corpus.root);
    const expectedRoot = copy(corpus.root);
    const client = startClient(embedded, { options: { project: root, noDaemon: true } });
    try {
      const document = [...filesOf(root).keys()].find((path) => path.endsWith(".mara.md"));
      const read = { node: document };
      const first = await client.run("get", read);
      assert.deepEqual(first.result, cli(expectedRoot, "get", read).json);
      const edited = Buffer.concat([filesOf(root).get(document), Buffer.from("\nAn edited line.\n")]);
      restore(root, new Map([...filesOf(root), [document, edited]]));
      restore(expectedRoot, new Map([...filesOf(expectedRoot), [document, edited]]));
      const second = await client.run("get", read);
      const expected = cli(expectedRoot, "get", read).json;
      assert.deepEqual(second.result, expected);
      assert.notDeepEqual(second.result, first.result, "the edit changed nothing that the read returns");
    } finally {
      await client.close();
      rmSync(root, { recursive: true });
    }
  });
});
