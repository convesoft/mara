// The real packaging script and install verification on the dispatcher's `docs/` (ADR-BUNDLED-DOCS,
// VER-INSTALL-WITHOUT-TOOLCHAIN): run in a disposable workspace, a Git repository with a copy of
// packages/mara-npm and a Cargo workspace whose `cargo metadata` gives the version. Every child
// starts in the test's temporary directory, so that no project `.npmrc` of the caller applies.

import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { test } from "node:test";
import { fromTarListing, hostPlatform, root, tarballName } from "../scripts/packages.mjs";
import { environment, run, startRegistry, temporaryDirectory } from "./support.mjs";

const version = "0.0.1";
const dispatcher = tarballName("@convesoft/mara", version);
const pages = {
  "docs/index.mara.md": "# Index\n",
  "docs/rules/index.mara.md": "# Rules\n",
  // npm reads brackets in a manifest's `files` as a pattern; tar lists é and \ escaped in the C
  // locale.
  "docs/[id].md": "# By ID\n",
  "docs/démarrage.md": "# Démarrage\n",
  "docs/back\\slash.md": "# Backslash\n",
};

// The workspace, committed, with `pages` under docs/ and `extra` files anywhere.
function workspace(t, extra = {}, cargoVersion = version) {
  const directory = temporaryDirectory(t);
  const repo = path.join(directory, "repo");
  const env = environment(directory, { LC_ALL: "C" });
  const project = path.join(repo, "packages", "mara-npm");
  for (const folder of ["bin", "scripts"]) {
    fs.cpSync(path.join(root, "packages", "mara-npm", folder), path.join(project, folder), {
      recursive: true,
    });
  }
  // `run` starts a script by its path.
  for (const name of ["pack.mjs", "verify-install.mjs"]) {
    fs.chmodSync(path.join(project, "scripts", name), 0o755);
  }
  for (const file of ["rust-toolchain.toml", "README.md", "LICENSE-MIT", "LICENSE-APACHE"]) {
    fs.copyFileSync(path.join(root, file), path.join(repo, file));
  }
  const files = {
    "Cargo.toml": `[workspace]\nmembers = ["crates/mara"]\nresolver = "3"\n`,
    "crates/mara/Cargo.toml":
      `[package]\nname = "mara"\nversion = "${cargoVersion}"\nedition = "2024"\n` +
      `repository = "https://example.invalid/mara"\n`,
    "crates/mara/src/main.rs": "fn main() {}\n",
    ...pages,
    ...extra,
  };
  for (const [file, contents] of Object.entries(files)) {
    fs.mkdirSync(path.dirname(path.join(repo, file)), { recursive: true });
    fs.writeFileSync(path.join(repo, file), contents);
  }
  const git = (...args) =>
    execFileSync("git", ["-C", repo, ...args], { cwd: directory, env, encoding: "utf8" });
  git("init", "--quiet");
  git("add", ".");
  git("commit", "--quiet", "--message", "workspace");
  const script = (name) => path.join(project, "scripts", name);
  return { directory, repo, env, git, script };
}

async function pack({ directory, env, script }, out = path.join(directory, "out")) {
  const result = await run(script("pack.mjs"), ["dispatcher", out], { cwd: directory, env });
  return { ...result, tarball: path.join(out, dispatcher), out };
}

const listing = (tarball, { directory, env }) =>
  execFileSync("tar", ["-tzf", tarball], { cwd: directory, env, encoding: "latin1" })
    .split("\n")
    .filter(Boolean)
    .map((name) => fromTarListing(name).replace(/^package\//, ""));

test("the packaging script packs the docs that Git tracks, under any locale", async (t) => {
  const fixture = workspace(t);
  const result = await pack(fixture);
  assert.equal(result.status, 0, result.stderr);
  const tracked = fixture.git("ls-files", "-z", "docs").split("\0").filter(Boolean).sort();
  assert.deepEqual(tracked, Object.keys(pages).sort());
  const packed = listing(result.tarball, fixture);
  assert.deepEqual(packed.filter((name) => name.startsWith("docs/")).sort(), tracked);
  const manifest = JSON.parse(
    execFileSync("tar", ["-xzOf", result.tarball, "package/package.json"], {
      cwd: fixture.directory,
      encoding: "utf8",
    }),
  );
  assert.deepEqual(manifest.files, [
    "bin/mara.cjs",
    "README.md",
    "LICENSE-MIT",
    "LICENSE-APACHE",
    "docs",
  ]);
});

for (const [name, change, named] of [
  [
    "a modified page",
    ({ repo }) => fs.appendFileSync(path.join(repo, "docs", "index.mara.md"), "x"),
    "docs/index.mara.md",
  ],
  [
    "an untracked page",
    ({ repo }) => fs.writeFileSync(path.join(repo, "docs", "new.md"), "x"),
    "docs/new.md",
  ],
]) {
  test(`the packaging script refuses docs/ with ${name} and writes no tarball`, async (t) => {
    const fixture = workspace(t);
    change(fixture);
    const result = await pack(fixture);
    assert.equal(result.status, 1, result.stdout);
    assert.match(result.stderr, /docs\/ in the working tree differs from HEAD/);
    assert.ok(result.stderr.includes(named), result.stderr);
    assert.equal(fs.existsSync(result.tarball), false);
  });
}

test("the packaging script fails, naming it, on a tracked page that npm never packs", async (t) => {
  const fixture = workspace(t, { "docs/notes.orig": "x" });
  const result = await pack(fixture);
  assert.equal(result.status, 1, result.stdout);
  assert.match(
    result.stderr,
    /the tarball of @convesoft\/mara is wrong: missing docs\/notes\.orig/,
  );
  assert.equal(fs.existsSync(result.tarball), false);
});

// A native package for the host whose `bin/mara` only prints the version: enough for npm to install
// the dispatcher with it, so that the install verification reaches the dispatcher's cases.
function nativeTarball({ directory, env }, out) {
  const platform = hostPlatform();
  const stage = path.join(directory, "native");
  fs.mkdirSync(path.join(stage, "bin"), { recursive: true });
  fs.writeFileSync(
    path.join(stage, "package.json"),
    JSON.stringify({
      name: platform.name,
      version,
      os: [platform.os],
      cpu: [platform.cpu],
      ...(platform.libc === undefined ? {} : { libc: [platform.libc] }),
    }),
  );
  fs.writeFileSync(path.join(stage, "bin", "mara"), `#!/bin/sh\necho "mara ${version}"\n`, {
    mode: 0o755,
  });
  execFileSync("npm", ["pack", stage, "--pack-destination", out, "--ignore-scripts", "--offline"], {
    cwd: directory,
    env,
    stdio: "ignore",
  });
}

async function verify({ directory, env, script }, args) {
  const result = await run(script("verify-install.mjs"), args, { cwd: directory, env });
  const line = (name) => result.stdout.split("\n").find((text) => text.includes(name));
  return {
    result,
    docs: line("the installed dispatcher's docs/ equals the checkout's"),
    files: line(`the file list of ${dispatcher}`),
  };
}

test("the install verification compares the dispatcher's docs/ with the checkout's HEAD", async (t) => {
  const fixture = workspace(t);
  const packed = await pack(fixture);
  assert.equal(packed.status, 0, packed.stderr);
  nativeTarball(fixture, packed.out);

  // The stand-in binary fails the cases that run mara, but not the dispatcher's.
  const same = await verify(fixture, ["--tarballs", packed.out]);
  assert.equal(same.docs, "ok      the installed dispatcher's docs/ equals the checkout's");
  assert.equal(same.files, `ok      the file list of ${dispatcher}`);

  // HEAD moves on: one page changed, one removed, so the tarball's copy is changed and extra.
  fs.writeFileSync(path.join(fixture.repo, "docs", "[id].md"), "# Changed\n");
  fixture.git("rm", "--quiet", "docs/démarrage.md");
  fixture.git("commit", "--quiet", "--all", "--message", "change");
  const moved = await verify(fixture, ["--tarballs", packed.out]);
  for (const line of [moved.docs, moved.files]) {
    assert.match(line, /^FAILED /);
    assert.ok(line.includes("docs/[id].md differs from the packed commit's"), line);
    assert.ok(line.includes("unexpected docs/démarrage.md"), line);
  }
});

test("with --registry, a docs mismatch at another version names both versions", async (t) => {
  const registry = await startRegistry(t);
  const fixture = workspace(t, {}, "0.0.2");
  // The dispatcher of 0.0.1, from a workspace at 0.0.1 whose pages differ.
  const old = workspace(t);
  fs.writeFileSync(path.join(old.repo, "docs", "index.mara.md"), "# Older\n");
  old.git("commit", "--quiet", "--all", "--message", "older");
  const packed = await pack(old);
  assert.equal(packed.status, 0, packed.stderr);
  registry.add(
    "@convesoft/mara",
    { version, bin: { mara: "bin/mara.cjs" } },
    fs.readFileSync(packed.tarball),
  );

  const { docs } = await verify(
    { ...fixture, env: { ...fixture.env, npm_config_registry: registry.url } },
    [`--registry=${version}`],
  );
  assert.match(docs, /^FAILED /);
  assert.ok(docs.includes("docs/index.mara.md differs from the packed commit's"), docs);
  assert.ok(
    docs.includes(
      `the checkout is at version 0.0.2, not ${version}: run from a checkout of ${version}'s commit`,
    ),
    docs,
  );
});
