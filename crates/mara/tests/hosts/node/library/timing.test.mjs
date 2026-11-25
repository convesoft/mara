// The time-limit case of VER-NODE-LIBRARY (docs/hosts/verification.mara.md): `project validate`
// of the long program of VER-EVALUATION-TIME-LIMIT with a time limit of 2 s, embedded and through
// the daemon, keeps the client's event loop free: a 10 ms interval timer started just before the
// call never waits more than 200 ms, and the promise settles after at least 2 s. And an embedded
// evaluation leaves Node's signals to Node (DES-RULE-EXECUTION).

import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { after, test } from "node:test";

import { copy, fixture, testDir } from "../support/hosts.mjs";
import {
  clientEnv,
  daemon,
  install,
  startClient,
  status,
  stopDaemons,
  waitFor,
} from "../support/library.mjs";
import { config, invariant } from "../support/programs.mjs";

/** VER-EVALUATION-TIME-LIMIT's long program, as `browser/limits.test.mjs` runs it. */
const LONG =
  '.include "mara.dl"\n.decl n(x: number)\nn(x) :- x = range(0, 1000).\n.decl bad(m: Mid, c: number)\n' +
  "bad(m, c) :- item(m, _, _, _, _, _), c = count : { n(w), n(x), n(y), n(z), w + x + y + z < 0 }.\n";

/** The largest wait of the event loop that `times` shows, and how long the call took, in ms. */
function waits({ called, callbacks, settled }) {
  const points = [called, ...callbacks.filter((time) => time <= settled), settled];
  const gaps = points.slice(1).map((time, index) => time - points[index]);
  return { longest: Math.max(...gaps), took: settled - called };
}

let installation;

after(async () => {
  if (installation) {
    await stopDaemons([installation.mara]);
  }
});

test("a validation that runs to the time limit keeps the event loop free", { timeout: 120_000 }, async () => {
  installation = install(mkdtempSync(join(testDir, "timing-")));
  const root = copy(fixture("programs").root);
  mkdirSync(join(root, ".mara/rules"), { recursive: true });
  writeFileSync(join(root, ".mara/rules/long.dl"), LONG);
  writeFileSync(join(root, ".mara/config.toml"), config(invariant("long", "long.dl", "bad", "error"), 2));
  daemon(installation.mara, ["start", "--project", root]);
  await waitFor("the daemon's session", () => (status(installation.mara).sessions.includes(root) ? true : undefined));

  for (const noDaemon of [true, false]) {
    const client = startClient(installation, { options: { project: root, noDaemon } });
    try {
      const before = status(installation.mara).served;
      const answer = await client.run("project_validate", {}, undefined, { timed: true });
      assert.equal(answer.result?.valid, false, JSON.stringify(answer));
      const faults = answer.result.diagnostics.filter((diagnostic) => diagnostic.code === "program-fault");
      assert.equal(faults.length, 1, JSON.stringify(answer.result));
      assert.equal(status(installation.mara).served - before, noDaemon ? 0 : 1, "the daemon served it");
      const { longest, took } = waits(answer.times);
      assert.ok(took >= 2000, `the call settled after ${took} ms`);
      assert.ok(longest <= 200, `the event loop waited ${longest} ms (noDaemon ${noDaemon})`);
    } finally {
      await client.close();
    }
  }
});

test("an embedded evaluation leaves the process's signals to Node", { timeout: 120_000 }, async () => {
  const embedded = install(mkdtempSync(join(testDir, "signals-")), { binary: false });
  const root = copy(fixture("programs").root);
  mkdirSync(join(root, ".mara/rules"), { recursive: true });
  writeFileSync(join(root, ".mara/rules/p.dl"), '.include "mara.dl"\n.decl bad(m: Mid)\nbad(m) :- item(m, _, _, "nope", _, _).\n');
  writeFileSync(join(root, ".mara/config.toml"), config(invariant("p", "p.dl", "bad", "error")));
  const script = join(embedded.dir, "signals.mjs");
  writeFileSync(
    script,
    'import { createClient } from "./library.mjs";\n' +
      'process.on("SIGINT", () => { console.log("handled"); process.exit(0); });\n' +
      "const validated = await createClient({ noDaemon: true }).run(\"project_validate\", {}, { debugStats: true });\n" +
      "console.log(JSON.stringify(validated.stats));\n" +
      "setInterval(() => {}, 1000);\n",
  );
  const child = spawn(process.execPath, [script], { cwd: root, env: clientEnv(), stdio: ["ignore", "pipe", "inherit"] });
  let printed = "";
  child.stdout.on("data", (chunk) => {
    printed += chunk;
    if (printed.includes("program_runs")) {
      child.kill("SIGINT");
    }
  });
  const [code, signal] = await new Promise((resolve) => child.on("exit", (...ended) => resolve(ended)));
  const [stats, handled] = printed.trim().split("\n");
  assert.ok(JSON.parse(stats).program_runs > 0, "the validation ran no program");
  assert.deepEqual([code, signal, handled], [0, null, "handled"]);
});
