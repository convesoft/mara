---
title: Browser-local workspace
status: done
type: feature
parent: mara-xzfeyj
created_at: 2026-11-13T22:10:06Z
closed_at: 2026-11-25T11:27:15Z
---

Context: REQ-BROWSER-LOCAL-WORKSPACE and DES-BROWSER-WORKSPACE in docs/hosts/index.mara.md; REQ-UNAVAILABLE-NOT-PARTIAL in docs/engine.mara.md.

- [x] Engine as WASM in a worker over supplied file contents
- [x] In-memory edits exported as a change set with base hashes
- [x] Capabilities the browser lacks are reported as unavailable
- [x] The Node and browser test setup of VER-HOST-COMPARISON (docs/verification.mara.md): `node --test` modules started by a Rust integration test, Playwright's Chromium
- [x] Tests: every case of VER-BROWSER-WORKSPACE (docs/hosts/verification.mara.md) passes but the program part of the caller-write row (mara-xsm8w6); the workspace parts of the first two rows of VER-UNAVAILABLE-NOT-PARTIAL; the browser workspace rows of VER-WORK-COUNTERS (docs/verification.mara.md) marked mara-jptfbq, through the workspace's debug stats option (DES-PERFORMANCE-HARNESS)

## Summary of Changes

- The CLI's operations (`crates/mara/src/cli*`) are part of the `mara` library (`mara::cli`), in the same files, so that another host calls them; MCP's argument checks moved to `cli/tool.rs`, which the browser workspace shares.
- `mara::system` is the host's filesystem, clock, random bytes and process ID. In the `wasm32-unknown-unknown` build the filesystem is the workspace's files in memory (`system/memory.rs`), and `tracked.rs`, `project.rs`, `inputs.rs` and `write.rs` reach files through it. That build leaves out the daemon, the watcher, MCP, `code add`, tree-sitter grammars (`grammar/browser.rs` reports each pinned grammar `unavailable-grammar`: the browser cannot load grammars) and Soufflé's process (each evaluation is a fault without a line until mara-xsm8w6).
- `cli/workspace.rs`: the workspace (create from files, run an operation as its MCP tool, write, remove, export with BLAKE3 `base`), refusing `project init` and `project convert` with `unavailable-operation`, with inputs kept between requests as a daemon's session keeps them.
- The crate `mara-wasm` exports the workspace's calls as plain functions; `packages/mara-browser` (`index.js`, `worker.js`, `index.d.ts`) runs the module in a dedicated worker. Nx: `mara-wasm:build` builds the release module, `mara-browser:build` copies it into the package, `mara:test` depends on it and keys the package, Node, Playwright's version and the browser revision.
- Tests: `crates/mara/tests/hosts` starts `node --test` modules (`node/browser/*.test.mjs`) with fixtures that it writes; they drive Playwright's Chromium (`playwright` 1.64.1, exact devDependency, Chromium revision 1248) from a loopback page server and compare with CLI direct mode on copies. Every case of VER-BROWSER-WORKSPACE but the program part of the caller-write row, the workspace parts of the first two rows of VER-UNAVAILABLE-NOT-PARTIAL, and the two browser warm rows of VER-WORK-COUNTERS marked mara-jptfbq. The corpus generator and the 0.3.0 input moved to `tests/support` to be shared.
- CI installs Chromium's headless shell in `ci.yml`; DES-CI-WORKFLOW, ADR-NX-RUST-TASKS, DES-BROWSER-WORKSPACE and AGENTS.md "Local checks" say so; AGENTS.md lost its obsolete section "Until `main` is fast-forwarded".
- Checks: `pnpm run checks` with a fresh Nx cache after merging `origin/main`: 869 passed, 0 failed, 16 skipped (837 and 14 in the workspace run, 32 and 2 in the performance run), `project validate` 0 errors and 0 warnings. Probes: each declared input of `mara:test` (package file, `PLAYWRIGHT_BROWSERS_PATH`, Node on `PATH`, Playwright's version, the module through `mara-wasm:build` and `mara-browser:build`) reran the task when changed and restored it when reverted; a deleted module is restored from the cache; the tests fail with the workspace's inputs not kept or with a wrong `base`, and the setup fails on a leftover process that names its directory.

## Decisions made

- The browser runs the `mara` library's own host code over a filesystem in the module's memory (ADR-MEMORY-FILESYSTEM).
- The module exports plain functions with frames of a JSON header and file bytes, built by Cargo alone, no `wasm-bindgen` (ADR-RAW-WASM-MODULE).
- The JavaScript API, its refusals (`invalid-argument` for malformed or conflicting paths, a removal where no file is, an unknown operation, a `project` argument; `unavailable-operation` before arguments are read), the memory's logical clock, and that reading supplied or written contents counts no file read (DES-BROWSER-WORKSPACE).
- `mara-wasm:build` builds the release module by default and adds `mara_wasm.wasm` to the plugin's outputs; `mara:test` depends on `mara-browser:build` (ADR-NX-RUST-TASKS).
- The grammar fixture of the unavailable cases is `empty` with the code files and pins of VER-GRAMMAR-MARKERS and no program, since programs need Soufflé's module (mara-xsm8w6).
- Chromium is installed in `ci.yml`, not in the setup action that the release shares (DES-CI-WORKFLOW).

Follow-up: mara-apb968 (lint the WebAssembly build).
