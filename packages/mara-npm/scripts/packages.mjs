// What the packaging script and the install verification share (DES-PACKAGES): the workspace's
// version, repository and artifact directory, the platforms, and the exact file list of each package.

import { execFileSync } from "node:child_process";
import { createRequire } from "node:module";
import path from "node:path";
import { fileURLToPath } from "node:url";

export const project = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
export const root = path.resolve(project, "..", "..");
export const dispatcherScript = path.join(project, "bin", "mara.cjs");
export const { minimumGlibc, platforms } = createRequire(import.meta.url)(dispatcherScript);
export const dispatcherName = "@convesoft/mara";

// Each packed path with its mode.
export const dispatcherFiles = {
  "package.json": 0o644,
  "bin/mara.cjs": 0o755,
  "README.md": 0o644,
  "LICENSE-MIT": 0o644,
  "LICENSE-APACHE": 0o644,
};
export const nativeFiles = {
  "package.json": 0o644,
  "bin/mara": 0o755,
  "bin/souffle": 0o755,
  "LICENSE-MIT": 0o644,
  "LICENSE-APACHE": 0o644,
  "LICENSE-SOUFFLE": 0o644,
  "THIRD-PARTY-LICENSES": 0o644,
};

// The workspace's `mara` as Cargo reads it: `cargo metadata --no-deps`. Its version is every
// package's version and must carry no build metadata, which npm drops (DES-PACKAGES); the
// artifact directory is where Cargo writes the builds, `CARGO_TARGET_DIR` and Cargo's
// configuration included.
export function workspace() {
  const metadata = JSON.parse(
    execFileSync("cargo", ["metadata", "--no-deps", "--format-version", "1", "--offline"], {
      cwd: root,
      encoding: "utf8",
      maxBuffer: 64 * 1024 * 1024,
    }),
  );
  const members = new Set(metadata.workspace_members);
  const mara = metadata.packages.find((crate) => crate.name === "mara" && members.has(crate.id));
  if (mara === undefined) {
    throw new Error(`the workspace at ${root} has no member mara`);
  }
  if (mara.version.includes("+")) {
    throw new Error(`the version ${mara.version} has build metadata, which npm drops`);
  }
  if (!mara.repository) {
    throw new Error(`the package mara of ${root} has no repository`);
  }
  return {
    version: mara.version,
    repository: mara.repository,
    targetDirectory: metadata.target_directory,
  };
}

export function hostPlatform() {
  const platform = platforms.find(
    (candidate) => candidate.os === process.platform && candidate.cpu === process.arch,
  );
  if (platform === undefined) {
    throw new Error(`no native package is defined for ${process.platform} ${process.arch}`);
  }
  return platform;
}

// The file name that `npm pack` gives a package's tarball.
export function tarballName(name, version) {
  return `${name.replace(/^@/, "").replace("/", "-")}-${version}.tgz`;
}

// The problems of a tarball's file list against `files`: a missing, unexpected or repeated file,
// or a file of another mode or type. `command` and `output` are the listing that was checked.
// `tar -tzf` gives each member's name on a line of its own, spaces included; `tar -tvzf` lists the
// members in the same order and starts each line with the type and mode, on GNU and BSD tar.
export function tarballProblems(tarball, files) {
  const list = (flags) =>
    execFileSync("tar", [flags, tarball], { encoding: "utf8" }).split("\n").filter(Boolean);
  const names = list("-tzf");
  const verbose = list("-tvzf");
  const modes = verbose.map((line) => line.trim().split(/\s+/)[0]);
  const entries = names.map((name, index) => ({
    path: name.replace(/^package\//, ""),
    mode: modes[index],
  }));
  const modeString = (mode) =>
    "-" +
    [6, 3, 0]
      .map((shift) => {
        const bits = (mode >> shift) & 7;
        return `${bits & 4 ? "r" : "-"}${bits & 2 ? "w" : "-"}${bits & 1 ? "x" : "-"}`;
      })
      .join("");
  const counts = new Map();
  for (const entry of entries) {
    counts.set(entry.path, (counts.get(entry.path) ?? 0) + 1);
  }
  const problems = [
    ...(names.length === modes.length ? [] : ["the two listings of tar differ in length"]),
    ...Object.keys(files)
      .filter((file) => !counts.has(file))
      .map((file) => `missing ${file}`),
    ...[...counts]
      .filter(([, count]) => count > 1)
      .map(([file, count]) => `${file} ${count} times`),
    ...entries.filter((entry) => !(entry.path in files)).map((entry) => `unexpected ${entry.path}`),
    ...entries
      .filter((entry) => entry.path in files && entry.mode !== modeString(files[entry.path]))
      .map((entry) => `${entry.path} is ${entry.mode}, not ${modeString(files[entry.path])}`),
  ];
  return {
    problems,
    command: `tar -tvzf ${tarball}`,
    output: `${verbose.join("\n")}\n`,
  };
}
