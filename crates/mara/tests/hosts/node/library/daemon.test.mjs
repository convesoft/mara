// The Node rows of VER-DAEMON-START and VER-DAEMON-WRITES (docs/daemon/verification.mara.md) and
// the embedded Node row of VER-CONCURRENT-WRITERS (docs/authoring/verification.mara.md): the
// Node client is a client of the daemon of its package's `mara`, as that package's CLI is.
//
// A call is served in-process when its debug stats hold the counts of a cold request, those of
// the same read in CLI direct mode, and no daemon of the test can have answered it: none ran. A
// call is served by the daemon when `served` of `mara daemon status` rose by exactly one across
// it, under the same process (VER-DAEMON-START).

import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import {
  chmodSync,
  copyFileSync,
  mkdtempSync,
  readFileSync,
  renameSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { join } from "node:path";
import { setTimeout as sleep } from "node:timers/promises";
import { after, describe, test } from "node:test";

import { cliStats, copy, filesOf, fixture, testDir } from "../support/hosts.mjs";
import {
  clientEnv,
  daemon,
  daemonsOfTest,
  install,
  startClient,
  status,
  stopDaemons,
  waitFor,
} from "../support/library.mjs";

const maraBin = process.env.MARA_BIN;

/** The read of the cases. */
const READ = ["item_list", { limit: 5 }];

/** A new directory of the test. */
const dirOf = (name) => mkdtempSync(join(testDir, `${name}-`));

/**
 * A copy of the fixture `workspace` with `REQ-TWO` in `docs/reqs.mara.md`, as the CLI writes it,
 * whose files last changed at least 2 s before this returns (VER-DAEMON-START).
 */
async function project() {
  const root = copy(fixture("workspace").root);
  const created = spawnSync(
    maraBin,
    ["item", "create", "requirement", "docs/reqs.mara.md", "--id", "REQ-TWO", "--title", "Two"]
      .concat(["--field", "status=draft", "--body", "Text."]),
    { cwd: root, env: clientEnv({ MARA_NO_DAEMON: "1" }), encoding: "utf8" },
  );
  assert.equal(created.status, 0, created.stderr);
  await sleep(2000);
  return root;
}

/** The debug stats of `READ` in CLI direct mode in `root`: the counts of a cold request. */
function cold(root) {
  const { format_version, ...counts } = cliStats(root, ...READ);
  return { format_version, ...counts };
}

/** No daemon of the test starts within `ms`. */
async function noneStarts(ms = 1000) {
  const end = Date.now() + ms;
  while (Date.now() < end) {
    assert.deepEqual(daemonsOfTest(), [], "a daemon of the test started");
    await sleep(20);
  }
}

/** Runs `script` with Node in `cwd`, a module beside the installation's `library.mjs`. */
function node(installation, script, cwd, env = clientEnv()) {
  const file = join(installation.dir, `script-${Date.now()}-${Math.random()}.mjs`);
  writeFileSync(file, script);
  return spawn(process.execPath, [file], { cwd, env, stdio: ["ignore", "pipe", "pipe"] });
}

describe("the Node client is a client of its package's daemon", { timeout: 600_000 }, () => {
  const installation = install(dirOf("package"));
  const copied = install(dirOf("copy"));

  after(async () => {
    await stopDaemons([installation.mara, copied.mara]);
  });

  test("a read with no daemon running is served in-process and starts the package's daemon", async () => {
    const root = await project();
    assert.deepEqual(daemonsOfTest(), []);
    const child = node(
      installation,
      'import { createClient } from "./library.mjs";\n' +
        `const answer = await createClient().run(${JSON.stringify(READ[0])}, ${JSON.stringify(READ[1])}, { debugStats: true });\n` +
        "console.log(JSON.stringify({ settled: Date.now(), stats: answer.stats }));\n",
      root,
    );
    let printed = "";
    child.stdout.on("data", (chunk) => (printed += chunk));
    let errors = "";
    child.stderr.on("data", (chunk) => (errors += chunk));
    const code = await new Promise((resolve) => child.on("exit", resolve));
    const exited = Date.now();
    assert.equal(code, 0, errors);
    const { settled, stats } = JSON.parse(printed);
    assert.deepEqual(stats, cold(root), "the counts of a cold request");
    assert.ok(exited - settled < 5000, `Node exited ${exited - settled} ms after the call settled`);
    await waitFor("the daemon's session", () => (status(installation.mara).sessions.includes(root) ? true : undefined));

    // The reads of the second case: 2 s apart, once that session is open.
    const client = startClient(installation, { cwd: root });
    try {
      await sleep(2000);
      const first = await client.run(...READ, { debugStats: true });
      assert.deepEqual(first.result.stats, cold(root), "the daemon's first answer: a cold request");
      await sleep(2000);
      const before = status(installation.mara);
      const second = await client.run(...READ, { debugStats: true });
      const now = status(installation.mara);
      assert.equal(now.served - before.served, 1, "the daemon served the second read");
      assert.equal(now.pid, before.pid, "the CLI's status names the process that served it");
      assert.deepEqual(second.result.result, first.result.result);
      assert.deepEqual(daemonsOfTest(), [now.pid], "another daemon of the test started");
    } finally {
      await client.close();
    }
  });

  test("CI, MARA_NO_DAEMON=1 and the direct-mode option serve every call in-process", async () => {
    const root = await project();
    const ways = [
      [{}, { CI: "true" }],
      [{}, { MARA_NO_DAEMON: "1" }],
      [{ noDaemon: true }, {}],
    ];
    await stopDaemons([installation.mara]);
    for (const [options, extra] of ways) {
      const client = startClient(installation, { cwd: root, options, env: clientEnv(extra) });
      try {
        const answer = await client.run(...READ, { debugStats: true });
        assert.deepEqual(answer.result.stats, cold(root), JSON.stringify(extra));
        await noneStarts();
      } finally {
        await client.close();
      }
    }
    daemon(installation.mara, ["start", "--project", root]);
    const running = status(installation.mara);
    for (const [options, extra] of ways) {
      const client = startClient(installation, { cwd: root, options, env: clientEnv(extra) });
      try {
        const answer = await client.run(...READ, { debugStats: true });
        assert.deepEqual(answer.result.stats, cold(root), JSON.stringify(extra));
        const now = status(installation.mara);
        assert.equal(now.served, running.served, `${JSON.stringify(extra)}: the daemon served it`);
        assert.deepEqual(daemonsOfTest(), [running.pid]);
      } finally {
        await client.close();
      }
    }
  });

  test("MARA_NO_DAEMON=1 and CI in a worker thread's environment serve its calls in-process", async () => {
    const root = await project();
    // A worker thread's `env` is its own: the native process never sees it.
    const read = (variables) => {
      const script = join(installation.dir, `worker-${Date.now()}-${Math.random()}.mjs`);
      writeFileSync(
        script,
        'import { Worker } from "node:worker_threads";\n' +
          `const library = new URL("./library.mjs", import.meta.url).href;\n` +
          "const code = `const { createClient } = await import(${JSON.stringify(library)});\n" +
          "const { parentPort } = await import('node:worker_threads');\n" +
          `const answer = await createClient().run(${JSON.stringify(READ[0])}, ${JSON.stringify(READ[1])}, { debugStats: true });\n` +
          "parentPort.postMessage(answer.stats);`;\n" +
          `const worker = new Worker(code, { eval: true, env: { ...process.env, ...${JSON.stringify(variables)} } });\n` +
          'worker.on("message", (stats) => { console.log(JSON.stringify(stats)); worker.terminate(); });\n' +
          'worker.on("error", (error) => { console.error(error); process.exitCode = 1; });\n',
      );
      const ran = spawnSync(process.execPath, [script], { cwd: root, env: clientEnv(), encoding: "utf8" });
      assert.equal(ran.status, 0, ran.stderr);
      return JSON.parse(ran.stdout);
    };
    const ways = [{ MARA_NO_DAEMON: "1" }, { CI: "true" }];
    await stopDaemons([installation.mara]);
    for (const variables of ways) {
      assert.deepEqual(read(variables), cold(root), JSON.stringify(variables));
      await noneStarts();
    }
    daemon(installation.mara, ["start", "--project", root]);
    const running = status(installation.mara);
    for (const variables of ways) {
      assert.deepEqual(read(variables), cold(root), JSON.stringify(variables));
      assert.equal(status(installation.mara).served, running.served, JSON.stringify(variables));
    }
  });

  test("a loaded addon keeps its build when its mara is replaced at its path", async () => {
    const root = await project();
    const running = status(installation.mara);
    assert.equal(running.running, true);
    const client = startClient(installation, { cwd: root });
    // The original, kept aside by a rename, which keeps its metadata to the nanosecond.
    const original = `${installation.mara}.original`;
    try {
      assert.ok((await client.run(...READ)).result);
      assert.equal(status(installation.mara).served, running.served + 1);
      // Another build at the same path, as a reinstall of the same version leaves it.
      renameSync(installation.mara, original);
      copyFileSync(original, installation.mara);
      chmodSync(installation.mara, 0o755);
      assert.notEqual(statSync(installation.mara).mtimeMs, statSync(original).mtimeMs);
      const second = await client.run(...READ);
      assert.ok(second.result, JSON.stringify(second));
      await sleep(500);
      // The running daemon's image is now named `mara.original`; no other daemon may run.
      const others = daemonsOfTest().filter((pid) => pid !== running.pid);
      assert.deepEqual(others, [], "the addon started the new file's daemon");
      process.kill(running.pid, 0);
    } finally {
      renameSync(original, installation.mara);
      await client.close();
    }
    const now = status(installation.mara);
    assert.equal(now.pid, running.pid);
    assert.equal(now.served, running.served + 2, "the daemon of the addon's build served both reads");
  });

  test("the client of a package whose binary is a copy at another path starts that copy's daemon", async () => {
    const root = await project();
    const running = status(installation.mara);
    assert.equal(running.running, true);
    assert.equal(status(copied.mara).running, false);
    const client = startClient(copied, { cwd: root });
    try {
      const answer = await client.run(...READ);
      assert.ok(answer.result, JSON.stringify(answer));
    } finally {
      await client.close();
    }
    const other = await waitFor("the copy's daemon", () => {
      const now = status(copied.mara);
      return now.running ? now : undefined;
    });
    assert.notEqual(other.pid, running.pid);
    assert.equal(status(installation.mara).pid, running.pid);
  });

  test("a loaded addon starts no daemon of a mara that replaced its own", async () => {
    const root = await project();
    if (!status(installation.mara).running) {
      daemon(installation.mara, ["start"]);
    }
    const client = startClient(installation, { cwd: root });
    const original = `${installation.mara}.original`;
    try {
      // The first call takes the build of the package's `mara`, whose daemon serves it.
      const before = status(installation.mara).served;
      assert.ok((await client.run(...READ)).result);
      assert.equal(status(installation.mara).served, before + 1);
      await stopDaemons([installation.mara, copied.mara]);
      // Another build at the same path, and no daemon of the test running.
      renameSync(installation.mara, original);
      copyFileSync(original, installation.mara);
      chmodSync(installation.mara, 0o755);
      const answer = await client.run(...READ, { debugStats: true });
      assert.deepEqual(answer.result?.stats, cold(root), "served in-process");
      await noneStarts();
    } finally {
      rmSync(installation.mara, { force: true });
      renameSync(original, installation.mara);
      await client.close();
    }
  });
});

/** A daemon of `installation` whose writer waits at each pause point, and its pauses' directory. */
function pausedDaemon(installation) {
  const pauses = dirOf("pauses");
  daemon(installation.mara, ["start"], { MARA_TEST_PAUSE: pauses });
  return { pid: status(installation.mara).pid, paused: join(pauses, "paused") };
}

/** What the writer's pause holds, once it waits there. */
async function pausedAt(paused) {
  return waitFor("the daemon's writer to pause", () => {
    try {
      return readFileSync(paused, "utf8");
    } catch {
      return undefined;
    }
  });
}

// The writer's pause points exist in debug builds only (ADR-DEBUG-PAUSE-POINT), as for the
// daemon's own write cases.
const pauses = process.env.MARA_HOSTS_DEBUG === "1" ? false : "the pause points exist in debug builds only";

describe("a Node write whose daemon is killed fails with daemon-lost and is not run again", { timeout: 600_000, skip: pauses }, () => {
  const installation = install(dirOf("writes"));

  after(async () => {
    await stopDaemons([installation.mara]);
  });

  test("a create killed before step 2 changes nothing", async () => {
    const root = await project();
    const before = filesOf(root);
    const { pid, paused } = pausedDaemon(installation);
    const client = startClient(installation, { cwd: root });
    try {
      const call = client.run("item_create", {
        type: "requirement",
        path: "docs/reqs.mara.md",
        title: "Four",
        field: ["status=draft"],
        body: "Text.",
      });
      assert.equal(await pausedAt(paused), "", "the pause before step 2");
      process.kill(pid, "SIGKILL");
      const answer = await call;
      assert.equal(answer.error?.code, "daemon-lost", JSON.stringify(answer));
      assert.match(answer.error.message, new RegExp(`^the daemon \\(process ${pid}\\) gave no answer to`));
    } finally {
      await client.close();
      rmSync(paused, { force: true });
    }
    await waitFor("no daemon", () => (daemonsOfTest().length === 0 ? true : undefined));
    const after = filesOf(root);
    assert.deepEqual([...after.keys()], [...before.keys()]);
    for (const [path, bytes] of after) {
      assert.ok(bytes.equals(before.get(path)), path);
    }
  });

  test("a move killed at its second file is left interrupted and reported", async () => {
    const root = await project();
    const { pid, paused } = pausedDaemon(installation);
    const client = startClient(installation, { cwd: root });
    try {
      const call = client.run("item_move", { "id-or-mid": "REQ-TWO", path: "docs/moved.mara.md" });
      const files = [];
      while (files.length < 2) {
        const at = await pausedAt(paused);
        if (at !== "") {
          files.push(at);
        }
        if (files.length < 2) {
          rmSync(paused);
        }
      }
      assert.deepEqual(files, ["docs/moved.mara.md", "docs/reqs.mara.md"]);
      process.kill(pid, "SIGKILL");
      const answer = await call;
      assert.equal(answer.error?.code, "daemon-lost", JSON.stringify(answer));
    } finally {
      await client.close();
      rmSync(paused, { force: true });
    }
    await waitFor("no daemon", () => (daemonsOfTest().length === 0 ? true : undefined));
    const moved = readFileSync(join(root, "docs/moved.mara.md"), "utf8");
    const source = readFileSync(join(root, "docs/reqs.mara.md"), "utf8");
    assert.ok(moved.includes("REQ-TWO") && source.includes("REQ-TWO"), "an interrupted move");
    // No daemon runs, so the validation is served in-process, and reports it.
    const validator = startClient(installation, { cwd: root, options: { noDaemon: true } });
    try {
      const validated = await validator.run("project_validate", {});
      const codes = validated.result.diagnostics.map((diagnostic) => diagnostic.code);
      assert.ok(codes.some((code) => code.startsWith("duplicate-")), JSON.stringify(codes));
    } finally {
      await validator.close();
    }
  });
});

test("an embedded Node write and a CLI write at once land as two CLI writes do", { timeout: 300_000 }, async () => {
  const installation = install(dirOf("writers"), { binary: false });
  const root = await project();
  const client = startClient(installation, { cwd: root, options: { noDaemon: true } });
  const create = (title) => ({
    type: "requirement",
    path: "docs/reqs.mara.md",
    title,
    field: ["status=draft"],
    body: "Text.",
  });
  let created = 0;
  try {
    const listedBefore = await client.run("item_list", { type: ["requirement"] });
    for (let round = 0; round < 3; round += 1) {
      const cli = spawn(
        maraBin,
        ["--format", "json", "item", "create", "requirement", "docs/reqs.mara.md", "--title", `By the CLI ${round}`]
          .concat(["--field", "status=draft", "--body", "Text."]),
        { cwd: root, env: clientEnv({ MARA_NO_DAEMON: "1" }), stdio: ["ignore", "pipe", "pipe"] },
      );
      let printed = "";
      cli.stdout.on("data", (chunk) => (printed += chunk));
      const [call, code] = await Promise.all([
        client.run("item_create", create(`By Node ${round}`)),
        new Promise((resolve) => cli.on("exit", resolve)),
      ]);
      const document = readFileSync(join(root, "docs/reqs.mara.md"), "utf8");
      for (const [ok, json] of [
        [code === 0, JSON.parse(printed)],
        [call.result !== undefined, call.result ?? call.error.reply],
      ]) {
        if (ok) {
          created += 1;
          assert.ok(document.includes(`<a id="${json.item.mid}"></a>`), JSON.stringify(json));
        } else {
          assert.equal(typeof json.error.code, "string", JSON.stringify(json));
        }
      }
    }
    assert.ok(created > 0);
    const listed = await client.run("item_list", { type: ["requirement"] });
    assert.equal(listed.result.items.length, listedBefore.result.items.length + created);
    const validated = await client.run("project_validate", {});
    assert.deepEqual(validated.result.diagnostics, []);
  } finally {
    await client.close();
  }
});
