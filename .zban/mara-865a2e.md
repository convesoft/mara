---
title: The Node addon and the browser package are packed, verified and in the release
status: todo
type: feature
parent: mara-xzfeyj
blocked_by:
  - mara-vfkgbs
  - mara-xsm8w6
  - mara-23j2h9
  - mara-gn76br
  - mara-c199tz
  - mara-2c6hfk
created_at: 2026-11-25T10:18:52Z
---

Goal: `0.4.0-rc.2` publishes what the slice built: each Linux native package holds the Node addon, the library is importable from the installed packages, and the browser package with the engine and Soufflé WASM modules is a package of the release.

Context: `DES-PACKAGES`, `DES-RELEASE-WORKFLOW`, `ADR-SCRIPT-FREE-NPM-PACKAGES` (docs/distribution.mara.md, docs/decisions.mara.md), `VER-INSTALL-WITHOUT-TOOLCHAIN` (docs/verification.mara.md), `ADR-NODE-NATIVE-ADDON`, `REQ-NODE-LIBRARY` (docs/hosts/index.mara.md). The three items come from mara-a4m588, which keeps what 0.4.0 adds after them. `packages/mara-npm/` holds the packer, the file checks and the install verification; mara-23j2h9 made the release's package table the one place a new package is added, and mara-gn76br put `docs/` into the dispatcher. The names are the maintainer's: mara-2c6hfk. What a package contains is a high-risk area: binaries, WASM modules and licences.

- [ ] Each Linux native package holds the Node addon built in the release's Build job on the same commit as `bin/mara`; `DES-PACKAGES` names the file, and the `GLIBC_` 2.35 floor of the install verification covers it
- [ ] The typed ESM client is importable as mara-2c6hfk decides, with no lifecycle script and no dependency fetched at install
- [ ] The browser package holds the engine's and Soufflé's WASM modules, the worker and the client, its manifest and licences (Soufflé's included), under the name that mara-2c6hfk decides; `DES-PACKAGES` lists its files, and the packed file list is checked by name and mode as the others are
- [ ] The Emscripten build of Soufflé runs where the release packs the browser package, pinned as `DES-SOUFFLE-BUILD` says; nothing published comes from a cache or from another commit
- [ ] The install verification installs the packed files with `--ignore-scripts` in an empty directory, loads the Node addon through the library and runs one operation embedded, and loads the browser package in Chromium and runs one operation in a workspace
- [ ] The browser package is one row of the release's package table: Dry run, Publish, Registry check and Tag take it with no step of its own
- [ ] One dry dispatch of the release workflow on the pushed branch passes with every package of rc.2

Limits:

- macOS arm64 stays in mara-ag4ees.
- Nothing is published: the publication of rc.2 is the maintainer's.

Context from mara-vfkgbs (DES-NODE-LIBRARY, ADR-NAPI-RS-ADDON in docs/hosts/index.mara.md):

- The library is `packages/mara-npm/lib/` (`index.js`, `index.d.ts`, `operations.d.ts`). `libraryFiles` and `libraryManifest` in `packages/mara-npm/scripts/packages.mjs` name its files and the dispatcher manifest's `type` and `exports`; `packDispatcher` does not pack them yet. The host tests install them with `crates/mara/tests/hosts/node/support/library.mjs`.
- The addon is the `cdylib` of the crate `mara-node`, `target/<profile>/libmara_node.so` from `nx run mara-node:build`; the library loads it as `bin/mara.node` of the native package and runs the `mara` beside it as its daemon.
- The browser package gained `daemon.js` and `errors.js` in its `files`, and `operations.d.ts`, which `mara-browser:build` copies from the library (gitignored, like `mara.wasm`).
