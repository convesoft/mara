---
title: Review the decisions workers made in alpha.6
status: todo
type: task
tags:
  - maintainer
blocked_by:
  - mara-xzfeyj
created_at: 2026-11-14T11:14:21Z
---

Goal: the maintainer reviews what workers decided on their own in the alpha.6 slice.

Context: AGENTS.md (Deciding during the rewrite). When the alpha.6 epic (mara-xzfeyj) closes, the orchestrator appends the `## Decisions made` list of each of its issues below. This issue blocks the release workflow, not the next slice. No worker starts it.

- [ ] Every listed decision accepted, or overturned by a follow-up issue

- 2026-11-16: mara-274e1m (the Verification items for alpha.6) was started before the alpha.5 epic closed, since it writes documents only. Undo: not needed.

- 2026-11-25, provisional (question mara-2c6hfk): the browser package is built as `@convesoft/mara-browser`, private and named in its manifest only, and the Node library is imported from `@convesoft/mara`; nothing is packed or published under either before the maintainer answers. Rests on `DES-PACKAGES`, `REQ-NODE-LIBRARY`. Undo: rename the package in its manifest; move the client's files.
- 2026-11-25: mara-gn76br (`docs/` in the dispatcher) starts after mara-23j2h9, because both change the packer's file lists and the file check before publication; the maintainer asked only that it start after the version is `0.4.0-rc.2`. Undo: remove the edge.

- 2026-11-25, mara-fkkcad: the origin check of `daemon start --http` parses no host. It checks a fixed list (`DES-BROWSER-LISTENER`): the scheme, no path or wildcard characters, no upper case, a port that is not the default, a bracketed host through the standard library's IPv6 parser. The first version re-implemented a browser's URL parsing and was wrong both ways (review, P2). Undo: replace the list by a URL library's serialization.
- 2026-11-25, mara-fkkcad: `VER-BROWSER-ACCESS` expected `not-an-input` for a write that leaves the project through `..`; CLI direct mode answers `invalid-argument` (`DES-OPERATIONS`), so the Accepted item changed to that. The requirement holds: the request is refused and the other project is untouched. Undo: change direct mode and the item together.
- 2026-11-25, mara-fkkcad: the macOS `lsof` case of `VER-BROWSER-ACCESS` is not built; the item marks it as running from mara-ag4ees, which has a task for it (the maintainer's decision of 2026-11-14 defers everything for macOS).
- 2026-11-25, mara-jptfbq, after a second opinion (Gemini): the browser runs the `mara` library's own host code as WebAssembly over an in-memory filesystem, one per module instance (`ADR-MEMORY-FILESYSTEM`), not a second host around the engine crate. Rests on `REQ-SAME-RESULT-ON-EVERY-HOST` and the reason of `ADR-NODE-NATIVE-ADDON`. Undo: a filesystem handle passed through the host functions.
- 2026-11-25, provisional (question mara-aedd1d): the browser module has plain exports and no `wasm-bindgen`.
- 2026-11-25, mara-jptfbq: a read of a file's contents counts as a file read in the browser workspace as on every host; the worker's exemption for the memory filesystem was removed, so that the bound of `VER-WORK-COUNTERS` means something there. Undo: restore the sentence in `DES-BROWSER-WORKSPACE`.
- 2026-11-25, mara-jptfbq: with Cargo's target directory redirected, the browser package's build takes the module from that directory or fails; it never copies a module the build did not write. Same limit as mara-6z7bjs.
- 2026-11-25, provisional (question mara-w8zr8m): the release steps are scripts with tests (mara-23j2h9), where zban and nx-rust keep inline shell.
- 2026-11-25, mara-23j2h9: `ADR-DISPATCHED-RELEASE`'s line on npm's dry-run refusal was made precise on evidence (npm 12.2.1 refuses a dry run only of a published version without a prerelease part); the decision is unchanged.
- 2026-11-25, mara-xh6mp6: the daemon tests and `warm_scaling.rs` keep their second kill pass; only the code tests' `Daemon::stop` waits once, through a consuming `kill_once`.

- 2026-11-25, mara-2c6hfk (settled by maintainer): single main package `@convesoft/mara` carries all platform-independent parts (CLI dispatcher, Node library at `.`, browser workspace + `connectDaemon` + WASM modules at `./browser`, `docs/`, licences); no separate `@convesoft/mara-browser` package. Supersedes the earlier provisional mara-2c6hfk line.
- 2026-11-25, mara-jptfbq: the file-read line above is withdrawn — `VER-WORK-COUNTERS`'s Accepted preamble already states contents the caller supplies or writes are no file reads (`DES-BROWSER-WORKSPACE` keeps that exemption).
- 2026-11-25, mara-jptfbq: non-blocking P3 point 10 dropped (`DES-BROWSER-WORKSPACE` already classifies unsendable arguments as `invalid-argument`).
- 2026-11-25, mara-6s982p: removed the speed-dependent debug-only 2-read middle case from `code_add_counts`; test checks settled (1/1) and unsettled (3/3), and `VER-CODE-WORK-COUNTERS` notes 2 as an untested consequence.
- 2026-11-25, mara-gn76br: `docs/` packed from Git `HEAD` objects under manifest folder entry `docs` (matching nx-rust `773c382`), compared by blob set in `pack.mjs` and `verify-install.mjs` (Registry check included) with strict C-locale tar escape decoding; no `PLUGIN.md` (mara is not an Nx plugin).
- 2026-11-25, provisional, mara-xsm8w6: moved the Emscripten build of Soufflé (locally and in CI, cached like native Soufflé) out of deferred mara-4dr2p2 into mara-xsm8w6 (rests on `DES-SOUFFLE-BUILD` "by 0.4.0-rc.2" and `VER-HOST-COMPARISON\").
- 2026-11-25, mara-xsm8w6: kept per-instance `FS.write` interception (`packages/mara-browser/output-limit.js`) for the browser output-size limit on pinned Emscripten SDK 6.0.12 (no `trackingDelegate` in 6.0.12; MEMFS has no `RLIMIT_FSIZE`), hardened with post-run `FS.stat` check and a 16-byte smoke case in `scripts/souffle-wasm-smoke.mjs`.
- 2026-11-25, provisional (question mara-ecnpg6), mara-xsm8w6: adopted Option A on `VER-EXPLANATIONS` record numbering (`record #3` native vs `record #2` WASM in Soufflé 2.5 provenance); undoable by stripping `#N` in `rules/explain.rs`.
- 2026-11-25, mara-xsm8w6: amended `VER-WORK-COUNTERS` browser warm-view row on evidence (`readiness.dl` has no invariant, so validation compiled against empty facts; `query view` runs once then 0, matching the native daemon test).
- 2026-11-25, provisional (question mara-5xvztr), mara-vfkgbs: `mara-node` uses nx-rust's plain `build` (`artifacts = []`) instead of `napi-build` (`ADR-NAPI-RS-ADDON`).

- 2026-11-25, mara-vfkgbs: embedded Node calls never call `libc::umask`; direct calls skip umask lookup and daemon calls read `/proc/self/status` read-only, falling back to in-process execution when unavailable (`DES-NODE-LIBRARY`, `DES-DAEMON-LIFECYCLE`).
- 2026-11-25, mara-865a2e: `pack.mjs dispatcher` refuses a redirected `CARGO_TARGET_DIR` other than `target/` (`mara-6z7bjs`), matching `packages/mara-browser/copy-module.mjs`; `packages/mara-npm/licenses/napi-rs-LICENSE` vendored for napi-rs crates; `cargo metadata --locked` runs without `--offline`.
- 2026-11-25, provisional (questions mara-h5fde2, mara-jv0311): `0.4.0-rc.2` ships with Rust + Soufflé licence notices only (C/C++ runtime notices in mara-h5fde2) and unstripped release `mara`/`mara.node` (stripping in mara-jv0311).
- 2026-11-25, audit at `4cbcf26`: filed 3 bugs (mara-w7amcb, mara-8zm9hk, mara-332wqc) + arm64 timing flake (mara-zfn4jg) under `mara-qzp1gd`, 9 backlog audit items (mara-dp5tn5, mara-mmr6mp, mara-rdx1w9, mara-5nnswe, mara-yqvt83, mara-rm6gye, mara-yyvqz7, mara-0np0gc, mara-2ctbmx), and notes on `mara-fwy8zs`, `mara-dwytrs`, `mara-5d6pcs`.

### Worker `## Decisions made` lists from closed children of mara-xzfeyj

- **mara-fkkcad:** `DES-BROWSER-PROTOCOL` v1 wire format (`POST /operation`, refusals 400/401/403/404/503, 204 preflight); `project` and `input` options refused over HTTP (inputs travel in `supplied`); `--allow-project` resolved from cwd; fixed origin-check list in `DES-BROWSER-LISTENER` (no URL host parsing); `ADR-HTTPARSE-LISTENER` (`httparse` on `std::net`); 100 ms stop close & 10 s per-read/write timeout; `VER-BROWSER-ACCESS` `../<B>` expects `invalid-argument`; macOS `lsof` deferred to `mara-ag4ees`.
- **mara-jptfbq:** `ADR-MEMORY-FILESYSTEM` (host library on wasm32 over per-instance memory FS); `ADR-RAW-WASM-MODULE` (plain exports, provisional `mara-aedd1d`); `DES-BROWSER-WORKSPACE` API, refusals, logical clock, empty dirs dropped with last file, memory reads count 0 file reads (`VER-WORK-COUNTERS`); `mara-wasm:build` release default + `mara-wasm:lint:wasm32` (`mara-apb968`); Chromium installed in `ci.yml` without headless shell and keyed by path/size/mtime; `mara-browser:build` refuses redirected `CARGO_TARGET_DIR` (`mara-6z7bjs`).
- **mara-xsm8w6:** `ADR-BROWSER-EVALUATION-PASSES` (hand-off by passes, explanations by rounds with timeout fallback to prior round); browser program run counts 0 subprocess spawns; Emscripten SDK 6.0.12 (`-sEXIT_RUNTIME`, 1 GiB `-sABORTING_MALLOC`, 8 MiB stack overflow check, no C++ exceptions); Node-only smoke check on cache hit; CI installs SDK on cache miss only; `FS.write` output-limit guard in `output-limit.js` + post-run `FS.stat` + 16-byte smoke check; provisional Option A on `mara-ecnpg6` record numbering; `VER-WORK-COUNTERS` warm browser view row corrected on evidence.
- **mara-vfkgbs:** `ADR-NAPI-RS-ADDON` (napi-rs built by plain nx-rust `build` with `artifacts = []`, provisional `mara-5xvztr`); `bin/mara.node` locates sibling `mara` via `dladdr` and freezes `Build` on first use; no signal handlers and no `libc::umask` mutation in embedded host; single `operations.d.ts` copied into browser package; `createClient` / `connectDaemon` parity with required-option prevalidation and value-only `malformed-request` mapping; `mara:test` forwards options to `mara-node:build`; carry-over cleanups for `DES-BROWSER-PROTOCOL`, `VER-BROWSER-ACCESS`, and ancestry validation.
- **mara-xh6mp6:** `kill_once` flag on `Daemons` rather than `std::mem::forget` to avoid leaking owned `PathBuf`s.
- **mara-23j2h9:** Bash scripts in `.github/scripts/` (provisional `mara-w8zr8m`); `--provenance` passed as arg; single package table `releasePackages`/`anyPlatformPackages`; HTTP proxy stub for `gh`; `mara-npm:test` uncached; test-only env vars verified absent from workflow; standalone assignments for checked commands; `ADR-DISPATCHED-RELEASE` npm 12.2.1 dry-run refusal precision.
- **mara-n918j4:** Release project `mara` only; Nx defaults for tag/release title and `strictPreid`; Tag runs `nx release changelog` only for missing tag/release; Dry run passes identical `--git-push-args`.
- **mara-gn76br:** `ADR-BUNDLED-DOCS` packs Git `HEAD` objects under manifest folder `docs` (mode 0644), fails on working-tree differences / non-regular entries / non-UTF-8 / line breaks; strict C-locale tar escape decoding; `verify-install` compares docs in both modes with `--registry` version-mismatch hint.
- **mara-865a2e:** `ADR-ONE-MAIN-PACKAGE` (single main package `@convesoft/mara` per `mara-2c6hfk`); `bin/mara.node` mode 0644; Pack dispatcher on `ubuntu-22.04` sharing `.github/actions/emscripten`; vendored `napi-rs-LICENSE`; `pack-dispatcher` reads built artifacts from `target/` and refuses redirected `CARGO_TARGET_DIR`; `cargo metadata --locked` without `--offline`; `packages/mara-browser` private source folder with `browserFiles` in `packages.mjs`.
- **mara-6s982p:** `code_add_counts` tests deterministic settled (1) and unsettled (3) states; speed-dependent 2-read middle case removed and noted in `VER-CODE-WORK-COUNTERS`.
- **mara-mv8x83:** `GH_TELEMETRY=false` in `packages/mara-npm/tests/support.mjs` (takes precedence over `DO_NOT_TRACK`).
