#!/usr/bin/env node
// Packs the npm packages of DES-PACKAGES into tarballs, and fails unless each holds exactly its
// files:
//
//   pack.mjs dispatcher <souffle-wasm-dir> <out-dir>
//   pack.mjs native <souffle> <souffle-licence> <out-dir>
//
// Each reads the builds of Cargo's release profile in the artifact directory that `cargo metadata`
// names: `dispatcher` the engine's WebAssembly module `wasm32-unknown-unknown/release/mara_wasm.wasm`,
// with Soufflé's module `souffle.js` and `souffle.wasm` and its `LICENSE` from <souffle-wasm-dir>;
// `native`, the package of the host's platform, `release/mara` and the addon
// `release/libmara_node.so`. Every package carries the workspace version.

import { execFileSync } from "node:child_process";
import {
  chmodSync,
  copyFileSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  renameSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import os from "node:os";
import path from "node:path";
import {
  blobContents,
  browserFiles,
  dispatcherContents,
  dispatcherName,
  dispatcherScript,
  hostPlatform,
  libraryFiles,
  libraryManifest,
  nativeFiles,
  platforms,
  root,
  tarballName,
  tarballProblems,
  workspace,
} from "./packages.mjs";
import { hostTriple, thirdPartyLicenses } from "./third-party-licenses.mjs";

const { version, repository, targetDirectory } = workspace();

function usage() {
  console.error(
    "usage: pack.mjs dispatcher <souffle-wasm-dir> <out-dir>\n" +
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
// `{ contents }`, with `manifest` as `package.json`, whose `files` lists the sources unless the
// manifest names them itself; packs it with npm, offline and with an empty
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
      `${JSON.stringify({ ...manifest, files: manifest.files ?? Object.keys(sources) }, null, 2)}\n`,
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

// A build that the packaging script reads, which must exist: `what` names it in the error.
function built(file, what, target) {
  if (!existsSync(file)) {
    throw new Error(
      `no ${what} at ${file}, in the artifact directory that cargo metadata names: run nx run ${target}`,
    );
  }
  return file;
}

// The engine's WebAssembly module, which must export the functions that the browser's worker calls
// (ADR-RAW-WASM-MODULE).
function engineModule() {
  const file = built(
    path.join(targetDirectory, "wasm32-unknown-unknown", "release", "mara_wasm.wasm"),
    "release build of mara-wasm for wasm32-unknown-unknown",
    "mara-npm:pack-dispatcher",
  );
  let exported;
  try {
    exported = WebAssembly.Module.exports(new WebAssembly.Module(readFileSync(file))).map(
      (entry) => entry.name,
    );
  } catch (error) {
    throw new Error(`${file} is no WebAssembly module: ${error.message}`);
  }
  const missing = ["memory", "mara_alloc", "mara_call", "mara_answer", "mara_answer_length"].filter(
    (name) => !exported.includes(name),
  );
  if (missing.length > 0) {
    throw new Error(`${file} does not export ${missing.join(", ")}`);
  }
  return file;
}

function packDispatcher(souffleWasm, outDir) {
  const manifest = {
    ...baseManifest(
      dispatcherName,
      "Mara: engineering knowledge in Markdown, as a CLI, an MCP server, a daemon, a Node library " +
        "and a browser workspace",
      "(MIT OR Apache-2.0) AND UPL-1.0",
    ),
    ...libraryManifest,
    bin: { mara: "bin/mara.cjs" },
    engines: { node: ">=24" },
    optionalDependencies: Object.fromEntries(platforms.map((platform) => [platform.name, version])),
    // Each file, and the folder `docs`, not each page, which npm would read as a pattern
    // (ADR-BUNDLED-DOCS).
    files: [
      "bin/mara.cjs",
      ...Object.keys(libraryFiles),
      ...Object.keys(browserFiles),
      "README.md",
      "LICENSE-MIT",
      "LICENSE-APACHE",
      "LICENSE-SOUFFLE",
      "THIRD-PARTY-LICENSES",
      "docs",
    ],
  };
  // `docs/` as Git tracks it at HEAD, read from Git's objects (ADR-BUNDLED-DOCS).
  const { files, blobs } = dispatcherContents();
  const browserSource = path.join(root, "packages", "mara-browser");
  const sources = {
    "bin/mara.cjs": dispatcherScript,
    ...Object.fromEntries(
      Object.keys(libraryFiles).map((file) => [
        file,
        path.join(root, "packages", "mara-npm", file),
      ]),
    ),
    ...Object.fromEntries(
      Object.keys(browserFiles).map((file) => [
        file,
        path.join(browserSource, path.posix.basename(file)),
      ]),
    ),
    "browser/operations.d.ts": path.join(root, "packages", "mara-npm", "lib", "operations.d.ts"),
    "browser/mara.wasm": engineModule(),
    "browser/souffle.js": path.join(souffleWasm, "souffle.js"),
    "browser/souffle.wasm": path.join(souffleWasm, "souffle.wasm"),
    "README.md": path.join(root, "README.md"),
    "LICENSE-MIT": path.join(root, "LICENSE-MIT"),
    "LICENSE-APACHE": path.join(root, "LICENSE-APACHE"),
    "LICENSE-SOUFFLE": path.join(souffleWasm, "LICENSE"),
    "THIRD-PARTY-LICENSES": {
      contents: thirdPartyLicenses(root, ["mara-wasm"], version, "wasm32-unknown-unknown"),
    },
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
  const mara = built(
    path.join(targetDirectory, "release", "mara"),
    "release build of mara",
    "mara-npm:pack-native:release",
  );
  const addon = built(
    path.join(targetDirectory, "release", "libmara_node.so"),
    "release build of the Node addon mara-node",
    "mara-npm:pack-native:release",
  );
  const reported = execFileSync(mara, ["--version"], { encoding: "utf8" }).trim();
  if (reported !== `mara ${version}`) {
    throw new Error(`${mara} --version prints "${reported}", not "mara ${version}"`);
  }
  const manifest = {
    ...baseManifest(
      platform.name,
      `The mara binary, its Node addon and Soufflé for ${platform.label}, run by ${dispatcherName}`,
      "(MIT OR Apache-2.0) AND UPL-1.0",
    ),
    os: [platform.os],
    cpu: [platform.cpu],
    ...(platform.libc === undefined ? {} : { libc: [platform.libc] }),
  };
  const sources = {
    "bin/mara": mara,
    "bin/souffle": souffle,
    "bin/mara.node": addon,
    "LICENSE-MIT": path.join(root, "LICENSE-MIT"),
    "LICENSE-APACHE": path.join(root, "LICENSE-APACHE"),
    "LICENSE-SOUFFLE": souffleLicence,
    "THIRD-PARTY-LICENSES": {
      contents: thirdPartyLicenses(root, ["mara", "mara-node"], version, hostTriple(root)),
    },
  };
  pack(outDir, manifest, sources, nativeFiles);
}

const [command, ...args] = process.argv.slice(2);
try {
  if (command === "dispatcher" && args.length === 2) {
    const [souffleWasm, outDir] = args.map((arg) => path.resolve(arg));
    packDispatcher(souffleWasm, outDir);
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
