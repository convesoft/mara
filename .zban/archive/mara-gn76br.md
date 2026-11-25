---
title: The dispatcher package carries the docs folder
status: done
type: feature
parent: mara-xzfeyj
blocked_by:
  - mara-c199tz
  - mara-23j2h9
created_at: 2026-11-25T10:10:46Z
closed_at: 2026-11-25T12:20:48Z
---

Goal: from `0.4.0-rc.2`, `@convesoft/mara` carries this repository's `docs/` as it is, so a developer or a coding agent in a repository that installed the package reads the documentation of the installed version at `node_modules/@convesoft/mara/docs/`.

Decided by the maintainer on 2026-11-25 ("add a zban task to bundle docs in rc2"). The pattern is nx-rust, commit `773c382` (`ADR-BUNDLED-DOCS` there; the checkout `/home/araketski/Work/github.com/convesoft/nx-rust` is read-only): a departure from it is a question for the maintainer.

Context: `docs/distribution.mara.md` (DES-PACKAGES: the dispatcher's contents, and "Each packed file list is checked by name and mode"), `packages/mara-npm/scripts/packages.mjs` (`dispatcherFiles`), `pack.mjs`, `verify-install.mjs`, `packages/mara-npm/project.json`, `README.md`.

- [x] A Decision item in `docs/decisions.mara.md` records the choice and the rejected alternatives, and DES-PACKAGES names `docs/` in the dispatcher's contents from `0.4.0-rc.2`
- [x] `pack.mjs` packs every file under `docs/` of the packed commit into the dispatcher at the same path, mode 0644
- [x] The file check before publication and `verify-install.mjs` compare the tarball's `docs/` with the repository's: no file missing, none extra, none changed
- [x] The Decision item says what a reader of the package finds for the links that leave `docs/`: `../README.md` in `docs/migration.mara.md` resolves in the package, `../scripts/build-souffle.sh` in `docs/distribution.mara.md` does not
- [x] `README.md` names the path of the installed version's documentation
- [x] The pull request gives the packed dispatcher's size before and after, and one dry dispatch of the release workflow on the pushed branch passes

Limits:

- `.mara/` (configuration, schema, rules) is not packed: the packed `docs/` is read as files and is no Mara project. Packing it is a question for the maintainer.
- The native packages do not change.
- A dry dispatch refuses a version that npm already has: this issue starts after the workspace version is `0.4.0-rc.2`.

## Summary of Changes

- `packages/mara-npm/scripts/packages.mjs`: `trackedDocs` reads the files that Git tracks under `docs/` at HEAD (`git ls-tree -r -z`) with their blob IDs, and fails on a tracked entry that is not a regular file (a symbolic link, mode 120000), on a name with a line break, on a commit without `docs/`, and when `git status --porcelain --untracked-files=all --ignored=matching -- docs` lists anything. `dispatcherContents` adds those paths, mode 0644, to the dispatcher's five files. `blobContents` reads the pages from Git's objects with one `git cat-file --batch`. `tarballProblems` takes the blobs and, besides the name and mode check, extracts the tarball into a new temporary directory and compares each `docs/` file's `git hash-object --no-filters` ID with the commit's. `installedDocsProblems` walks an installed `docs/` without following links and compares it the same way.
- `pack.mjs` packs `docs/` from Git's objects and checks the tarball against those paths, modes and blobs before it renames it into `dist/npm`.
- `verify-install.mjs` has a new case, "the installed dispatcher's docs/ equals the checkout's", in both modes (the Build jobs and the Registry check have a checkout of the recorded commit), and the dispatcher's file-list case now covers `docs/` by name, mode and contents.
- `packages/mara-npm/tests/docs.test.mjs`: 12 tests in `mara-npm:test` on a temporary repository: the tracked set and blobs; contents from Git, not the working tree; refusal of a modified, staged, deleted, untracked or ignored file and of an untracked link; refusal of a tracked symbolic link and of a commit without `docs/`; the tarball check on a changed, missing, extra, executable or linked page; the installed check on a changed, missing, extra or linked page, a linked `docs/` and none.
- ADR-BUNDLED-DOCS (new, `docs/decisions.mara.md`) justifies DES-PACKAGES; DES-PACKAGES names `docs/` in the dispatcher from 0.4.0-rc.2 and its comparison as a set; VER-INSTALL-WITHOUT-TOOLCHAIN has the new case and the extended file-list row. `README.md` names `node_modules/@convesoft/mara/docs/`.
- Size of the packed dispatcher (`npm pack`): before 5 files, 8.1 kB packed, 22.3 kB unpacked; after 36 files, 170.9 kB packed, 539.9 kB unpacked.
- Links that leave `docs/`: `../README.md#install-the-prerelease` in `docs/migration.mara.md` resolves to the package's `README.md`; `../scripts/build-souffle.sh` in `docs/distribution.mara.md` resolves to no file of the package; `../needs.mara.md` in `docs/format.mara.md` is inside a fenced example and is no link. The README's links into `docs/` now resolve in the package; its `AGENTS.md` link does not, as before.

Checks: `pnpm run checks` with a fresh Nx cache: 971 passed, 0 failed, 16 skipped (workspace: nextest 843 passed and 14 skipped, `mara-npm:test` 96 passed; performance 32 passed and 2 skipped). `pack-dispatcher` and `pack-native:release`, then `verify-install` locally: every case passed but the two `GLIBC_` analyses, which fail on this machine's glibc 2.39 build as VER-INSTALL-WITHOUT-TOOLCHAIN says; the docs cases passed. Negative probes: `pack.mjs` refuses a modified and an untracked page; `verify-install --tarballs` on a tarball with a changed and an extra page fails both docs cases, naming them. An install with `npm install --ignore-scripts --offline` into an empty directory gives a `docs/` that `diff -r` finds identical to `git archive HEAD docs`, 31 regular files, mode 644. `project validate`: 0 errors, 0 warnings. The dry dispatch runs on the pushed branch; its run is named in the pull request.

## Decisions made

- The packed pages are the files that Git tracks under `docs/` at HEAD, read from Git's objects; packaging fails when the working tree's `docs/` differs from HEAD, an ignored file included, or a tracked entry is not a regular file (ADR-BUNDLED-DOCS). A tracked page with mode 100755 is packed as 0644.
- The install verification compares the installed `docs/` with the checkout's HEAD in both modes, so `verify-install --registry=<version>` must run from a checkout of that version's commit, as the Registry check does.
- The dispatcher's manifest lists each page in `files`, as it lists the other files.
