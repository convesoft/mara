// The helper that runs the scripts: a script that exits before it reads its standard input still
// gives its exit status, and one that cannot be started fails the test.

import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { test } from "node:test";
import { environment, run, temporaryDirectory } from "./support.mjs";

test("a script that exits without reading its input gives its exit status and output", async (t) => {
  const directory = temporaryDirectory(t);
  const script = path.join(directory, "exits.sh");
  fs.writeFileSync(script, "#!/bin/sh\necho exiting\nexit 3\n", { mode: 0o755 });
  // More than a pipe holds, so the write cannot finish before the script exits: it ends in EPIPE.
  const input = "x".repeat(4 * 1024 * 1024);
  const result = await run(script, [], { cwd: directory, env: environment(directory), input });
  assert.equal(result.status, 3);
  assert.equal(result.stdout, "exiting\n");
});

test("a script that cannot be started rejects", async (t) => {
  const directory = temporaryDirectory(t);
  await assert.rejects(
    run(path.join(directory, "missing.sh"), [], { cwd: directory, env: environment(directory) }),
    { code: "ENOENT" },
  );
});
