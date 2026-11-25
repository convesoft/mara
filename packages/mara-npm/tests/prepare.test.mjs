// .github/scripts/release-prepare.sh in a workspace of its own, with a temporary bare Git remote
// as `origin` and the registry stub.

import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { test } from "node:test";
import { releasePackages, root } from "../scripts/packages.mjs";
import { environment, git, outputs, run, startRegistry, temporaryDirectory } from "./support.mjs";

// A Cargo workspace whose package `mara` has the version, committed and pushed to `origin`.
function workspace(directory, env, version) {
  const origin = path.join(directory, "origin.git");
  const checkout = path.join(directory, "checkout");
  git(directory, env, "init", "-q", "--bare", origin);
  fs.mkdirSync(path.join(checkout, "src"), { recursive: true });
  fs.writeFileSync(path.join(checkout, "src", "lib.rs"), "");
  fs.writeFileSync(
    path.join(checkout, "Cargo.toml"),
    `[package]\nname = "mara"\nversion = "${version}"\nedition = "2021"\n`,
  );
  fs.writeFileSync(
    path.join(checkout, "Cargo.lock"),
    `version = 4\n\n[[package]]\nname = "mara"\nversion = "${version}"\n`,
  );
  fs.copyFileSync(
    path.join(root, "rust-toolchain.toml"),
    path.join(checkout, "rust-toolchain.toml"),
  );
  git(checkout, env, "init", "-q", "-b", "main");
  git(checkout, env, "add", ".");
  git(checkout, env, "commit", "-q", "-m", "version");
  git(checkout, env, "remote", "add", "origin", origin);
  git(checkout, env, "push", "-q", "origin", "main");
  return checkout;
}

async function prepare(
  t,
  {
    version = "0.4.0-rc.2",
    distTag = "next",
    dryRun = "true",
    ref = "refs/heads/main",
    setUp = () => {},
  } = {},
) {
  const directory = temporaryDirectory(t);
  const registry = await startRegistry(t);
  const env = environment(directory, {
    NPM_REGISTRY: registry.url,
    DIST_TAG: distTag,
    DRY_RUN: dryRun,
    REF: ref,
  });
  const checkout = workspace(directory, env, version);
  setUp({ registry, checkout, env });
  const result = await run("release-prepare.sh", [], { cwd: checkout, env });
  return { registry, result, outputs: outputs(env), checkout, env };
}

test("a dispatch records the commit, the version, the tag, the dist-tag and the packages", async (t) => {
  const {
    result,
    outputs: recorded,
    checkout,
    env,
    registry,
  } = await prepare(t, { dryRun: "false" });
  assert.equal(result.status, 0, result.output);
  assert.deepEqual(recorded, {
    sha: git(checkout, env, "rev-parse", "HEAD"),
    version: "0.4.0-rc.2",
    tag: "v0.4.0-rc.2",
    dist_tag: "next",
    packages: JSON.stringify(releasePackages("0.4.0-rc.2")),
  });
  const packages = JSON.parse(recorded.packages);
  assert.ok(packages.length >= 2);
  // Native packages first, the dispatcher last.
  assert.deepEqual(
    packages.map((entry) => entry.native),
    packages.map((entry) => entry.native).toSorted((a, b) => Number(b) - Number(a)),
  );
  assert.equal(packages.at(-1).name, "@convesoft/mara");
  assert.deepEqual(
    registry.log,
    packages.map((entry) => `GET ${entry.name}`),
  );
});

test("a version without a prerelease part may take latest", async (t) => {
  const { result, outputs: recorded } = await prepare(t, { version: "0.4.0", distTag: "latest" });
  assert.equal(result.status, 0, result.output);
  assert.equal(recorded.dist_tag, "latest");
});

for (const [label, options, message] of [
  [
    "a version with build metadata",
    { version: "0.4.0+build.1" },
    /::error::the version "0\.4\.0\+build\.1" is not SemVer/,
  ],
  ["an empty dist-tag", { distTag: "" }, /::error::the dist-tag "" does not match/],
  ["a dist-tag with a capital", { distTag: "Next" }, /::error::the dist-tag "Next" does not match/],
  [
    "latest for a prerelease",
    { distTag: "latest" },
    /::error::0\.4\.0-rc\.2 is a prerelease, which is never published under latest/,
  ],
  [
    "a publication from another ref",
    { dryRun: "false", ref: "refs/heads/work" },
    /::error::a publication runs only from refs\/heads\/main, not refs\/heads\/work/,
  ],
]) {
  test(`Prepare refuses ${label}`, async (t) => {
    const { result, outputs: recorded, registry } = await prepare(t, options);
    assert.equal(result.status, 1, result.output);
    assert.match(result.stdout, message);
    assert.deepEqual(recorded, {});
    assert.deepEqual(registry.log, []);
  });
}

test("a dry run from another ref is allowed", async (t) => {
  const { result } = await prepare(t, { ref: "refs/heads/work" });
  assert.equal(result.status, 0, result.output);
});

test("Prepare refuses a version whose tag origin has", async (t) => {
  const { result, outputs: recorded } = await prepare(t, {
    setUp: ({ checkout, env }) => {
      git(checkout, env, "tag", "v0.4.0-rc.2");
      git(checkout, env, "push", "-q", "origin", "v0.4.0-rc.2");
      git(checkout, env, "tag", "-d", "v0.4.0-rc.2");
    },
  });
  assert.equal(result.status, 1, result.output);
  assert.match(result.stdout, /::error::the tag v0\.4\.0-rc\.2 exists/);
  assert.deepEqual(recorded, {});
});

test("Prepare refuses a version that npm has for any package", async (t) => {
  const name = releasePackages("0.4.0-rc.2")[1].name;
  const { result, outputs: recorded } = await prepare(t, {
    setUp: ({ registry }) =>
      registry.add(name, { version: "0.4.0-rc.2" }, Buffer.from("published")),
  });
  assert.equal(result.status, 1, result.output);
  assert.match(
    result.stdout,
    new RegExp(`::error::npm has 0\\.4\\.0-rc\\.2 of ${name}; set a new version`),
  );
  assert.deepEqual(recorded, {});
});

test("Prepare fails on a registry error and never reads it as absence", async (t) => {
  const name = releasePackages("0.4.0-rc.2")[0].name;
  const { result, outputs: recorded } = await prepare(t, {
    setUp: ({ registry }) =>
      registry.answer(name, ...Array.from({ length: 5 }, () => ({ status: 500, body: {} }))),
  });
  assert.equal(result.status, 1, result.output);
  assert.match(result.stderr, /HTTP 500/);
  assert.deepEqual(recorded, {});
});

test("Prepare fails when origin cannot be read, and never reads that as no tag", async (t) => {
  const {
    result,
    outputs: recorded,
    registry,
  } = await prepare(t, {
    setUp: ({ checkout, env }) =>
      git(checkout, env, "remote", "set-url", "origin", `${checkout}-missing.git`),
  });
  assert.notEqual(result.status, 0, result.output);
  assert.match(result.stderr, /does not appear to be a git repository/);
  assert.deepEqual(recorded, {});
  assert.deepEqual(registry.log, []);
});
