// The first three rows of the Node and browser table of VER-HOST-COMPARISON
// (docs/verification.mara.md): the daemon case's requests on the 1× corpus through the Node client
// embedded and through the daemon, then from Chromium through the daemon's listener and in a
// browser workspace, each against CLI direct mode, in lockstep. With them, the parity cases of
// VER-NODE-LIBRARY (docs/hosts/verification.mara.md): the embedded client with the package's
// `mara` removed and none on PATH, the client through the daemon with that `mara` made
// non-executable, every call a promise, an undeclared option rejected, and a read after an edit;
// the Chromium case of VER-BROWSER-ACCESS (docs/daemon/verification.mara.md); and the part of the
// first row of VER-UNAVAILABLE-NOT-PARTIAL through the daemon from Chromium.
//
// A case needs only what its host uses (VER-HOST-COMPARISON): the Node cases the addon, which
// `node()` installs, and the browser cases Chromium and the browser package, which `page()`
// opens. Each is made on the first case that needs it, so a missing one fails only those cases.

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
  waitFor,
} from "../support/library.mjs";
import {
  Lockstep,
  diskHost,
  mcpDirect,
  printedModule,
  restore,
  toolList,
  workspaceHost,
} from "../support/lockstep.mjs";
import { tools as browserTools } from "../../../../../../packages/mara-browser/daemon.js";

/** The build's `mara`, whose daemon listens for the test's pages: no case of it needs the addon. */
const maraBin = process.env.MARA_BIN;

/** The document operations that run in the client's process, never in the daemon. */
const IN_PROCESS = ["project_init", "project_convert"];

/** The work of a call that ran nothing (DES-PERFORMANCE-HARNESS). */
const NO_WORK = {
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

/** A new empty directory of the test. */
function emptyDir(name) {
  const parent = join(testDir, "empty");
  mkdirSync(parent, { recursive: true });
  return mkdtempSync(join(parent, `${name}-`));
}

/** `make()` once, on the first call; its failure fails every later call too. */
function once(make) {
  let made;
  return () => (made ??= make());
}

/** The files of `root` must be `before`, byte for byte. */
function unchanged(root, before) {
  const after = filesOf(root);
  assert.deepEqual([...after.keys()], [...before.keys()]);
  for (const [path, bytes] of after) {
    assert.ok(bytes.equals(before.get(path)), path);
  }
}

/**
 * The daemon's `served` rose by `count` across each call, under the same process
 * (VER-DAEMON-START). With `nonExecutable`, `mara` cannot be executed while the call runs.
 */
function servedBy(mara, pid, { nonExecutable = false } = {}) {
  return {
    before: () => {
      const now = status(mara);
      assert.equal(now.pid, pid);
      if (nonExecutable) {
        chmodSync(mara, 0o644);
      }
      return now.served;
    },
    after: (before, count, shown) => {
      chmodSync(mara, 0o755);
      const now = status(mara);
      assert.equal(now.pid, pid);
      assert.equal(now.served - before, count, `${shown}: the requests the daemon answered`);
    },
  };
}

describe("the Node and browser hosts answer as CLI direct mode", { timeout: 1_800_000 }, () => {
  const corpus = fixture("corpus");
  const legacy = fixture("legacy");
  const roots = {};
  let server;
  let other;
  let listener;
  let browser;
  const pages = [];
  /** What each request of the first lockstep that ran printed in direct mode. */
  let printed;

  /** The Node library installed twice: without `mara`, and with a copy of its own. */
  const node = once(async () => ({
    embedded: install(emptyDir("embedded"), { binary: false }),
    through: install(emptyDir("daemon")),
  }));

  /** A page of `on`, a server of the test, in Chromium, which loads the browser package. */
  const open = async (on) => {
    requireSouffle();
    browser ??= await launchBrowser();
    const page = await openPage(browser, on);
    pages.push(page);
    return page;
  };
  const page = once(() => open(server));
  const url = () => `http://127.0.0.1:${listener.port}`;

  before(async () => {
    roots.node = copy(corpus.root);
    roots.embedded = copy(corpus.root);
    roots.daemon = copy(corpus.root);
    roots.browser = copy(corpus.root);
    roots.http = copy(corpus.root);
    roots.legacy = copy(legacy.root);
    server = await startServer();
    other = await startServer();
    // The listening daemon, of the build's `mara`, outside every project, for the first
    // server's pages.
    listener = daemon(maraBin, [
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
    for (const page of pages) {
      await page.close();
    }
    await browser?.close();
    await server?.close();
    await other?.close();
    const binaries = [maraBin];
    try {
      const { through } = await node();
      chmodSync(through.mara, 0o755);
      binaries.push(through.mara);
    } catch {
      // No installation: its daemon never ran.
    }
    await stopDaemons(binaries);
  });

  test("a page of another origin is refused, and one of the allowed origin is served", async () => {
    // VER-BROWSER-ACCESS's Chromium case: the second page first, then the first.
    const second = await open(other);
    assert.deepEqual(await second.harness("connect", url(), listener.token, roots.http), {});
    const refused = await second.harness("daemonRun", "item_list", { limit: 3 });
    assert.equal(refused.error?.name, "TypeError", JSON.stringify(refused));
    assert.deepEqual(status(maraBin).sessions, [], "a refused request opened a session");

    const first = await page();
    assert.deepEqual(await first.harness("connect", url(), listener.token, roots.http), {});
    const served = await first.harness("daemonRun", "item_list", { limit: 3 });
    assert.deepEqual(served, { result: cli(roots.http, "item_list", { limit: 3 }).json });
  });

  test("the daemon case's requests give the Node client direct mode's results", async () => {
    const { embedded, through } = await node();
    daemon(through.mara, ["start"]);
    // The session of the daemon client's copy, opened by a CLI read of the package.
    const opened = spawnSync(through.mara, ["--format", "json", "item", "list", "--limit", "1"], {
      cwd: roots.daemon,
      env: clientEnv(),
      encoding: "utf8",
    });
    assert.equal(opened.status, 0, opened.stderr);
    await waitFor("the daemon's session", () =>
      status(through.mara).sessions.includes(roots.daemon) ? true : undefined,
    );
    const pid = status(through.mara).pid;
    const embeddedClient = startClient(embedded, {
      options: { project: roots.embedded, noDaemon: true },
      env: clientEnv({ PATH: pathWithoutMara() }),
    });
    const daemonClient = startClient(through, { options: { project: roots.daemon } });
    try {
      const { mtimeMs, size } = statSync(through.mara);
      const lockstep = new Lockstep(roots.node, [
        diskHost("embedded", roots.embedded, (name, args) => embeddedClient.run(name, args)),
        diskHost("through the daemon", roots.daemon, (name, args) => daemonClient.run(name, args), {
          served: servedBy(through.mara, pid, { nonExecutable: true }),
        }),
      ]);
      await agentWork(lockstep);
      printed ??= lockstep.printed;

      const tools = toolList(roots.node).map((tool) => tool.name);
      const documentOperations = tools.filter((name) => !IN_PROCESS.includes(name));
      assert.deepEqual([...lockstep.sent].sort(), documentOperations.sort(), "the operations sent");

      // Every operation with an undeclared option: rejected before it runs.
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
      const unchangedBinary = statSync(through.mara);
      assert.deepEqual([unchangedBinary.mtimeMs, unchangedBinary.size], [mtimeMs, size]);
    } finally {
      chmodSync(through.mara, 0o755);
      await embeddedClient.close();
      await daemonClient.close();
    }
    assert.equal(status(through.mara).pid, pid, "the package's daemon is the one that served");
    assert.ok(daemonsOfTest().includes(pid));
  });

  test("the daemon case's requests give Chromium through the daemon and a workspace direct mode's results", async () => {
    const browserPage = await page();
    const pid = status(maraBin).pid;
    assert.deepEqual(await browserPage.harness("connect", url(), listener.token, roots.http), {});
    const supplied = filesOf(roots.browser);
    assert.deepEqual(await browserPage.harness("create", encode(supplied)), {});
    const lockstep = new Lockstep(roots.browser, [
      diskHost("Chromium through the daemon", roots.http, (name, args) => browserPage.harness("daemonRun", name, args), {
        served: servedBy(maraBin, pid),
      }),
      workspaceHost("a browser workspace", browserPage, supplied),
    ]);
    await agentWork(lockstep);
    printed ??= lockstep.printed;
    const documentOperations = toolList(roots.browser)
      .map((tool) => tool.name)
      .filter((name) => !IN_PROCESS.includes(name));
    assert.deepEqual([...lockstep.sent].sort(), documentOperations.sort(), "the operations sent");
  });

  test("the library's types hold what each request of the parity cases printed", async () => {
    assert.ok(printed?.length > 0, "no parity case ran a request");
    // Only the declarations: an installation without the native package's files would do.
    const dir = emptyDir("printed");
    const { dispatcher } = install(dir, { binary: false, addon: false });
    assert.ok(dispatcher);
    writeFileSync(join(dir, "package.json"), JSON.stringify({ type: "module" }));
    writeFileSync(join(dir, "printed.ts"), printedModule(printed));
    const checked = spawnSync(
      process.execPath,
      [process.env.MARA_TSC, "--strict", "--noEmit", "--module", "nodenext", "--moduleResolution", "nodenext", "--target", "es2023", "printed.ts"],
      { cwd: dir, encoding: "utf8" },
    );
    assert.equal(`${checked.stdout}${checked.stderr}`, "");
    assert.equal(checked.status, 0);
  });

  test("the browser's daemon client declares the parameters of mara mcp's tools", () => {
    const kinds = { string: "string", integer: "integer" };
    const tools = toolList(roots.legacy);
    for (const tool of tools) {
      const declared = browserTools[tool.name];
      assert.ok(declared, `the browser's client lacks ${tool.name}`);
      const properties = tool.inputSchema.properties;
      const names = [...declared.arguments, ...Object.keys(declared.options)].sort();
      assert.deepEqual(names, Object.keys(properties).sort(), tool.name);
      for (const argument of declared.arguments) {
        assert.equal(properties[argument].type, "string", `${tool.name} ${argument}`);
      }
      // Required: the positional arguments, then the required options.
      const required = tool.inputSchema.required ?? [];
      assert.deepEqual(
        [...required].sort(),
        [...declared.arguments, ...(declared.required ?? [])].sort(),
        `${tool.name}: the required parameters`,
      );
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
    assert.equal(Object.keys(browserTools).length, tools.length);
  });

  test("project init and convert run in the Node client's process as in direct mode", async () => {
    const { embedded, through } = await node();
    for (const [installation, noDaemon] of [
      [embedded, true],
      [through, false],
    ]) {
      const daemons = daemonsOfTest();
      for (const [name, source] of [
        ["project_init", null],
        ["project_convert", legacy.root],
      ]) {
        const root = source ? copy(source) : emptyDir("init");
        const expectedRoot = source ? copy(source) : emptyDir("init");
        const client = startClient(installation, { options: { project: root, noDaemon } });
        try {
          const answer = await client.run(name, {});
          assert.equal(answer.promised, true, name);
          const expected = cli(expectedRoot, name, {});
          assert.deepEqual(answer.result ?? answer.error?.reply, expected.json, `${name} (${noDaemon})`);
        } finally {
          await client.close();
        }
        unchanged(root, filesOf(expectedRoot));
      }
      assert.deepEqual(daemonsOfTest(), daemons, "a request that runs in-process reached a daemon");
    }
  });

  test("Chromium through the daemon reports project init and convert as unavailable", async () => {
    const browserPage = await page();
    const legacyBefore = filesOf(roots.legacy);
    const httpBefore = filesOf(roots.http);
    for (const [root, name, args] of [
      [roots.http, "project_init", {}],
      [roots.http, "project_init", { template: "engineering" }],
      [roots.http, "project_init", { template: "empty" }],
      // Arguments of no shape: the operation is unavailable before they are read.
      [roots.http, "project_init", [1]],
      [roots.legacy, "project_convert", {}],
    ]) {
      assert.deepEqual(await browserPage.harness("connect", url(), listener.token, root), {});
      const before = status(maraBin).served;
      const { error } = await browserPage.harness("daemonRun", name, args);
      assert.equal(error?.code, "unavailable-operation", JSON.stringify(error));
      assert.match(error.message, new RegExp(`\`${name.replace("_", " ")}\``));
      assert.equal(status(maraBin).served, before);
    }
    unchanged(roots.legacy, legacyBefore);
    unchanged(roots.http, httpBefore);
  });

  test("a failure that ran nothing reports no work when the call asked for debug stats", async () => {
    const { embedded } = await node();
    // The library refuses an option of the client itself; the addon an undeclared option.
    const refusedClient = startClient(embedded, { options: { project: 5 } });
    const client = startClient(embedded, { options: { project: roots.legacy, noDaemon: true } });
    try {
      const refused = await refusedClient.run("item_list", {}, { debugStats: true });
      assert.equal(refused.error?.code, "invalid-argument", JSON.stringify(refused));
      assert.deepEqual(refused.error.stats, NO_WORK);
      const undeclared = await client.run("item_list", { undeclared: 1 }, { debugStats: true });
      assert.equal(undeclared.error?.code, "invalid-argument", JSON.stringify(undeclared));
      assert.deepEqual(undeclared.error.stats, NO_WORK);
    } finally {
      await refusedClient.close();
      await client.close();
    }
  });

  test("the browser's daemon client reports no work for a failure that ran nothing", async () => {
    const browserPage = await page();
    assert.deepEqual(await browserPage.harness("connect", url(), listener.token, roots.legacy), {});
    const before = status(maraBin).served;
    const undeclared = await browserPage.harness("daemonRun", "item_list", { undeclared: 1 }, { debugStats: true });
    assert.equal(undeclared.error?.code, "invalid-argument", JSON.stringify(undeclared));
    assert.deepEqual(undeclared.error.stats, NO_WORK);
    // A value that the listener refuses, as `mara mcp` refuses it.
    const value = await browserPage.harness("daemonRun", "item_list", { limit: 0 }, { debugStats: true });
    assert.equal(value.error?.code, "invalid-argument", JSON.stringify(value));
    assert.equal(value.error.message, mcpDirect(roots.legacy, "item_list", { limit: 0 }).json.error.message);
    assert.deepEqual(value.error.stats, NO_WORK);
    // A refusal of the listener: another token.
    assert.deepEqual(await browserPage.harness("connect", url(), "0".repeat(64), roots.legacy), {});
    const token = await browserPage.harness("daemonRun", "item_list", {}, { debugStats: true });
    assert.equal(token.error?.code, "refused", JSON.stringify(token));
    assert.deepEqual(token.error.stats, NO_WORK);
    assert.equal(status(maraBin).served, before);
  });

  test("an unbound client runs each call in the working directory it was made in", async () => {
    const { embedded } = await node();
    const a = copy(corpus.root);
    const b = emptyDir("other");
    assert.equal(cli(b, "project_init", {}).status, 0);
    const client = startClient(embedded, { cwd: a, options: { noDaemon: true } });
    try {
      // More calls than libuv's pool has threads, so that some wait for one after the change.
      const calls = Array.from({ length: 8 }, () => client.run("item_list", { limit: 3 }));
      await client.chdir(b);
      const expected = cli(a, "item_list", { limit: 3 }).json;
      for (const answer of await Promise.all(calls)) {
        assert.deepEqual(answer.result, expected);
      }
      assert.notDeepEqual(cli(b, "item_list", { limit: 3 }).json, expected);
    } finally {
      await client.close();
    }
  });

  test("the embedded client's read after an edit returns direct mode's result on the edited files", async () => {
    const { embedded } = await node();
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
