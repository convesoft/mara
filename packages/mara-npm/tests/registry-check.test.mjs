// .github/scripts/release-dist-tags.sh, the dist-tags baseline of Publish and Registry check, and
// .github/scripts/release-npx-check.sh, against the registry stub.

import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { test } from "node:test";
import {
  environment,
  natives,
  others,
  outputs,
  pack,
  packages,
  run,
  startRegistry,
  temporaryDirectory,
  version,
} from "./support.mjs";

async function distTags(t, command, setUp, extra = {}) {
  const directory = temporaryDirectory(t);
  const registry = await startRegistry(t);
  const file = path.join(directory, "runner-temp", "dist-tags", "dist-tags-before.json");
  setUp(registry, file);
  const env = environment(directory, {
    PACKAGES: JSON.stringify(packages),
    VERSION: version,
    DIST_TAG: "next",
    RELEASE_WAIT_SECONDS: "60",
    NPM_REGISTRY: registry.url,
    ...extra,
  });
  const result = await run("release-dist-tags.sh", [command, file], { cwd: directory, env });
  return { registry, result, file, env };
}

const baseline = Object.fromEntries(
  packages.map((entry, index) => [
    entry.name,
    index === 0 ? {} : { latest: "0.3.0", next: "0.0.0" },
  ]),
);

test("record writes every package's dist-tags and asks for the upload", async (t) => {
  const { result, file, env } = await distTags(t, "record", (registry) => {
    for (const [name, tags] of Object.entries(baseline)) {
      if (Object.keys(tags).length > 0) {
        registry.tags(name, tags);
      }
    }
  });
  assert.equal(result.status, 0, result.output);
  assert.deepEqual(JSON.parse(fs.readFileSync(file, "utf8")), baseline);
  assert.equal(outputs(env).recorded, "true");
});

test("record keeps what an earlier attempt recorded and reads nothing", async (t) => {
  const earlier = { [natives[0].name]: { next: "earlier" } };
  const { registry, result, file, env } = await distTags(t, "record", (registry, file) => {
    fs.mkdirSync(path.dirname(file), { recursive: true });
    fs.writeFileSync(file, JSON.stringify(earlier));
  });
  assert.equal(result.status, 0, result.output);
  assert.deepEqual(JSON.parse(fs.readFileSync(file, "utf8")), earlier);
  assert.equal(outputs(env).recorded, undefined);
  assert.deepEqual(registry.log, []);
});

test("record fails on a registry error and writes nothing", async (t) => {
  const { result, file, env } = await distTags(t, "record", (registry) =>
    registry.answer(
      others[0].name,
      ...Array.from({ length: 5 }, () => ({ status: 500, body: {} })),
    ),
  );
  assert.equal(result.status, 1, result.output);
  assert.equal(fs.existsSync(file), false);
  assert.equal(outputs(env).recorded, undefined);
});

function writeBaseline(file) {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, JSON.stringify(baseline));
}

function published(registry, { changed } = {}) {
  for (const [name, tags] of Object.entries(baseline)) {
    // Another order of the keys than the baseline's: the comparison is by JSON value.
    const after = { next: version, ...tags };
    after.next = version;
    if (name === changed) {
      after.latest = "9.9.9";
    }
    registry.tags(name, after, { old: tags, reads: 2 });
  }
}

test("check waits until the dist-tag names the version and the others are unchanged", async (t) => {
  const { registry, result } = await distTags(t, "check", (registry, file) => {
    writeBaseline(file);
    published(registry);
  });
  assert.equal(result.status, 0, result.output);
  for (const entry of packages) {
    assert.match(result.stdout, new RegExp(`${entry.name}: .*"next":"${version}"`));
    // Two reads still showed the old dist-tags.
    assert.equal(registry.log.filter((line) => line === `GET ${entry.name}`).length, 3);
  }
});

test("check fails when another dist-tag changed", async (t) => {
  const { result } = await distTags(t, "check", (registry, file) => {
    writeBaseline(file);
    published(registry, { changed: others.at(-1).name });
  });
  assert.equal(result.status, 1, result.output);
  assert.match(
    result.stdout,
    new RegExp(`::error::the other dist-tags of ${others.at(-1).name} changed`),
  );
});

test("check gives up when the dist-tag does not name the version in time", async (t) => {
  const { result } = await distTags(
    t,
    "check",
    (registry, file) => {
      writeBaseline(file);
      registry.tags(natives[0].name, { next: "0.0.0" });
    },
    { RELEASE_WAIT_SECONDS: "0" },
  );
  assert.equal(result.status, 1, result.output);
  assert.match(
    result.stdout,
    new RegExp(`::error::next of ${natives[0].name} does not name ${version} within 0 seconds`),
  );
});

test("check fails at once on a registry error", async (t) => {
  const { result } = await distTags(t, "check", (registry, file) => {
    writeBaseline(file);
    published(registry);
    registry.answer(
      natives[0].name,
      ...Array.from({ length: 5 }, () => ({ status: 500, body: {} })),
    );
  });
  assert.equal(result.status, 1, result.output);
  assert.match(result.stderr, /HTTP 500/);
  assert.doesNotMatch(result.stdout, /::error::next of/);
});

async function npxCheck(t, printed, { answers = [], ...extra } = {}) {
  const directory = temporaryDirectory(t);
  const registry = await startRegistry(t);
  const dispatcher = others.at(-1).name;
  registry.add(
    dispatcher,
    { version, bin: { mara: "bin/mara.cjs" } },
    pack(directory, dispatcher, { printed }),
  );
  registry.answer(dispatcher, ...answers);
  const env = environment(directory, {
    VERSION: version,
    NPM_REGISTRY: registry.url,
    RELEASE_WAIT_SECONDS: "60",
    ...extra,
  });
  const result = await run("release-npx-check.sh", [], { cwd: directory, env });
  return { registry, result };
}

test("npx runs the dispatcher of the version from the registry", async (t) => {
  const { registry, result } = await npxCheck(t, `mara ${version}`);
  assert.equal(result.status, 0, result.output);
  assert.equal(result.stdout, `mara ${version}\n`);
  assert.ok(
    registry.log.some((line) => line.endsWith(".tgz")),
    registry.log.join("\n"),
  );
});

test("npx is tried again, in an empty directory and cache, while the registry lacks the version", async (t) => {
  const { result } = await npxCheck(t, `mara ${version}`, {
    answers: Array.from({ length: 5 }, () => ({ status: 404, body: { error: "Not found" } })),
  });
  assert.equal(result.status, 0, result.output);
  assert.match(result.stdout, /^(printed: nothing\n)+mara 0\.0\.1\n$/);
});

test("npx that prints another version fails once the time is up", async (t) => {
  const { result } = await npxCheck(t, "mara 0.0.0", { RELEASE_WAIT_SECONDS: "0" });
  assert.equal(result.status, 1, result.output);
  assert.match(
    result.stdout,
    /printed: mara 0\.0\.0\n::error::npx did not run mara 0\.0\.1 from the registry within 0 seconds/,
  );
});

test("record of an empty selection writes {} and makes no request", async (t) => {
  const { registry, result, file } = await distTags(t, "record", () => {}, { PACKAGES: "[]" });
  assert.equal(result.status, 0, result.output);
  assert.deepEqual(JSON.parse(fs.readFileSync(file, "utf8")), {});
  assert.deepEqual(registry.log, []);
});
