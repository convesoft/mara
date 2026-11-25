// The pathname that the install verification's Node-library case reads from each line of
// `/proc/self/maps` (VER-INSTALL-WITHOUT-TOOLCHAIN), and the module that it runs with it.

import assert from "node:assert/strict";
import { readFileSync, realpathSync } from "node:fs";
import { test } from "node:test";
import { embeddedScript, mappedPath } from "../scripts/verify-hosts.mjs";

test("a mapped pathname keeps its spaces, adjacent ones included", () => {
  const file = "/tmp/a  b/node_modules/@convesoft/mara-linux-x64-gnu/bin/mara.node";
  assert.equal(
    mappedPath(`7f0000000000-7f0000001000 r--p 00000000 08:01 1234                       ${file}`),
    file,
  );
  assert.equal(mappedPath("7ffd00000000-7ffd00021000 rw-p 00000000 00:00 0 "), "");
  assert.equal(mappedPath("7ffd00000000-7ffd00021000 rw-p 00000000 00:00 0"), "");
});

test(
  "the pathnames of this process's maps name its own executable",
  { skip: process.platform !== "linux" },
  () => {
    const mapped = readFileSync("/proc/self/maps", "utf8").split("\n").map(mappedPath);
    assert.ok(mapped.includes(realpathSync(process.execPath)), mapped.join("\n"));
  },
);

test("the Node-library module reads the maps with the same function", () => {
  assert.ok(embeddedScript.includes(`${mappedPath}\n`));
  assert.ok(embeddedScript.includes(".map(mappedPath)"));
});
