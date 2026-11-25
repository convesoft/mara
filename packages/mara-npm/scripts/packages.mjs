// What the packaging script and the install verification share (DES-PACKAGES): the workspace's
// version, repository and artifact directory, the platforms, and the exact file list of each package.

import { execFileSync } from "node:child_process";
import { lstatSync, mkdtempSync, readdirSync, rmSync } from "node:fs";
import { createRequire } from "node:module";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

export const project = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
export const root = path.resolve(project, "..", "..");
export const dispatcherScript = path.join(project, "bin", "mara.cjs");
export const { minimumGlibc, platforms } = createRequire(import.meta.url)(dispatcherScript);
export const dispatcherName = "@convesoft/mara";

// Each packed path with its mode; the dispatcher also holds `docs/`, from `dispatcherContents`.
export const dispatcherFiles = {
  "package.json": 0o644,
  "bin/mara.cjs": 0o755,
  "README.md": 0o644,
  "LICENSE-MIT": 0o644,
  "LICENSE-APACHE": 0o644,
};
// The Node library in the dispatcher (DES-NODE-LIBRARY), with the members of the dispatcher's
// manifest that let an application import it by the package's name. The library loads the
// native package's addon, `bin/mara.node`. mara-865a2e packs them; until then the host tests install them
// (`crates/mara/tests/hosts/node/support/library.mjs`).
export const libraryFiles = {
  "lib/index.js": 0o644,
  "lib/index.d.ts": 0o644,
  "lib/operations.d.ts": 0o644,
};
export const libraryManifest = {
  type: "module",
  exports: { ".": { types: "./lib/index.d.ts", default: "./lib/index.js" } },
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

// The parts of `buffer` between the bytes `separator`, without empty ones.
function split(buffer, separator) {
  const parts = [];
  let start = 0;
  for (let end = buffer.indexOf(separator); end !== -1; end = buffer.indexOf(separator, start)) {
    parts.push(buffer.subarray(start, end));
    start = end + 1;
  }
  parts.push(buffer.subarray(start));
  return parts.filter((part) => part.length > 0);
}

function git(repository, args, options = {}) {
  return execFileSync("git", ["-C", repository, ...args], {
    maxBuffer: 256 * 1024 * 1024,
    stdio: ["pipe", "pipe", "inherit"],
    ...options,
  });
}

// The files that Git tracks under `docs/` at HEAD of `repository`, the packed commit (ADR-BUNDLED-
// DOCS): a map from each path to its blob's object ID. Fails when a tracked entry is not a regular
// file, such as a symbolic link, or its name is not UTF-8 or holds a line break, and when Git
// reports the working tree's `docs/` as different from HEAD by a modified, staged, deleted,
// untracked or ignored file; files that a sparse checkout leaves out are not compared, and what is
// packed always comes from the commit.
export function trackedDocs(repository = root) {
  const docs = new Map();
  const others = [];
  const listing = git(repository, ["ls-tree", "-r", "-z", "HEAD", "--", "docs"]);
  for (const bytes of split(listing, 0)) {
    const entry = bytes.toString("utf8");
    const [, mode, type, id, file] = entry.match(/^(\d+) (\w+) (\S+)\t(.*)$/s);
    // The names are compared as UTF-8 text, and tar and `git hash-object --stdin-paths` list one
    // name per line.
    if (!Buffer.from(entry, "utf8").equals(bytes) || file.includes("\n")) {
      others.push(JSON.stringify(file));
    } else if (type === "blob" && (mode === "100644" || mode === "100755")) {
      docs.set(file, id);
    } else {
      others.push(`${file} (mode ${mode})`);
    }
  }
  if (others.length > 0) {
    throw new Error(
      `docs/ at HEAD holds entries that are not regular files, or whose name is not UTF-8 or holds a line break: ${others.join(", ")}`,
    );
  }
  if (docs.size === 0) {
    throw new Error(`Git tracks no file under docs/ at HEAD of ${repository}`);
  }
  const status = git(
    repository,
    ["status", "--porcelain=v1", "-z", "--untracked-files=all", "--ignored=matching", "--", "docs"],
    { encoding: "utf8" },
  )
    .split("\0")
    .filter(Boolean);
  if (status.length > 0) {
    throw new Error(
      `docs/ in the working tree differs from HEAD, which is what is packed; commit or remove: ${status.join(", ")}`,
    );
  }
  return docs;
}

// The dispatcher's paths with their modes and the blob that each file of `docs/` must equal.
export function dispatcherContents(repository = root) {
  const blobs = trackedDocs(repository);
  const files = { ...dispatcherFiles };
  for (const file of blobs.keys()) {
    files[file] = 0o644;
  }
  return { files, blobs };
}

// The contents of each blob of `blobs`, a map from a path to an object ID, read from Git's object
// database with one `git cat-file --batch`, never from the working tree.
export function blobContents(blobs, repository = root) {
  const ids = [...blobs.values()];
  const output = git(repository, ["cat-file", "--batch"], { input: `${ids.join("\n")}\n` });
  const contents = new Map();
  let offset = 0;
  for (const [file, id] of blobs) {
    const end = output.indexOf(0x0a, offset);
    const header = output.subarray(offset, end).toString("utf8");
    const match = header.match(/^(\S+) blob (\d+)$/);
    if (match === null || match[1] !== id) {
      throw new Error(`git cat-file answered "${header}" for ${file}, the blob ${id}`);
    }
    const size = Number(match[2]);
    contents.set(file, output.subarray(end + 1, end + 1 + size));
    offset = end + 1 + size + 1;
  }
  return contents;
}

// The object ID that Git gives each file's contents, without filters, as `git hash-object`.
function blobIds(files, repository) {
  if (files.length === 0) {
    return [];
  }
  return git(repository, ["hash-object", "--no-filters", "--stdin-paths"], {
    input: `${files.join("\n")}\n`,
    encoding: "utf8",
  })
    .split("\n")
    .filter(Boolean);
}

// The problems of `docs` against `blobs`, the map of `dispatcherContents`: `docs` maps each path
// found to its file, or to undefined for an entry that is not a regular file.
function docsProblems(docs, blobs, repository) {
  const compared = [...docs].filter(([name, file]) => file !== undefined && blobs.has(name));
  const ids = blobIds(
    compared.map(([, file]) => file),
    repository,
  );
  return [
    ...[...blobs.keys()].filter((name) => !docs.has(name)).map((name) => `missing ${name}`),
    ...[...docs]
      .filter(([name, file]) => file === undefined && blobs.has(name))
      .map(([name]) => `${name} is not a regular file`),
    ...[...docs].filter(([name]) => !blobs.has(name)).map(([name]) => `unexpected ${name}`),
    ...compared
      .filter(([name], index) => ids[index] !== blobs.get(name))
      .map(([name]) => `${name} differs from the packed commit's`),
  ];
}

// The problems of an installed package's `docs/` folder against `blobs`, without following a
// symbolic link: a missing, unexpected or changed file, or an entry that is not a regular file.
export function installedDocsProblems(directory, blobs, repository = root) {
  const docs = new Map();
  const walk = (relative) => {
    for (const entry of readdirSync(path.join(directory, relative), { withFileTypes: true })) {
      const name = path.posix.join(relative, entry.name);
      if (entry.isDirectory()) {
        walk(name);
      } else {
        docs.set(name, entry.isFile() ? path.join(directory, name) : undefined);
      }
    }
  };
  let folder;
  try {
    folder = lstatSync(path.join(directory, "docs"));
  } catch {
    return ["no docs/"];
  }
  if (!folder.isDirectory()) {
    return ["docs is not a directory"];
  }
  walk("docs");
  return docsProblems(docs, blobs, repository);
}

const tarEscapes = { a: 0x07, b: 0x08, f: 0x0c, n: 0x0a, r: 0x0d, t: 0x09, v: 0x0b, "\\": 0x5c };
const utf8 = new TextDecoder("utf-8", { fatal: true });

// A name as tar lists it in the C locale, decoded into the name's bytes and read as UTF-8. Throws,
// naming the listed line, on a backslash before anything but those escapes or three octal digits,
// on an octal value above 0o377, and on bytes that are not UTF-8: nothing is guessed.
export function fromTarListing(listed) {
  const bytes = [];
  const fail = (reason) => new Error(`tar listed ${JSON.stringify(listed)}, ${reason}`);
  for (let index = 0; index < listed.length; index += 1) {
    const code = listed.charCodeAt(index);
    if (code !== 0x5c) {
      bytes.push(code);
    } else if (/^[0-7]{3}$/.test(listed.slice(index + 1, index + 4))) {
      const value = Number.parseInt(listed.slice(index + 1, index + 4), 8);
      if (value > 0o377) {
        throw fail(`whose escape \\${listed.slice(index + 1, index + 4)} is above \\377`);
      }
      bytes.push(value);
      index += 3;
    } else if (Object.hasOwn(tarEscapes, listed[index + 1] ?? "")) {
      bytes.push(tarEscapes[listed[index + 1]]);
      index += 1;
    } else {
      throw fail("with a backslash that starts no escape of tar's");
    }
  }
  try {
    return utf8.decode(Uint8Array.from(bytes));
  } catch {
    throw fail("which is not UTF-8");
  }
}

// The file name that `npm pack` gives a package's tarball.
export function tarballName(name, version) {
  return `${name.replace(/^@/, "").replace("/", "-")}-${version}.tgz`;
}

// The packages for any platform, which one job packs: the dispatcher last, because the release
// publishes them in this order after every native package, and the dispatcher's optional
// dependencies name the native packages.
export const anyPlatformPackages = [dispatcherName];

// Every package of a version, in the release workflow's order (DES-RELEASE-WORKFLOW): the native
// packages of `platforms`, which a Build job each packs, then `anyPlatformPackages`.
export function releasePackages(version) {
  return [
    ...platforms.map((platform) => ({ name: platform.name, native: true })),
    ...anyPlatformPackages.map((name) => ({ name, native: false })),
  ].map((entry) => ({ ...entry, tarball: tarballName(entry.name, version) }));
}

// The problems of a tarball's file list against `files`: a missing, unexpected or repeated file,
// or a file of another mode or type. `command` and `output` are the listing that was checked.
// `tar -tzf` gives each member's name on a line of its own, spaces included; `tar -tvzf` lists the
// members in the same order and starts each line with the type and mode, on GNU and BSD tar. Both
// run in the C locale, where GNU tar and bsdtar write a backslash as `\\`, a control character as
// a C escape and every other byte outside printable ASCII as `\ooo`; `fromTarListing` decodes that
// into the name's UTF-8 text, so that the caller's locale changes nothing.
// `blobs`, from `dispatcherContents`, adds the comparison of the tarball's `docs/` with the packed
// commit, on a copy that tar extracts into a new temporary directory.
export function tarballProblems(tarball, files, blobs = new Map()) {
  const list = (flags) =>
    execFileSync("tar", [flags, tarball], {
      encoding: "latin1",
      env: { ...process.env, LC_ALL: "C" },
    })
      .split("\n")
      .filter(Boolean);
  // A line that does not decode is a problem, and stays as listed so that it matches no file.
  const undecoded = [];
  const names = list("-tzf").map((line) => {
    try {
      return fromTarListing(line);
    } catch (error) {
      undecoded.push(error.message);
      return line;
    }
  });
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
    ...undecoded,
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
  if (blobs.size > 0) {
    const extracted = mkdtempSync(path.join(os.tmpdir(), "mara-docs-"));
    try {
      execFileSync("tar", ["-xzf", tarball, "-C", extracted]);
      const docs = new Map(
        entries
          .filter((entry) => entry.path.startsWith("docs/"))
          .map((entry) => [
            entry.path,
            entry.mode.startsWith("-") ? path.join(extracted, "package", entry.path) : undefined,
          ]),
      );
      // A missing or unexpected file is also a problem of the list above, named alike.
      for (const problem of docsProblems(docs, blobs, root)) {
        if (!problems.includes(problem)) {
          problems.push(problem);
        }
      }
    } finally {
      rmSync(extracted, { recursive: true, force: true });
    }
  }
  return {
    problems,
    command: `tar -tvzf ${tarball}`,
    output: `${verbose.join("\n")}\n`,
  };
}
