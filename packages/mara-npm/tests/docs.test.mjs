// The dispatcher's `docs/` (ADR-BUNDLED-DOCS, DES-PACKAGES): what packaging reads from Git, and the
// checks of a tarball and of an installed package against the packed commit, which the packaging
// script and the install verification run before anything is published.

import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { test } from "node:test";
import {
  blobContents,
  dispatcherContents,
  dispatcherFiles,
  installedDocsProblems,
  tarballProblems,
  trackedDocs,
} from "../scripts/packages.mjs";
import { environment, temporaryDirectory } from "./support.mjs";

// A repository whose HEAD tracks `docs/` with a nested page, a name with a blank and an executable
// page, and a file outside `docs/`.
function repository(t) {
  const directory = temporaryDirectory(t);
  const repo = path.join(directory, "repo");
  const env = environment(directory);
  const git = (...args) => execFileSync("git", ["-C", repo, ...args], { env, encoding: "utf8" });
  fs.mkdirSync(path.join(repo, "docs", "rules"), { recursive: true });
  fs.writeFileSync(path.join(repo, "docs", "index.mara.md"), "# Index\n");
  fs.writeFileSync(path.join(repo, "docs", "rules", "page one.mara.md"), "# Rules\n");
  fs.writeFileSync(path.join(repo, "docs", "run.mara.md"), "# Run\n", { mode: 0o755 });
  fs.writeFileSync(path.join(repo, "README.md"), "# Readme\n");
  fs.writeFileSync(path.join(repo, ".gitignore"), "*.ignored\n");
  git("init", "--quiet");
  git("add", ".");
  git("commit", "--quiet", "--message", "docs");
  return { directory, repo, git };
}

const pages = ["docs/index.mara.md", "docs/rules/page one.mara.md", "docs/run.mara.md"];

test("the docs are the files that Git tracks under docs/ at HEAD, with their blobs", (t) => {
  const { repo, git } = repository(t);
  const docs = trackedDocs(repo);
  assert.deepEqual([...docs.keys()].sort(), pages);
  for (const page of pages) {
    assert.equal(docs.get(page), git("rev-parse", `HEAD:${page}`).trim());
  }
  const { files, blobs } = dispatcherContents(repo);
  assert.deepEqual(files, {
    ...dispatcherFiles,
    ...Object.fromEntries(pages.map((p) => [p, 0o644])),
  });
  assert.equal(blobs.size, 3);
});

test("packaging reads the docs' contents from Git's objects, not from the working tree", (t) => {
  const { repo } = repository(t);
  const docs = trackedDocs(repo);
  // Changed after the check: what is read is still the commit's.
  fs.writeFileSync(path.join(repo, "docs", "index.mara.md"), "# Changed\n");
  const contents = blobContents(docs, repo);
  assert.equal(contents.get("docs/index.mara.md").toString(), "# Index\n");
  assert.equal(contents.get("docs/rules/page one.mara.md").toString(), "# Rules\n");
});

for (const [name, change, named] of [
  [
    "a modified page",
    ({ repo }) => fs.appendFileSync(path.join(repo, "docs", "index.mara.md"), "x"),
    "docs/index.mara.md",
  ],
  [
    "a staged page",
    ({ repo, git }) => {
      fs.appendFileSync(path.join(repo, "docs", "index.mara.md"), "x");
      git("add", "docs/index.mara.md");
    },
    "docs/index.mara.md",
  ],
  [
    "a deleted page",
    ({ repo }) => fs.rmSync(path.join(repo, "docs", "run.mara.md")),
    "docs/run.mara.md",
  ],
  [
    "an untracked page",
    ({ repo }) => fs.writeFileSync(path.join(repo, "docs", "new.mara.md"), "new"),
    "docs/new.mara.md",
  ],
  [
    "an ignored file",
    ({ repo }) => fs.writeFileSync(path.join(repo, "docs", "notes.ignored"), "x"),
    "docs/notes.ignored",
  ],
  [
    "an untracked symbolic link",
    ({ repo }) => fs.symlinkSync(path.join(repo, "README.md"), path.join(repo, "docs", "link.md")),
    "docs/link.md",
  ],
]) {
  test(`packaging refuses docs/ with ${name} in the working tree`, (t) => {
    const setup = repository(t);
    change(setup);
    assert.throws(
      () => trackedDocs(setup.repo),
      (error) => {
        assert.match(error.message, /docs\/ in the working tree differs from HEAD/);
        assert.ok(error.message.includes(named), error.message);
        return true;
      },
    );
  });
}

test("packaging refuses a symbolic link that Git tracks under docs/", (t) => {
  const { repo, git } = repository(t);
  fs.symlinkSync("../README.md", path.join(repo, "docs", "readme.md"));
  git("add", "docs/readme.md");
  git("commit", "--quiet", "--message", "link");
  assert.throws(() => trackedDocs(repo), /not regular files.*docs\/readme\.md \(mode 120000\)/);
});

test("packaging refuses a commit without docs/", (t) => {
  const { repo, git } = repository(t);
  git("rm", "--quiet", "-r", "docs");
  git("commit", "--quiet", "--message", "no docs");
  assert.throws(() => trackedDocs(repo), /Git tracks no file under docs\//);
});

// tar and `git hash-object --stdin-paths` list one name per line, and names compare as UTF-8.
for (const [name, file] of [
  ["a line break", Buffer.from("docs/two\nlines.md")],
  [
    "a byte that is not UTF-8",
    Buffer.from([...Buffer.from("docs/latin"), 0xe9, ...Buffer.from(".md")]),
  ],
]) {
  test(`packaging refuses a tracked page whose name holds ${name}`, (t) => {
    const { repo, git } = repository(t);
    fs.writeFileSync(Buffer.concat([Buffer.from(`${repo}/`), file]), "x");
    git("add", "docs");
    git("commit", "--quiet", "--message", "name");
    assert.throws(() => trackedDocs(repo), /whose name is not UTF-8 or holds a line break/);
  });
}

// A tarball as npm packs it: one member per file under `package/`, no directory members.
function tarball(directory, name, files) {
  const stage = path.join(directory, `stage-${name}`);
  for (const [file, { contents, mode = 0o644, link }] of Object.entries(files)) {
    const destination = path.join(stage, "package", file);
    fs.mkdirSync(path.dirname(destination), { recursive: true });
    if (link === undefined) {
      fs.writeFileSync(destination, contents);
      fs.chmodSync(destination, mode);
    } else {
      fs.symlinkSync(link, destination);
    }
  }
  const out = path.join(directory, `${name}.tgz`);
  const members = Object.keys(files).map((file) => `package/${file}`);
  execFileSync("tar", ["-czf", out, "-C", stage, "--no-recursion", ...members]);
  return out;
}

function packedFiles(repo, docs) {
  const files = {
    "package.json": { contents: "{}" },
    "bin/mara.cjs": { contents: "", mode: 0o755 },
    "README.md": { contents: "" },
    "LICENSE-MIT": { contents: "" },
    "LICENSE-APACHE": { contents: "" },
  };
  for (const page of pages) {
    files[page] = { contents: fs.readFileSync(path.join(repo, page)) };
  }
  return { ...files, ...docs };
}

test("the tarball check compares docs/ with the packed commit by name, mode and contents", (t) => {
  const { directory, repo } = repository(t);
  const { files, blobs } = dispatcherContents(repo);
  const problems = (name, docs) =>
    tarballProblems(tarball(directory, name, packedFiles(repo, docs)), files, blobs).problems;

  assert.deepEqual(problems("exact", {}), []);
  assert.deepEqual(problems("changed", { "docs/index.mara.md": { contents: "# Other\n" } }), [
    "docs/index.mara.md differs from the packed commit's",
  ]);
  const missing = packedFiles(repo, {});
  delete missing["docs/run.mara.md"];
  assert.deepEqual(tarballProblems(tarball(directory, "missing", missing), files, blobs).problems, [
    "missing docs/run.mara.md",
  ]);
  assert.deepEqual(problems("extra", { "docs/extra.mara.md": { contents: "x" } }), [
    "unexpected docs/extra.mara.md",
  ]);
  assert.deepEqual(problems("mode", { "docs/run.mara.md": { contents: "# Run\n", mode: 0o755 } }), [
    "docs/run.mara.md is -rwxr-xr-x, not -rw-r--r--",
  ]);
  const link = problems("link", { "docs/index.mara.md": { link: "../README.md" } });
  assert.ok(link.includes("docs/index.mara.md is not a regular file"), link.join("; "));
  assert.ok(
    link.some((problem) => problem.startsWith("docs/index.mara.md is l")),
    link.join("; "),
  );
});

test("the install verification compares the installed docs/ with the packed commit", (t) => {
  const { directory, repo } = repository(t);
  const blobs = trackedDocs(repo);
  const installed = path.join(directory, "installed");
  const install = () => {
    fs.rmSync(installed, { recursive: true, force: true });
    for (const page of pages) {
      fs.mkdirSync(path.dirname(path.join(installed, page)), { recursive: true });
      fs.copyFileSync(path.join(repo, page), path.join(installed, page));
    }
  };

  install();
  assert.deepEqual(installedDocsProblems(installed, blobs, repo), []);

  fs.appendFileSync(path.join(installed, "docs", "rules", "page one.mara.md"), "x");
  assert.deepEqual(installedDocsProblems(installed, blobs, repo), [
    "docs/rules/page one.mara.md differs from the packed commit's",
  ]);

  install();
  fs.rmSync(path.join(installed, "docs", "run.mara.md"));
  fs.writeFileSync(path.join(installed, "docs", "extra.md"), "x");
  assert.deepEqual(installedDocsProblems(installed, blobs, repo), [
    "missing docs/run.mara.md",
    "unexpected docs/extra.md",
  ]);

  // A link to a file with the right contents is not followed.
  install();
  fs.rmSync(path.join(installed, "docs", "index.mara.md"));
  fs.symlinkSync(
    path.join(repo, "docs", "index.mara.md"),
    path.join(installed, "docs", "index.mara.md"),
  );
  assert.deepEqual(installedDocsProblems(installed, blobs, repo), [
    "docs/index.mara.md is not a regular file",
  ]);

  // Nor is docs/ itself as a link to the repository's folder.
  fs.rmSync(installed, { recursive: true, force: true });
  fs.mkdirSync(installed);
  fs.symlinkSync(path.join(repo, "docs"), path.join(installed, "docs"));
  assert.deepEqual(installedDocsProblems(installed, blobs, repo), ["docs is not a directory"]);

  fs.rmSync(path.join(installed, "docs"));
  assert.deepEqual(installedDocsProblems(installed, blobs, repo), ["no docs/"]);
});
