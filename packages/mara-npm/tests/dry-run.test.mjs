// .github/scripts/release-dry-run.sh, Dry run's `npm publish --dry-run`, with the real npm against
// the registry stub: it publishes nothing.

import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { after, before, test } from "node:test";
import {
  environment,
  natives,
  npmLogin,
  packages,
  run,
  startRegistry,
  tarballDirectory,
  temporaryDirectory,
  version,
} from "./support.mjs";

let shared;
let tarballs;

before(() => {
  shared = fs.mkdtempSync(path.join(os.tmpdir(), "mara-release-"));
  tarballs = tarballDirectory(shared);
});
after(() => fs.rmSync(shared, { recursive: true, force: true }));

async function dryRun(t, setUp = () => {}, directoryOf = () => tarballs) {
  const directory = temporaryDirectory(t);
  const registry = await startRegistry(t);
  setUp(registry);
  const env = npmLogin(
    environment(directory, {
      PACKAGES: JSON.stringify(packages),
      VERSION: version,
      DIST_TAG: "next",
    }),
    registry,
  );
  const result = await run("release-dry-run.sh", [directoryOf(directory)], { cwd: directory, env });
  return { registry, result };
}

test("every package's dry run runs, native packages first, and nothing is published", async (t) => {
  const { registry, result } = await dryRun(t);
  assert.equal(result.status, 0, result.output);
  const order = packages.map((entry) => result.output.indexOf(`+ ${entry.name}@${version}`));
  assert.ok(
    order.every((index) => index >= 0),
    result.output,
  );
  assert.deepEqual(
    order,
    [...order].sort((a, b) => a - b),
  );
  assert.match(result.output, /\(dry-run\)/);
  assert.deepEqual(registry.puts(), []);
});

test("a version that npm has before its dry run is left out with a warning", async (t) => {
  const { registry, result } = await dryRun(t, (registry) =>
    registry.add(natives[0].name, { version }, Buffer.from("published")),
  );
  assert.equal(result.status, 0, result.output);
  assert.match(
    result.stdout,
    new RegExp(`::warning::npm has ${natives[0].name}@${version}, so its dry run is left out`),
  );
  assert.doesNotMatch(result.output, new RegExp(`\\+ ${natives[0].name}@`));
  assert.deepEqual(registry.puts(), []);
});

test("a version published between the read and the dry run is left out with a warning", async (t) => {
  // The script's read sees no version; npm's own read, and the script's second, see it.
  const { registry, result } = await dryRun(t, (registry) =>
    registry.add(natives[0].name, { version }, Buffer.from("published"), { hiddenReads: 1 }),
  );
  assert.equal(result.status, 0, result.output);
  assert.match(result.stderr, /npm error/);
  assert.match(
    result.stdout,
    new RegExp(`::warning::npm has ${natives[0].name}@${version}, so its dry run is left out`),
  );
  assert.deepEqual(registry.puts(), []);
});

test("a dry run that npm refuses for a version it does not have fails", async (t) => {
  const { result } = await dryRun(
    t,
    () => {},
    (directory) => {
      const broken = path.join(directory, "broken");
      fs.mkdirSync(broken);
      for (const entry of packages) {
        fs.writeFileSync(path.join(broken, entry.tarball), "not a tarball");
      }
      return broken;
    },
  );
  assert.equal(result.status, 1, result.output);
  assert.match(
    result.stdout,
    new RegExp(
      `::error::npm publish --dry-run of ${natives[0].name} failed, and npm has no ${version} of it`,
    ),
  );
});

test("a registry error fails the dry run", async (t) => {
  const { result } = await dryRun(t, (registry) =>
    registry.answer(
      natives[0].name,
      ...Array.from({ length: 5 }, () => ({ status: 500, body: {} })),
    ),
  );
  assert.equal(result.status, 1, result.output);
  assert.doesNotMatch(result.output, /\+ @convesoft/);
});
