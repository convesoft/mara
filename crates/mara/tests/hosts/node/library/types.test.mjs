// The `tsc` case of VER-NODE-LIBRARY (docs/hosts/verification.mara.md): `tsc --strict --noEmit`
// of `types.ts`, a module of the test that imports the library by its name where the library is
// installed, exits with 0: the calls type-check, each line marked `@ts-expect-error` is an error,
// and no argument or result type holds `any` at any depth.

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { copyFileSync, mkdtempSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { testDir } from "../support/hosts.mjs";
import { install } from "../support/library.mjs";

const tsc = process.env.MARA_TSC;

test("the library's types check every operation and hold no any", () => {
  const dir = mkdtempSync(join(testDir, "types-"));
  install(dir, { binary: false });
  copyFileSync(fileURLToPath(new URL("./types.ts", import.meta.url)), join(dir, "types.ts"));
  // The module stands alone: no configuration of the repository applies.
  writeFileSync(join(dir, "package.json"), JSON.stringify({ type: "module" }));
  const checked = spawnSync(
    process.execPath,
    [
      tsc,
      "--strict",
      "--noEmit",
      "--module",
      "nodenext",
      "--moduleResolution",
      "nodenext",
      "--target",
      "es2023",
      "types.ts",
    ],
    { cwd: dir, encoding: "utf8" },
  );
  assert.equal(checked.error, undefined, `TypeScript's compiler at ${tsc}: ${checked.error}`);
  assert.equal(`${checked.stdout}${checked.stderr}`, "");
  assert.equal(checked.status, 0);
});
