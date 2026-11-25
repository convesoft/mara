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
- CI installs Playwright's Chromium (without its headless shell) in `ci.yml`; DES-CI-WORKFLOW, ADR-NX-RUST-TASKS, DES-BROWSER-WORKSPACE and AGENTS.md "Local checks" say so; AGENTS.md lost its obsolete section "Until `main` is fast-forwarded".
- Fix round 1 (review of cce334d): the module's frames are boxed slices of their exact length and its numbers are checked conversions; a second workspace in one instance, arguments that are no object (after `project init` and `project convert` answer), arguments that cannot be sent to the worker, and a NUL in a path are refused with `invalid-argument`; the worker reads the module's pointers and lengths as unsigned and serializes a call before the module sees it, so such a call no longer stops the workspace; the package takes bytes from another realm; the memory drops a directory with its last file; a stat in the memory settles at once; the host tests launch Chromium by the path that `support/browser.mjs` resolves and key its path, size and modification time; `mara-browser:build` refuses a target directory other than `target/` (mara-6z7bjs); `mara-wasm:lint:wasm32` lints the WebAssembly build in `pnpm run checks` (mara-apb968, done); `origin/main` with #83 merged without conflicts.
- Checks: `pnpm run checks` with a fresh Nx cache after merging `origin/main` with #83: 882 passed, 0 failed, 16 skipped (850 and 14 in the workspace run, 32 and 2 in the performance run), `project validate` 0 errors and 0 warnings; 22 Node cases in Chromium. Probes: with a warm cache and Chromium's executable moved aside under a private `PLAYWRIGHT_BROWSERS_PATH`, `mara:test --test=hosts` ran again and failed naming the executable, and was restored when it was moved back; with `CARGO_TARGET_DIR` redirected, `mara-browser:build` failed naming the directory and mara-6z7bjs, and was restored without it; each input of the first round's probes as before.

## Decisions made

- The browser runs the `mara` library's own host code over a filesystem in the module's memory (ADR-MEMORY-FILESYSTEM).
- The module exports plain functions with frames of a JSON header and file bytes, built by Cargo alone, no `wasm-bindgen` (ADR-RAW-WASM-MODULE).
- The JavaScript API, its refusals (`invalid-argument` for malformed or conflicting paths, a removal where no file is, an unknown operation, a `project` argument; `unavailable-operation` before arguments are read), and the memory's logical clock (DES-BROWSER-WORKSPACE).
- A read from the memory counts no file read: the rule stands as VER-WORK-COUNTERS states it; the orchestrator's contrary decision of fix round 1 was reversed on that item.
- `mara-wasm:build` builds the release module by default and adds `mara_wasm.wasm` to the plugin's outputs; `mara:test` depends on `mara-browser:build` (ADR-NX-RUST-TASKS).
- The grammar fixture of the unavailable cases is `empty` with the code files and pins of VER-GRAMMAR-MARKERS and no program, since programs need Soufflé's module (mara-xsm8w6).
- Chromium is installed in `ci.yml`, not in the setup action that the release shares, and without its headless shell (DES-CI-WORKFLOW).
- Decided by the orchestrator in the review: the design stands, the library's host code over an in-memory filesystem held per WebAssembly instance (ADR-MEMORY-FILESYSTEM); the plain module boundary (ADR-RAW-WASM-MODULE) stands provisionally, `wasm-bindgen` through nx-rust's `wasm-build` going to the maintainer as a question.
- The host tests launch Chromium from `chromium.executablePath()` and key that executable's path, size and modification time, not a hash of some 300 MB (ADR-NX-RUST-TASKS).
- `mara-browser:build` refuses a Cargo target directory other than `target/`, since a restored `mara-wasm:build` leaves its module only there (ADR-NX-RUST-TASKS, mara-6z7bjs).
- The memory drops a directory with its last file, as a Git checkout holds none, and a stat there settles at once (DES-BROWSER-WORKSPACE).
- `mara-wasm:lint:wasm32`, a configuration of the inferred `lint`, joins `pnpm run checks` (ADR-NX-RUST-TASKS, mara-apb968).

Follow-up mara-apb968 was done in this issue's fix round.
