// .github/scripts/npm-registry.sh against the registry stub: a version is absent only when npm
// says so, and any other answer fails after its tries.

import assert from "node:assert/strict";
import { test } from "node:test";
import {
  environment,
  natives,
  run,
  sha1,
  startRegistry,
  temporaryDirectory,
  version,
} from "./support.mjs";

const name = natives[0].name;

async function read(t, setUp, ...args) {
  const directory = temporaryDirectory(t);
  const registry = await startRegistry(t);
  setUp(registry);
  const env = environment(directory, { NPM_REGISTRY: registry.url });
  return { registry, result: await run("npm-registry.sh", args, { cwd: directory, env }) };
}

test("a listed version prints its SHA-1", async (t) => {
  const tarball = Buffer.from("listed");
  const { result } = await read(
    t,
    (registry) => registry.add(name, { version }, tarball),
    "shasum",
    name,
    version,
  );
  assert.equal(result.status, 0, result.output);
  assert.equal(result.stdout, `${sha1(tarball)}\n`);
});

test("a version that the document lacks, or a package that npm does not know, prints nothing", async (t) => {
  const other = await read(
    t,
    (registry) => registry.add(name, { version: "0.0.0" }, Buffer.from("x")),
    "shasum",
    name,
    version,
  );
  assert.equal(other.result.status, 0, other.result.output);
  assert.equal(other.result.stdout, "");
  const unknown = await read(t, () => {}, "shasum", name, version);
  assert.equal(unknown.result.status, 0, unknown.result.output);
  assert.equal(unknown.result.stdout, "");
  const tags = await read(t, () => {}, "dist-tags", name);
  assert.equal(tags.result.stdout, "{}\n");
});

test("dist-tags prints the package's dist-tags", async (t) => {
  const { result } = await read(
    t,
    (registry) => registry.tags(name, { latest: "0.3.0", next: "0.4.0-rc.1" }),
    "dist-tags",
    name,
  );
  assert.equal(result.status, 0, result.output);
  assert.deepEqual(JSON.parse(result.stdout), { latest: "0.3.0", next: "0.4.0-rc.1" });
});

for (const [label, answer] of [
  ["an HTTP 500", { status: 500, body: { error: "internal" } }],
  ["a 404 without npm's document", { status: 404, body: "<html>not found</html>" }],
  [
    "a 200 that is not the package's document",
    { status: 200, body: { name: "other", versions: {} } },
  ],
  [
    "a listed version without a SHA-1",
    { status: 200, body: { name, versions: { [version]: { dist: {} } } } },
  ],
]) {
  test(`${label} on every try fails, and is never read as absence`, async (t) => {
    const answers = Array.from({ length: 5 }, () => answer);
    const { registry, result } = await read(
      t,
      (registry) => registry.answer(name, ...answers),
      "shasum",
      name,
      version,
    );
    assert.equal(result.status, 1, result.output);
    assert.equal(result.stdout, "");
    assert.match(result.stderr, /try 5 of 5/);
    assert.equal(registry.log.length, 5);
  });
}

test("an error on the first tries is retried", async (t) => {
  const tarball = Buffer.from("retried");
  const { registry, result } = await read(
    t,
    (registry) => {
      registry.add(name, { version }, tarball);
      registry.answer(name, { status: 503, body: {} }, { status: 502, body: {} });
    },
    "shasum",
    name,
    version,
  );
  assert.equal(result.status, 0, result.output);
  assert.equal(result.stdout, `${sha1(tarball)}\n`);
  assert.equal(registry.log.length, 3);
});
