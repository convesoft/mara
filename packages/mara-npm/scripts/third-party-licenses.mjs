// `THIRD-PARTY-LICENSES` of a native package (DES-PACKAGES): the licence files of every crate
// that Cargo builds for the workspace's `mara` on the host, from `cargo metadata`.

import { execFileSync } from "node:child_process";
import { readdirSync, readFileSync } from "node:fs";
import path from "node:path";

// A licence file at the top of a crate, such as `LICENSE`, `LICENSE-MIT`, `COPYING`, `NOTICE` or
// `UNLICENSE`, or a file in a top-level `LICENSES` folder.
const licenceFile = /^(licen[cs]e|copying|copyright|notice|unlicen[cs]e)($|[-._])/i;
const licenceFolder = /^licen[cs]es$/i;

function hostTriple(root) {
  const output = execFileSync("rustc", ["-vV"], { cwd: root, encoding: "utf8" });
  const triple = output.match(/^host: (\S+)$/m)?.[1];
  if (triple === undefined) {
    throw new Error(`rustc -vV names no host triple:\n${output}`);
  }
  return triple;
}

function licenceFiles(crate) {
  const directory = path.dirname(crate.manifest_path);
  const files = [];
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    if (entry.isFile() && licenceFile.test(entry.name)) {
      files.push(path.join(directory, entry.name));
    } else if (entry.isDirectory() && licenceFolder.test(entry.name)) {
      for (const inner of readdirSync(path.join(directory, entry.name), { withFileTypes: true })) {
        if (inner.isFile()) {
          files.push(path.join(directory, entry.name, inner.name));
        }
      }
    }
  }
  if (crate.license_file) {
    files.push(path.resolve(directory, crate.license_file));
  }
  return [...new Set(files)].sort();
}

// The crates that ship no licence file, each with the crate of the same upstream repository whose
// licence files apply to it. Both must be in the build and declare the same licence. Wasmtime
// publishes its internal crates without its `LICENSE`.
const licenceFilesOf = {
  "cranelift-assembler-x64": "wasmtime",
  "cranelift-assembler-x64-meta": "wasmtime",
  "cranelift-bitset": "wasmtime",
  "cranelift-isle": "wasmtime",
  "cranelift-srcgen": "wasmtime",
  "pulley-interpreter": "wasmtime",
  "pulley-macros": "wasmtime",
  "wasmtime-c-api-impl": "wasmtime",
  "wasmtime-internal-c-api-macros": "wasmtime",
  "wasmtime-internal-component-util": "wasmtime",
  "wasmtime-internal-core": "wasmtime",
  "wasmtime-internal-jit-debug": "wasmtime",
  "wasmtime-internal-jit-icache-coherence": "wasmtime",
  "wasmtime-internal-versioned-export-macros": "wasmtime",
};

// The crates that Cargo builds for the workspace member `packageName` on the host: its normal and
// build dependencies, transitively, without workspace members. `cargo metadata` resolves the
// features of the whole workspace, so the set holds every crate of the build and may hold more.
function buildCrates(root, packageName, triple) {
  const metadata = JSON.parse(
    execFileSync(
      "cargo",
      ["metadata", "--format-version", "1", "--locked", "--offline", "--filter-platform", triple],
      { cwd: root, encoding: "utf8", maxBuffer: 256 * 1024 * 1024 },
    ),
  );
  const members = new Set(metadata.workspace_members);
  const packages = new Map(metadata.packages.map((crate) => [crate.id, crate]));
  const nodes = new Map(metadata.resolve.nodes.map((node) => [node.id, node]));
  const start = metadata.packages.find(
    (crate) => crate.name === packageName && members.has(crate.id),
  );
  if (start === undefined) {
    throw new Error(`the workspace has no member ${packageName}`);
  }
  const seen = new Set();
  const pending = [start.id];
  while (pending.length > 0) {
    const id = pending.pop();
    if (seen.has(id)) {
      continue;
    }
    seen.add(id);
    for (const dependency of nodes.get(id).deps) {
      if (dependency.dep_kinds.some((kind) => kind.kind === null || kind.kind === "build")) {
        pending.push(dependency.pkg);
      }
    }
  }
  return [...seen]
    .filter((id) => !members.has(id))
    .map((id) => packages.get(id))
    .sort((a, b) => a.name.localeCompare(b.name) || a.version.localeCompare(b.version));
}

// The text of `THIRD-PARTY-LICENSES`. Fails, naming the crates, when a crate ships no licence file
// and `licenceFilesOf` names no crate whose files apply to it.
export function thirdPartyLicenses(root, packageName, version) {
  const triple = hostTriple(root);
  const crates = buildCrates(root, packageName, triple).map((crate) => ({
    crate,
    files: licenceFiles(crate),
  }));

  const missing = [];
  for (const entry of crates) {
    if (entry.files.length > 0) {
      continue;
    }
    const source = crates.find(({ crate }) => crate.name === licenceFilesOf[entry.crate.name]);
    if (
      source !== undefined &&
      source.files.length > 0 &&
      source.crate.license === entry.crate.license
    ) {
      entry.files = source.files;
    } else {
      missing.push(
        `${entry.crate.name} ${entry.crate.version} (${entry.crate.license ?? "no licence"})`,
      );
    }
  }
  if (missing.length > 0) {
    throw new Error(
      `these crates ship no licence file: ${missing.join(", ")}. Give each the crate of its ` +
        "upstream repository whose licence files apply to it, in licenceFilesOf of " +
        "packages/mara-npm/scripts/third-party-licenses.mjs, after reading its licence",
    );
  }

  // Each distinct text once, with the crates that ship it.
  const texts = new Map();
  for (const { crate, files } of crates) {
    for (const file of files) {
      const text = readFileSync(file, "utf8").replace(/\r\n/g, "\n").trimEnd();
      const users = texts.get(text) ?? [];
      users.push(`${crate.name} ${crate.version}`);
      texts.set(text, users);
    }
  }

  const rule = "=".repeat(80);
  const lines = [
    `Third-party licences of mara ${version} for ${triple}`,
    "",
    `The licence inventory covers these ${crates.length} resolved normal and build dependencies.`,
    "",
    ...crates.map(
      ({ crate }) => `- ${crate.name} ${crate.version}: ${crate.license ?? "no licence declared"}`,
    ),
    "",
    "Their licence files follow, each text once, after the crates that ship it.",
  ];
  for (const [text, users] of texts) {
    lines.push("", rule, `Used by: ${[...new Set(users)].join(", ")}`, rule, "", text);
  }
  return `${lines.join("\n")}\n`;
}
