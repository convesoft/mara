// The real packaging script and install verification on the packages' files (DES-PACKAGES,
// ADR-BUNDLED-DOCS, VER-INSTALL-WITHOUT-TOOLCHAIN): run in a disposable workspace, a Git repository
// with a copy of packages/mara-npm and the browser workspace's sources, a Cargo workspace whose
// `cargo metadata` gives the version, and stand-ins of the builds in its `target/`. Every child
// starts in the test's temporary directory, so that no project `.npmrc` of the caller applies.

import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { test } from "node:test";
import {
  browserFiles,
  fromTarListing,
  hostPlatform,
  libraryFiles,
  nativeFiles,
  root,
  tarballName,
} from "../scripts/packages.mjs";
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

// A WebAssembly module that exports `memory` and the functions `names`, each taking and returning
// nothing: a stand-in of the engine's module for the packer's check of its exports.
function wasmModule(names) {
  const vector = (items) => [items.length, ...items.flat()];
  const section = (id, content) => [id, content.length, ...content];
  const text = (name) => [name.length, ...Buffer.from(name)];
  // The magic number and version 1.
  const magic = [0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];
  return Buffer.from([
    ...magic,
    ...section(1, vector([[0x60, 0x00, 0x00]])),
    ...section(3, vector(names.map(() => [0x00]))),
    ...section(5, vector([[0x00, 0x01]])),
    ...section(
      7,
      vector([
        [...text("memory"), 0x02, 0x00],
        ...names.map((name, index) => [...text(name), 0x00, index]),
      ]),
    ),
    ...section(10, vector(names.map(() => [0x02, 0x00, 0x0b]))),
  ]);
}
const engineExports = ["mara_alloc", "mara_call", "mara_answer", "mara_answer_length"];
const engine = "target/wasm32-unknown-unknown/release/mara_wasm.wasm";

// The workspace, committed, with `pages` under docs/ and `extra` files anywhere, and the builds that
// `pack.mjs dispatcher` reads in `target/`, which Git ignores.
function workspace(t, extra = {}, cargoVersion = version, builds = {}) {
  const directory = temporaryDirectory(t);
  const repo = path.join(directory, "repo");
  const env = environment(directory, { LC_ALL: "C" });
  const project = path.join(repo, "packages", "mara-npm");
  for (const folder of ["bin", "scripts", "lib", "licenses"]) {
    fs.cpSync(path.join(root, "packages", "mara-npm", folder), path.join(project, folder), {
      recursive: true,
    });
  }
  // `run` starts a script by its path.
  for (const name of ["pack.mjs", "verify-install.mjs"]) {
    fs.chmodSync(path.join(project, "scripts", name), 0o755);
  }
  const sources = [
    "rust-toolchain.toml",
    "README.md",
    "LICENSE-MIT",
    "LICENSE-APACHE",
    // The sources of the browser workspace; the packer takes the built files from `target/`.
    ...["index.js", "index.d.ts", "errors.js", "daemon.js", "worker.js", "souffle-worker.js"].map(
      (file) => `packages/mara-browser/${file}`,
    ),
    "packages/mara-browser/output-limit.js",
  ];
  for (const file of sources) {
    fs.mkdirSync(path.dirname(path.join(repo, file)), { recursive: true });
    fs.copyFileSync(path.join(root, file), path.join(repo, file));
  }
  const crate = (name, main) => ({
    [`crates/${name}/Cargo.toml`]:
      `[package]\nname = "${name}"\nversion = "${cargoVersion}"\nedition = "2024"\n` +
      `repository = "https://example.invalid/mara"\n`,
    [`crates/${name}/src/${main ? "main" : "lib"}.rs`]: main ? "fn main() {}\n" : "\n",
  });
  const files = {
    ".gitignore": "/target/\n",
    "Cargo.toml": `[workspace]\nmembers = ["crates/*"]\nresolver = "3"\n`,
    ...crate("mara", true),
    ...crate("mara-node", false),
    ...crate("mara-wasm", false),
    ...pages,
    ...extra,
  };
  const built = {
    [engine]: wasmModule(engineExports),
    "target/souffle-2.5-wasm/souffle.js": "export default () => {};\n",
    "target/souffle-2.5-wasm/souffle.wasm": "souffle.wasm\n",
    "target/souffle-2.5-wasm/LICENSE": "The licence of Soufflé\n",
    ...builds,
  };
  for (const [file, contents] of Object.entries({ ...files, ...built })) {
    fs.mkdirSync(path.dirname(path.join(repo, file)), { recursive: true });
    fs.writeFileSync(path.join(repo, file), contents);
  }
  // The licence inventory reads the workspace with `--locked`; the crates have no dependency.
  execFileSync("cargo", ["generate-lockfile", "--offline"], { cwd: repo, env, stdio: "ignore" });
  const git = (...args) =>
    execFileSync("git", ["-C", repo, ...args], { cwd: directory, env, encoding: "utf8" });
  git("init", "--quiet");
  git("add", ".");
  git("commit", "--quiet", "--message", "workspace");
  const script = (name) => path.join(project, "scripts", name);
  return { directory, repo, env, git, script };
}

const extract = (tarball, file, { directory }, encoding = "utf8") =>
  execFileSync("tar", ["-xzOf", tarball, `package/${file}`], { cwd: directory, encoding });

async function pack({ directory, repo, env, script }, out = path.join(directory, "out")) {
  const souffleWasm = path.join(repo, "target", "souffle-2.5-wasm");
  const result = await run(script("pack.mjs"), ["dispatcher", souffleWasm, out], {
    cwd: directory,
    env,
  });
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
  const manifest = JSON.parse(extract(result.tarball, "package.json", fixture));
  assert.deepEqual(manifest.files, [
    "bin/mara.cjs",
    ...Object.keys(libraryFiles),
    ...Object.keys(browserFiles),
    "README.md",
    "LICENSE-MIT",
    "LICENSE-APACHE",
    "LICENSE-SOUFFLE",
    "THIRD-PARTY-LICENSES",
    "docs",
  ]);
});

test("the dispatcher holds the Node library, the browser workspace with both modules, and their licences", async (t) => {
  const fixture = workspace(t);
  const result = await pack(fixture);
  assert.equal(result.status, 0, result.stderr);
  const manifest = JSON.parse(extract(result.tarball, "package.json", fixture));
  assert.equal(manifest.license, "(MIT OR Apache-2.0) AND UPL-1.0");
  assert.equal(manifest.type, "module");
  assert.deepEqual(manifest.exports, {
    ".": { types: "./lib/index.d.ts", default: "./lib/index.js" },
    "./browser": { types: "./browser/index.d.ts", default: "./browser/index.js" },
  });
  assert.equal(manifest.scripts, undefined);
  assert.equal(manifest.dependencies, undefined);
  const same = (file, source) =>
    assert.ok(
      Buffer.from(extract(result.tarball, file, fixture, "buffer")).equals(
        fs.readFileSync(path.join(fixture.repo, source)),
      ),
      `${file} differs from ${source}`,
    );
  same("browser/mara.wasm", engine);
  same("browser/souffle.js", "target/souffle-2.5-wasm/souffle.js");
  same("browser/souffle.wasm", "target/souffle-2.5-wasm/souffle.wasm");
  same("LICENSE-SOUFFLE", "target/souffle-2.5-wasm/LICENSE");
  same("browser/worker.js", "packages/mara-browser/worker.js");
  same("browser/operations.d.ts", "packages/mara-npm/lib/operations.d.ts");
  same("lib/index.js", "packages/mara-npm/lib/index.js");
  assert.match(
    extract(result.tarball, "THIRD-PARTY-LICENSES", fixture),
    new RegExp(`^Third-party licences of mara-wasm ${version} for wasm32-unknown-unknown\n`),
  );
});

for (const [name, contents, problem] of [
  ["that is no WebAssembly module", "not wasm\n", /mara_wasm\.wasm is no WebAssembly module/],
  [
    "without a function that the worker calls",
    wasmModule(engineExports.filter((name) => name !== "mara_call")),
    /mara_wasm\.wasm does not export mara_call/,
  ],
]) {
  test(`the packaging script refuses an engine module ${name} and writes no tarball`, async (t) => {
    const fixture = workspace(t, {}, version, { [engine]: contents });
    const result = await pack(fixture);
    assert.equal(result.status, 1, result.stdout);
    assert.match(result.stderr, problem);
    assert.equal(fs.existsSync(result.tarball), false);
  });
}

// `pack.mjs native` on stand-ins of `release/mara`, which prints the version, of the addon
// `release/libmara_node.so` and of Soufflé.
async function packNative(fixture, { addon = true } = {}) {
  const release = path.join(fixture.repo, "target", "release");
  fs.mkdirSync(release, { recursive: true });
  fs.writeFileSync(path.join(release, "mara"), `#!/bin/sh\necho "mara ${version}"\n`, {
    mode: 0o755,
  });
  if (addon) {
    fs.writeFileSync(path.join(release, "libmara_node.so"), "the addon\n", { mode: 0o755 });
  }
  const souffle = path.join(fixture.directory, "souffle");
  fs.writeFileSync(souffle, "souffle\n", { mode: 0o755 });
  const licence = path.join(fixture.directory, "LICENSE");
  fs.writeFileSync(licence, "The licence of Soufflé\n");
  const out = path.join(fixture.directory, "out");
  const result = await run(fixture.script("pack.mjs"), ["native", souffle, licence, out], {
    cwd: fixture.directory,
    env: fixture.env,
  });
  return { ...result, tarball: path.join(out, tarballName(hostPlatform().name, version)) };
}

test("a native package holds the addon as bin/mara.node beside bin/mara", async (t) => {
  const fixture = workspace(t);
  const result = await packNative(fixture);
  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(listing(result.tarball, fixture).sort(), Object.keys(nativeFiles).sort());
  assert.equal(extract(result.tarball, "bin/mara.node", fixture), "the addon\n");
  const modes = execFileSync("tar", ["-tvzf", result.tarball], { encoding: "utf8" });
  assert.match(modes, /^-rw-r--r-- .* package\/bin\/mara\.node$/m);
  assert.match(
    extract(result.tarball, "THIRD-PARTY-LICENSES", fixture),
    new RegExp(`^Third-party licences of mara and mara-node ${version} for `),
  );
});

test("packing a native package without the addon fails and writes no tarball", async (t) => {
  const fixture = workspace(t);
  const result = await packNative(fixture, { addon: false });
  assert.equal(result.status, 1, result.stdout);
  assert.match(result.stderr, /no release build of the Node addon mara-node at .*libmara_node\.so/);
  assert.equal(fs.existsSync(result.tarball), false);
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
