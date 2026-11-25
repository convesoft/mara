#!/usr/bin/env node
// Packs the npm packages of DES-PACKAGES into tarballs, and fails unless each holds exactly its
// files:
//
//   pack.mjs dispatcher <out-dir>
//   pack.mjs native <souffle> <souffle-licence> <out-dir>
//
// `native` packs the package of the host's platform, with the `mara` of Cargo's release profile in
// the artifact directory that `cargo metadata` names. Every package carries the workspace version.

import { execFileSync } from "node:child_process";
import {
  chmodSync,
  copyFileSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  renameSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import os from "node:os";
import path from "node:path";
import {
  blobContents,
  dispatcherContents,
  dispatcherName,
  dispatcherScript,
  hostPlatform,
  nativeFiles,
  platforms,
  root,
  tarballName,
  tarballProblems,
  workspace,
} from "./packages.mjs";
import { thirdPartyLicenses } from "./third-party-licenses.mjs";

const { version, repository, targetDirectory } = workspace();

function usage() {
  console.error(
    "usage: pack.mjs dispatcher <out-dir>\n" +
      "       pack.mjs native <souffle> <souffle-licence> <out-dir>",
  );
  process.exit(2);
}

function baseManifest(name, description, license) {
  return {
    name,
    version,
    description,
    license,
    repository: { type: "git", url: `git+${repository}.git` },
    homepage: repository,
  };
}

// Stages `sources`, a map from each packed path but `package.json` to its source file or
// `{ contents }`, with `manifest` as `package.json`; packs it with npm, offline and with an empty
// cache of its own; checks the tarball against `files`, the package's paths and modes, and
// against `blobs`, the contents of the dispatcher's `docs/` (`tarballProblems`); and only then
// renames it over the tarball of the same name in `outDir`. A failure leaves that tarball as it
// was and removes only what this run created.
function pack(outDir, manifest, sources, files, blobs) {
  const stage = mkdtempSync(path.join(os.tmpdir(), "mara-pack-"));
  const name = tarballName(manifest.name, version);
  let output;
  try {
    const contents = path.join(stage, "package");
    for (const [packed, source] of Object.entries(sources)) {
      const destination = path.join(contents, packed);
      mkdirSync(path.dirname(destination), { recursive: true });
      if (typeof source === "string") {
        try {
          copyFileSync(source, destination);
        } catch (error) {
          throw new Error(
            `cannot copy ${source} as ${packed} of ${manifest.name}: ${error.message}`,
          );
        }
      } else {
        writeFileSync(destination, source.contents);
      }
      chmodSync(destination, files[packed] ?? 0o644);
    }
    writeFileSync(
      path.join(contents, "package.json"),
      `${JSON.stringify({ ...manifest, files: Object.keys(sources) }, null, 2)}\n`,
    );

    mkdirSync(outDir, { recursive: true });
    output = mkdtempSync(path.join(outDir, `.${name}-`));
    const flags = ["--ignore-scripts", "--offline", "--no-update-notifier"];
    execFileSync(
      "npm",
      [
        "pack",
        contents,
        "--pack-destination",
        output,
        ...flags,
        "--cache",
        path.join(stage, "npm-cache"),
      ],
      { stdio: ["ignore", "ignore", "inherit"] },
    );
    const packed = path.join(output, name);
    const { problems } = tarballProblems(packed, files, blobs);
    if (problems.length > 0) {
      throw new Error(`the tarball of ${manifest.name} is wrong: ${problems.join("; ")}`);
    }
    renameSync(packed, path.join(outDir, name));
    process.stdout.write(`${path.join(outDir, name)}\n`);
  } finally {
    if (output !== undefined) {
      rmSync(output, { recursive: true, force: true });
    }
    rmSync(stage, { recursive: true, force: true });
  }
}

function packDispatcher(outDir) {
  const manifest = {
    ...baseManifest(
      dispatcherName,
      "Mara: engineering knowledge in Markdown, as a CLI, an MCP server and a daemon",
      "MIT OR Apache-2.0",
    ),
    bin: { mara: "bin/mara.cjs" },
    engines: { node: ">=24" },
    optionalDependencies: Object.fromEntries(platforms.map((platform) => [platform.name, version])),
  };
  // `docs/` as Git tracks it at HEAD, read from Git's objects (ADR-BUNDLED-DOCS).
  const { files, blobs } = dispatcherContents();
  const sources = {
    "bin/mara.cjs": dispatcherScript,
    "README.md": path.join(root, "README.md"),
    "LICENSE-MIT": path.join(root, "LICENSE-MIT"),
    "LICENSE-APACHE": path.join(root, "LICENSE-APACHE"),
  };
  for (const [file, contents] of blobContents(blobs)) {
    sources[file] = { contents };
  }
  pack(outDir, manifest, sources, files, blobs);
}

function packNative(souffle, souffleLicence, outDir) {
  const platform = hostPlatform();
  if (process.env.CARGO_BUILD_TARGET) {
    throw new Error(
      `CARGO_BUILD_TARGET is ${process.env.CARGO_BUILD_TARGET}; the native package is the host's ` +
        "build, so unset it",
    );
  }
  const mara = path.join(targetDirectory, "release", "mara");
  if (!existsSync(mara)) {
    throw new Error(
      `no release build of mara at ${mara}, in the artifact directory that cargo metadata names: ` +
        "run nx run mara-npm:pack-native:release",
    );
  }
  const reported = execFileSync(mara, ["--version"], { encoding: "utf8" }).trim();
  if (reported !== `mara ${version}`) {
    throw new Error(`${mara} --version prints "${reported}", not "mara ${version}"`);
  }
  const manifest = {
    ...baseManifest(
      platform.name,
      `The mara binary and Soufflé for ${platform.label}, run by ${dispatcherName}`,
      "(MIT OR Apache-2.0) AND UPL-1.0",
    ),
    os: [platform.os],
    cpu: [platform.cpu],
    ...(platform.libc === undefined ? {} : { libc: [platform.libc] }),
  };
  const sources = {
    "bin/mara": mara,
    "bin/souffle": souffle,
    "LICENSE-MIT": path.join(root, "LICENSE-MIT"),
    "LICENSE-APACHE": path.join(root, "LICENSE-APACHE"),
    "LICENSE-SOUFFLE": souffleLicence,
    "THIRD-PARTY-LICENSES": { contents: thirdPartyLicenses(root, "mara", version) },
  };
  pack(outDir, manifest, sources, nativeFiles);
}

const [command, ...args] = process.argv.slice(2);
try {
  if (command === "dispatcher" && args.length === 1) {
    packDispatcher(path.resolve(args[0]));
  } else if (command === "native" && args.length === 3) {
    const [souffle, souffleLicence, outDir] = args.map((arg) => path.resolve(arg));
    packNative(souffle, souffleLicence, outDir);
  } else {
    usage();
  }
} catch (error) {
  console.error(`pack.mjs: ${error.message}`);
  process.exitCode = 1;
}
