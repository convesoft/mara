#!/usr/bin/env node

import { chmod, copyFile, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const targets = new Map([
  [
    "x86_64-unknown-linux-gnu",
    {
      name: "@convesoft/mara-linux-x64-gnu",
      os: ["linux"],
      cpu: ["x64"],
      libc: ["glibc"],
    },
  ],
  [
    "aarch64-unknown-linux-gnu",
    {
      name: "@convesoft/mara-linux-arm64-gnu",
      os: ["linux"],
      cpu: ["arm64"],
      libc: ["glibc"],
    },
  ],
  [
    "x86_64-apple-darwin",
    {
      name: "@convesoft/mara-darwin-x64",
      os: ["darwin"],
      cpu: ["x64"],
    },
  ],
  [
    "aarch64-apple-darwin",
    {
      name: "@convesoft/mara-darwin-arm64",
      os: ["darwin"],
      cpu: ["arm64"],
    },
  ],
]);

function usage() {
  console.error(
    "usage: package-npm.mjs main <output-dir> | platform <target> <binary> <output-dir>",
  );
  process.exit(2);
}

async function workspaceVersion() {
  const cargo = await readFile(path.join(repositoryRoot, "Cargo.toml"), "utf8");
  const workspace = cargo.match(/\[workspace\.package\]([\s\S]*?)(?:\n\[|$)/);
  const version = workspace?.[1].match(/^version\s*=\s*"([^"]+)"\s*$/m)?.[1];
  if (version === undefined) {
    throw new Error("could not read [workspace.package].version from Cargo.toml");
  }
  return version;
}

function baseManifest(name, version, description) {
  return {
    name,
    version,
    description,
    author: "Aliaksei Raketski",
    license: "MIT OR Apache-2.0",
    repository: {
      type: "git",
      url: "git+https://github.com/convesoft/mara.git",
    },
    publishConfig: { access: "public" },
  };
}

function directoryName(packageName) {
  return packageName.replace("@convesoft/", "");
}

async function resetPackage(outputRoot, packageName) {
  const destination = path.join(path.resolve(outputRoot), directoryName(packageName));
  await rm(destination, { recursive: true, force: true });
  await mkdir(path.join(destination, "bin"), { recursive: true });
  return destination;
}

async function copyCommonFiles(destination) {
  await Promise.all(
    ["README.md", "LICENSE-MIT", "LICENSE-APACHE"].map((file) =>
      copyFile(path.join(repositoryRoot, file), path.join(destination, file)),
    ),
  );
}

async function writeManifest(destination, manifest) {
  await writeFile(
    path.join(destination, "package.json"),
    `${JSON.stringify(manifest, null, 2)}\n`,
  );
}

async function packageMain(outputRoot, version) {
  const packageName = "@convesoft/mara";
  const destination = await resetPackage(outputRoot, packageName);
  const optionalDependencies = Object.fromEntries(
    [...targets.values()].map(({ name }) => [name, version]),
  );
  const manifest = {
    ...baseManifest(
      packageName,
      version,
      "Structured project knowledge CLI and MCP server",
    ),
    bin: { mara: "bin/mara.cjs" },
    engines: { node: ">=18" },
    optionalDependencies,
    files: ["bin/mara.cjs", "README.md", "LICENSE-MIT", "LICENSE-APACHE"],
  };

  await copyFile(path.join(repositoryRoot, "npm/mara.cjs"), path.join(destination, "bin/mara.cjs"));
  await chmod(path.join(destination, "bin/mara.cjs"), 0o755);
  await copyCommonFiles(destination);
  await writeManifest(destination, manifest);
  process.stdout.write(`${destination}\n`);
}

async function packagePlatform(target, binary, outputRoot, version) {
  const configuration = targets.get(target);
  if (configuration === undefined) {
    throw new Error(`unsupported Rust target: ${target}`);
  }

  const destination = await resetPackage(outputRoot, configuration.name);
  const manifest = {
    ...baseManifest(
      configuration.name,
      version,
      `Mara native binary for ${target}`,
    ),
    os: configuration.os,
    cpu: configuration.cpu,
    ...(configuration.libc === undefined ? {} : { libc: configuration.libc }),
    files: ["bin/mara", "README.md", "LICENSE-MIT", "LICENSE-APACHE"],
  };

  await copyFile(path.resolve(binary), path.join(destination, "bin/mara"));
  await chmod(path.join(destination, "bin/mara"), 0o755);
  await copyCommonFiles(destination);
  await writeManifest(destination, manifest);
  process.stdout.write(`${destination}\n`);
}

const [command, ...arguments_] = process.argv.slice(2);
const version = await workspaceVersion();

if (command === "main" && arguments_.length === 1) {
  await packageMain(arguments_[0], version);
} else if (command === "platform" && arguments_.length === 3) {
  await packagePlatform(arguments_[0], arguments_[1], arguments_[2], version);
} else {
  usage();
}
