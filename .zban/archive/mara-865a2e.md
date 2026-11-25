---
title: The Node addon, Node library and browser assets are packed, verified and in the release
status: done
type: feature
parent: mara-xzfeyj
blocked_by:
  - mara-vfkgbs
  - mara-xsm8w6
  - mara-23j2h9
  - mara-gn76br
  - mara-c199tz
created_at: 2026-11-25T10:18:52Z
closed_at: 2026-11-25T17:16:10Z
---

Goal: `0.4.0-rc.2` publishes what the slice built in the existing three packages (maintainer decision on mara-2c6hfk, 2026-11-25: single main package): each Linux native package holds the Node addon (`bin/mara.node`), and `@convesoft/mara` holds all platform-independent parts — the CLI dispatcher, the Node library at `.`, the browser workspace + `connectDaemon` + engine and Soufflé WASM modules at `./browser`, `docs/`, and licences.

Context: `DES-PACKAGES`, `DES-RELEASE-WORKFLOW`, `ADR-SCRIPT-FREE-NPM-PACKAGES` (docs/distribution.mara.md, docs/decisions.mara.md), `VER-INSTALL-WITHOUT-TOOLCHAIN` (docs/verification.mara.md), `ADR-NODE-NATIVE-ADDON`, `REQ-NODE-LIBRARY` (docs/hosts/index.mara.md). The three items come from mara-a4m588, which keeps what 0.4.0 adds after them. `packages/mara-npm/` holds the packer, the file checks and the install verification; mara-23j2h9 made the release's package table the one place a new package is added, and mara-gn76br put `docs/` into the dispatcher. The names are the maintainer's: mara-2c6hfk. What a package contains is a high-risk area: binaries, WASM modules and licences.

- [x] Each Linux native package holds the Node addon built in the release's Build job on the same commit as `bin/mara`; `DES-PACKAGES` names the file, and the `GLIBC_` 2.35 floor of the install verification covers it
- [x] `@convesoft/mara` exports the typed ESM Node library at `.` (`import … from "@convesoft/mara"`) and the browser workspace + `connectDaemon` at `./browser` (`import … from "@convesoft/mara/browser"`), with no lifecycle script and no runtime dependency fetched at install
- [x] `@convesoft/mara` holds the engine's and Soufflé's WASM modules, the browser client and workers, the Node library, `docs/`, `LICENSE-SOUFFLE`, and `THIRD-PARTY-LICENSES` for `mara.wasm` (licence `(MIT OR Apache-2.0) AND UPL-1.0`); `DES-PACKAGES` drops the separate browser-package row and lists the exact files of `@convesoft/mara`, and the packed file list is checked by name and mode as the others are
- [x] The Emscripten build of Soufflé and the `wasm32` build of `mara-wasm` run where the release packs `@convesoft/mara` (`Pack dispatcher`), pinned as `DES-SOUFFLE-BUILD` says; nothing published comes from a cache or from another commit
- [x] The install verification installs the packed files with `--ignore-scripts` in an empty directory, imports `@convesoft/mara` and runs one operation embedded through the addon, and imports `@convesoft/mara/browser` in Chromium and runs one operation in a browser workspace
- [x] No separate browser package is added to the release's package table: Dry run, Publish, Registry check and Tag run with the same three packages as `0.4.0-rc.1`
- [x] One dry dispatch of the release workflow on the pushed branch passes with every package of rc.2

Limits:

- macOS arm64 stays in mara-ag4ees.
- Nothing is published: the publication of rc.2 is the maintainer's.

Context from mara-vfkgbs (DES-NODE-LIBRARY, ADR-NAPI-RS-ADDON in docs/hosts/index.mara.md):

- The library is `packages/mara-npm/lib/` (`index.js`, `index.d.ts`, `operations.d.ts`). `libraryFiles` and `libraryManifest` in `packages/mara-npm/scripts/packages.mjs` name its files and the dispatcher manifest's `type` and `exports`; `packDispatcher` does not pack them yet. The host tests install them with `crates/mara/tests/hosts/node/support/library.mjs`.
- The addon is the `cdylib` of the crate `mara-node`, `target/<profile>/libmara_node.so` from `nx run mara-node:build`; the library loads it as `bin/mara.node` of the native package and runs the `mara` beside it as its daemon.
- The browser package gained `daemon.js` and `errors.js` in its `files`, and `operations.d.ts`, which `mara-browser:build` copies from the library (gitignored, like `mara.wasm`).

## Summary of Changes

- `@convesoft/mara` now packs the Node library (`lib/`), the browser workspace and daemon client with the engine's and Soufflé's WebAssembly modules (`browser/`), `LICENSE-SOUFFLE` and `THIRD-PARTY-LICENSES` for `mara.wasm` (48 crates, `wasm32-unknown-unknown`), beside the CLI dispatcher, README, licences and `docs/`; its manifest has `type: module`, `exports` `.` and `./browser`, licence `(MIT OR Apache-2.0) AND UPL-1.0`, no scripts and no dependencies. The packer checks that `mara.wasm` exports the worker's functions.
- Each Linux native package packs `bin/mara.node` (mode 0644), the release `libmara_node.so` built by `mara-node:build:release` beside `mara:build:release`; `THIRD-PARTY-LICENSES` covers `mara` and `mara-node` (198 crates on x86_64). napi-rs's crates ship no licence file: `packages/mara-npm/licenses/napi-rs-LICENSE` keeps the repository's.
- `pack-dispatcher` depends on `mara-wasm:build` and `cargo-workspace:souffle-wasm` and reads both modules from where they were built; Pack dispatcher runs on `ubuntu-22.04` with the Emscripten SDK, now the composite action `.github/actions/emscripten` shared with CI.
- The install verification gains a Node-library case (import by name, `project_validate` with `noDaemon`, the result equals `--format json project validate`, a program run counted, `bin/mara.node` mapped in the process) and a Chromium case (import `@convesoft/mara/browser` through an import map to the installed manifest's export, workspace `project_validate` equals direct mode, a program run counted, `mara.wasm` and `souffle.wasm` served from the installed package); its `GLIBC_` analysis covers `bin/mara.node`. Build and Registry check install Chromium before it. Helpers in `scripts/verify-hosts.mjs`.
- `packages/mara-browser` is the private source folder `mara-browser`; the host tests take the browser file list from `packages.mjs`.
- Docs: `ADR-ONE-MAIN-PACKAGE` (new), `DES-PACKAGES`, `DES-SOUFFLE-BUILD`, `DES-CI-WORKFLOW`, `DES-RELEASE-WORKFLOW`, `VER-INSTALL-WITHOUT-TOOLCHAIN`, `DES-NODE-LIBRARY`, `DES-BROWSER-WORKSPACE`, `ADR-NAPI-RS-ADDON`, `ADR-NX-RUST-TASKS`, `DES-RULE-EXECUTION`, README.

Checks:
- `pnpm run checks` under `berth --exclusive` with a fresh `NX_CACHE_DIRECTORY`: exit 0; workspace run 859 passed, 0 failed, 14 skipped (229 + 630); performance run 32 passed, 2 skipped; `mara-npm:test` 114 passed; `project validate` 0 errors, 0 warnings. Run before the last fix, which changes only `cargo metadata`'s `--offline` in the licence inventory; `mara-npm:test` and oxlint passed again after it.
- Local `verify-install` on packed x64 tarballs: every case passes but the three `GLIBC_` analyses, which fail on this machine's newer glibc as VER-INSTALL-WITHOUT-TOOLCHAIN says. Probes: without `bin/mara.node` the library case fails; with a truncated `souffle.wasm` the browser case fails.
- Release dry dispatch https://github.com/convesoft/mara/actions/runs/45660562566 on c2e310b: Prepare, Pack dispatcher (3 min 21 s, Soufflé's module built in 2 min 31 s), Build ubuntu-22.04 and ubuntu-22.04-arm (every install-verification case passed, the addon, Chromium and the `GLIBC_` 2.35 floor of `bin/mara.node` included), Dry run passed; Publish, Registry check and Tag skipped. Packages: `@convesoft/mara` 52 files, 2.8 MB packed, 11.4 MB unpacked; linux-x64-gnu 8 files, 23.6 MB / 78.8 MB; linux-arm64-gnu 8 files, 22.6 MB / 68.4 MB. The first dispatch (45659273508) failed in Pack dispatcher: `cargo metadata --offline` lacked `mara-node`'s crates there; fixed by fetching them at `Cargo.lock`'s versions.

## Decisions made

- `bin/mara.node` has mode 0644 (ADR-ONE-MAIN-PACKAGE).
- Pack dispatcher runs on `ubuntu-22.04`, where CI builds Soufflé's module (ADR-ONE-MAIN-PACKAGE).
- napi-rs's `LICENSE`, which its crates do not ship, is kept in `packages/mara-npm/licenses/` and named in the licence table (ADR-ONE-MAIN-PACKAGE, DES-PACKAGES).
- `pack-dispatcher` reads `mara.wasm` from Cargo's artifact directory and Soufflé's module from `target/souffle-2.5-wasm`, as `pack-native` reads its builds, not the copies in `packages/mara-browser` (DES-RELEASE-WORKFLOW).
- The Emscripten SDK step is the composite action `.github/actions/emscripten`, shared by CI and Pack dispatcher, so one SHA-256 pin (DES-CI-WORKFLOW).
- The licence inventory's `cargo metadata` runs without `--offline`, with `--locked` (DES-PACKAGES).
- `packages/mara-browser/package.json` is the private source folder `mara-browser`, without `files` or `exports`; the browser file list lives in `packages.mjs`.

## Fix round 1

- `pack.mjs dispatcher` refuses a Cargo target directory other than `target/` (mara-6z7bjs), where Nx restores `mara-wasm:build`'s module, as `copy-module.mjs` does; a test checks that it writes no tarball (DES-RELEASE-WORKFLOW).
- The Node-library case reads each `/proc/self/maps` pathname after its five fixed columns, so spaces in it are kept (`mappedPath` in `verify-hosts.mjs`, `tests/verify-hosts.test.mjs`); a run with two spaces in `TMPDIR` passes the case.
- Docs: DES-PACKAGES records the licence inventory's `cargo metadata --locked` without `--offline` and `browserFiles` as the browser file list; ADR-ONE-MAIN-PACKAGE says `0.4.0-rc.2` publishes the three packages of `0.4.0-rc.1`; VER-INSTALL-WITHOUT-TOOLCHAIN names each case's entry point.
- `pnpm run checks` (`berth --exclusive`, fresh `NX_CACHE_DIRECTORY`) on c36590a: exit 0; workspace run 859 passed, 0 failed, 14 skipped (229 + 630); performance run 32 passed, 2 skipped; `mara-npm:test` 118 passed; `project validate` 0 errors, 0 warnings. Local `verify-install` on x64 tarballs: every case but the three `GLIBC_` analyses of this machine's newer glibc.
