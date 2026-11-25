// .github/scripts/release-publish.sh, Publish's publication and its resumption, with the real npm
// against the registry stub.

import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { after, before, test } from "node:test";
import {
  environment,
  natives,
  npmLogin,
  others,
  packages,
  run,
  sha1,
  startRegistry,
  tarballDirectory,
  temporaryDirectory,
  version,
} from "./support.mjs";

let shared;
let tarballs;
const bytes = (entry) => fs.readFileSync(path.join(tarballs, entry.tarball));

before(() => {
  shared = fs.mkdtempSync(path.join(os.tmpdir(), "mara-release-"));
  tarballs = tarballDirectory(shared);
});
after(() => fs.rmSync(shared, { recursive: true, force: true }));

async function publish(t, setUp = () => {}, { args = [], login = true, env: extra = {} } = {}) {
  const directory = temporaryDirectory(t);
  const registry = await startRegistry(t);
  setUp(registry, directory);
  const base = environment(directory, {
    PACKAGES: JSON.stringify(packages),
    VERSION: version,
    DIST_TAG: "next",
    RELEASE_WAIT_SECONDS: "60",
    ...extra,
  });
  const env = login ? npmLogin(base, registry) : { ...base, NPM_REGISTRY: registry.url };
  const result = await run("release-publish.sh", [tarballs, ...args], { cwd: directory, env });
  return { registry, result };
}

const names = (entries) => entries.map((entry) => entry.name);

test("a publication publishes the native packages, waits until npm shows them, then the others", async (t) => {
  const { registry, result } = await publish(t, (registry) => (registry.hiddenReads = 2));
  assert.equal(result.status, 0, result.output);
  assert.deepEqual(registry.puts(), [...names(natives), ...names(others)]);
  for (const entry of packages) {
    assert.equal(registry.shasum(entry.name, version), sha1(bytes(entry)));
  }
  assert.match(result.stdout, /npm does not show .* yet/);
  // Every native package is read as shown before the first package for any platform is published.
  const firstOther = registry.log.indexOf(`PUT ${others[0].name}`);
  for (const entry of natives) {
    assert.ok(registry.log.lastIndexOf(`GET ${entry.name}`) < firstOther, registry.log.join("\n"));
  }
});

test("a re-run skips a version that npm has with the tarball's SHA-1 and publishes the rest", async (t) => {
  const { registry, result } = await publish(t, (registry) =>
    registry.add(natives[0].name, { version }, bytes(natives[0])),
  );
  assert.equal(result.status, 0, result.output);
  assert.deepEqual(registry.puts(), [...names(natives.slice(1)), ...names(others)]);
  assert.match(
    result.stdout,
    new RegExp(`npm has ${natives[0].name}@${version} with this tarball's SHA-1 .*; skipped`),
  );
});

test("a version that npm has with another SHA-1 fails at once, before that package and those after it are published", async (t) => {
  const { registry, result } = await publish(t, (registry) =>
    registry.add(natives.at(-1).name, { version }, Buffer.from("another tarball")),
  );
  assert.equal(result.status, 1, result.output);
  assert.match(
    result.stdout,
    /::error::npm has .* with the SHA-1 [0-9a-f]{40}, not this tarball's/,
  );
  assert.deepEqual(registry.puts(), names(natives.slice(0, -1)));
});

test("a registry error fails and is never read as absence", async (t) => {
  const { registry, result } = await publish(t, (registry) =>
    registry.answer(
      natives[0].name,
      ...Array.from({ length: 5 }, () => ({ status: 500, body: {} })),
    ),
  );
  assert.equal(result.status, 1, result.output);
  // Every try of the read reached the stub and got its error.
  assert.deepEqual(
    registry.log,
    Array.from({ length: 5 }, () => `GET ${natives[0].name}`),
  );
  assert.match(result.stderr, /HTTP 500/);
  assert.deepEqual(registry.puts(), []);
});

test("the wait fails at once on a registry error", async (t) => {
  const { registry, result } = await publish(t, (registry) => {
    registry.hiddenReads = 1;
    registry.answerAfterPublish(
      natives[0].name,
      ...Array.from({ length: 5 }, () => ({ status: 500, body: {} })),
    );
  });
  assert.equal(result.status, 1, result.output);
  assert.match(result.stderr, /HTTP 500/);
  assert.doesNotMatch(result.stdout, /npm does not show/);
  assert.deepEqual(registry.puts(), names(natives));
});

test("the wait fails at once when npm shows another SHA-1", async (t) => {
  const other = "0".repeat(40);
  const { registry, result } = await publish(t, (registry) =>
    registry.answerAfterPublish(natives[0].name, {
      status: 200,
      body: {
        name: natives[0].name,
        "dist-tags": {},
        versions: { [version]: { dist: { shasum: other } } },
      },
    }),
  );
  assert.equal(result.status, 1, result.output);
  assert.match(
    result.stdout,
    new RegExp(`::error::npm shows ${natives[0].name}@${version} with the SHA-1 ${other}`),
  );
  assert.deepEqual(registry.puts(), names(natives));
});

test("the wait gives up when npm does not show a native package in time", async (t) => {
  const { registry, result } = await publish(t, (registry) => (registry.hiddenReads = 1000), {
    env: { RELEASE_WAIT_SECONDS: "0" },
  });
  assert.equal(result.status, 1, result.output);
  assert.match(result.stdout, /::error::npm did not show .* within 0 seconds/);
  assert.deepEqual(registry.puts(), names(natives));
});

test("a package for any platform that npm has with another SHA-1 fails", async (t) => {
  const { registry, result } = await publish(t, (registry) => {
    for (const entry of natives) {
      registry.add(entry.name, { version }, bytes(entry));
    }
    registry.add(others.at(-1).name, { version }, Buffer.from("another dispatcher"));
  });
  assert.equal(result.status, 1, result.output);
  assert.deepEqual(registry.puts(), names(others.slice(0, -1)));
});

test("a publication that npm refuses fails, and nothing after it is published", async (t) => {
  const { registry, result } = await publish(t, () => {}, { login: false });
  assert.notEqual(result.status, 0, result.output);
  assert.match(result.stderr, /ENEEDAUTH/);
  assert.deepEqual(registry.puts(), []);
});

test("the given npm options reach npm: --provenance without an OIDC token publishes nothing", async (t) => {
  const { registry, result } = await publish(t, () => {}, { args: ["--provenance"] });
  assert.notEqual(result.status, 0, result.output);
  assert.match(result.stderr, /provenance/i);
  assert.deepEqual(registry.puts(), []);
});

test("npm older than 11.5.1 fails before the registry is read", async (t) => {
  const bin = temporaryDirectory(t);
  fs.writeFileSync(path.join(bin, "npm"), "#!/bin/sh\necho 11.5.0\n", { mode: 0o755 });
  const { registry, result } = await publish(t, () => {}, {
    env: { PATH: `${bin}:${process.env.PATH}` },
  });
  assert.equal(result.status, 1, result.output);
  assert.match(result.stdout, /::error::npm 11\.5\.0 is older than 11\.5\.1/);
  assert.deepEqual(registry.log, []);
});
