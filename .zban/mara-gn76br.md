---
title: The dispatcher package carries the docs folder
status: in-progress
type: feature
parent: mara-xzfeyj
blocked_by:
  - mara-c199tz
  - mara-23j2h9
created_at: 2026-11-25T10:10:46Z
---

Goal: from `0.4.0-rc.2`, `@convesoft/mara` carries this repository's `docs/` as it is, so a developer or a coding agent in a repository that installed the package reads the documentation of the installed version at `node_modules/@convesoft/mara/docs/`.

Decided by the maintainer on 2026-11-25 ("add a zban task to bundle docs in rc2"). The pattern is nx-rust, commit `773c382` (`ADR-BUNDLED-DOCS` there; the checkout `/home/araketski/Work/github.com/convesoft/nx-rust` is read-only): a departure from it is a question for the maintainer.

Context: `docs/distribution.mara.md` (DES-PACKAGES: the dispatcher's contents, and "Each packed file list is checked by name and mode"), `packages/mara-npm/scripts/packages.mjs` (`dispatcherFiles`), `pack.mjs`, `verify-install.mjs`, `packages/mara-npm/project.json`, `README.md`.

- [ ] A Decision item in `docs/decisions.mara.md` records the choice and the rejected alternatives, and DES-PACKAGES names `docs/` in the dispatcher's contents from `0.4.0-rc.2`
- [ ] `pack.mjs` packs every file under `docs/` of the packed commit into the dispatcher at the same path, mode 0644
- [ ] The file check before publication and `verify-install.mjs` compare the tarball's `docs/` with the repository's: no file missing, none extra, none changed
- [ ] The Decision item says what a reader of the package finds for the links that leave `docs/`: `../README.md` in `docs/migration.mara.md` resolves in the package, `../scripts/build-souffle.sh` in `docs/distribution.mara.md` does not
- [ ] `README.md` names the path of the installed version's documentation
- [ ] The pull request gives the packed dispatcher's size before and after, and one dry dispatch of the release workflow on the pushed branch passes

Limits:

- `.mara/` (configuration, schema, rules) is not packed: the packed `docs/` is read as files and is no Mara project. Packing it is a question for the maintainer.
- The native packages do not change.
- A dry dispatch refuses a version that npm already has: this issue starts after the workspace version is `0.4.0-rc.2`.
