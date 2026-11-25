---
title: The Node addon, Node library and browser assets are packed, verified and in the release
status: todo
type: feature
parent: mara-xzfeyj
blocked_by:
  - mara-vfkgbs
  - mara-xsm8w6
  - mara-23j2h9
  - mara-gn76br
  - mara-c199tz
created_at: 2026-11-25T10:18:52Z
---

Goal: `0.4.0-rc.2` publishes what the slice built in the existing three packages (maintainer decision on mara-2c6hfk, 2026-11-25: single main package): each Linux native package holds the Node addon (`bin/mara.node`), and `@convesoft/mara` holds all platform-independent parts — the CLI dispatcher, the Node library at `.`, the browser workspace + `connectDaemon` + engine and Soufflé WASM modules at `./browser`, `docs/`, and licences.

Context: `DES-PACKAGES`, `DES-RELEASE-WORKFLOW`, `ADR-SCRIPT-FREE-NPM-PACKAGES` (docs/distribution.mara.md, docs/decisions.mara.md), `VER-INSTALL-WITHOUT-TOOLCHAIN` (docs/verification.mara.md), `ADR-NODE-NATIVE-ADDON`, `REQ-NODE-LIBRARY` (docs/hosts/index.mara.md). The three items come from mara-a4m588, which keeps what 0.4.0 adds after them. `packages/mara-npm/` holds the packer, the file checks and the install verification; mara-23j2h9 made the release's package table the one place a new package is added, and mara-gn76br put `docs/` into the dispatcher. The names are the maintainer's: mara-2c6hfk. What a package contains is a high-risk area: binaries, WASM modules and licences.

- [ ] Each Linux native package holds the Node addon built in the release's Build job on the same commit as `bin/mara`; `DES-PACKAGES` names the file, and the `GLIBC_` 2.35 floor of the install verification covers it
- [ ] `@convesoft/mara` exports the typed ESM Node library at `.` (`import … from "@convesoft/mara"`) and the browser workspace + `connectDaemon` at `./browser` (`import … from "@convesoft/mara/browser"`), with no lifecycle script and no runtime dependency fetched at install
- [ ] `@convesoft/mara` holds the engine's and Soufflé's WASM modules, the browser client and workers, the Node library, `docs/`, `LICENSE-SOUFFLE`, and `THIRD-PARTY-LICENSES` for `mara.wasm` (licence `(MIT OR Apache-2.0) AND UPL-1.0`); `DES-PACKAGES` drops the separate browser-package row and lists the exact files of `@convesoft/mara`, and the packed file list is checked by name and mode as the others are
- [ ] The Emscripten build of Soufflé and the `wasm32` build of `mara-wasm` run where the release packs `@convesoft/mara` (`Pack dispatcher`), pinned as `DES-SOUFFLE-BUILD` says; nothing published comes from a cache or from another commit
- [ ] The install verification installs the packed files with `--ignore-scripts` in an empty directory, imports `@convesoft/mara` and runs one operation embedded through the addon, and imports `@convesoft/mara/browser` in Chromium and runs one operation in a browser workspace
- [ ] No separate browser package is added to the release's package table: Dry run, Publish, Registry check and Tag run with the same three packages as `0.4.0-rc.1`
- [ ] One dry dispatch of the release workflow on the pushed branch passes with every package of rc.2

Limits:

- macOS arm64 stays in mara-ag4ees.
- Nothing is published: the publication of rc.2 is the maintainer's.

Context from mara-vfkgbs (DES-NODE-LIBRARY, ADR-NAPI-RS-ADDON in docs/hosts/index.mara.md):

- The library is `packages/mara-npm/lib/` (`index.js`, `index.d.ts`, `operations.d.ts`). `libraryFiles` and `libraryManifest` in `packages/mara-npm/scripts/packages.mjs` name its files and the dispatcher manifest's `type` and `exports`; `packDispatcher` does not pack them yet. The host tests install them with `crates/mara/tests/hosts/node/support/library.mjs`.
- The addon is the `cdylib` of the crate `mara-node`, `target/<profile>/libmara_node.so` from `nx run mara-node:build`; the library loads it as `bin/mara.node` of the native package and runs the `mara` beside it as its daemon.
- The browser package gained `daemon.js` and `errors.js` in its `files`, and `operations.d.ts`, which `mara-browser:build` copies from the library (gitignored, like `mara.wasm`).
