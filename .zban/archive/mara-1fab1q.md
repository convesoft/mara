---
title: "Nx workspace: the repository's tasks run through Nx with nx-rust"
status: done
type: epic
created_at: 2026-11-21T23:23:01Z
closed_at: 2026-11-22T11:48:34Z
---

Outcome: this repository is an Nx workspace in which `@convesoft/nx-rust` makes the two crates Nx projects, and one Nx command runs the local checks of AGENTS.md with the results the plain cargo commands give. Cargo stays the build tool, and the cargo commands keep working.

Done when, in a fresh worktree after `pnpm install`, one documented `nx` command runs every local check and passes; a second run at the same tree restores the cached targets; a change to a file a check reads (a crate's source, a page under `docs/`, the Soufflé binary) runs that check again; and AGENTS.md describes it.

Asked by the maintainer on 2026-11-21, before the alpha.6 slice starts: "Before we start next milestone could we move this repo on this tooling? https://github.com/convesoft/nx-rust … Let it be a small epic because we need to initialize nx workspace for example and other routine". The slice mara-xzfeyj (Node library and browser) is blocked by this epic: its JS packages and its napi and WASM artifacts are to be built in this workspace.

Decided by the maintainer: until `@convesoft/nx-rust` is on npm, the repository uses the plugin from the maintainer's local checkout by path, loaded from its TypeScript source without a build (`/home/araketski/Work/github.com/convesoft/nx-rust/nx-rust`; mara-a4ga3m has what that needs), which works on this machine only; the dependency becomes the npm version when the package is published.

Decided by the orchestrator, provisional (review issue of this epic): the Nx targets are added beside the cargo commands, which stay documented as the fallback, and the orchestrator's gate runs both until a run has shown the same results; worktrees do not share an Nx cache, so the gate executes its checks itself.

nx-rust: Nx 24, Cargo 1.91 or later, Linux and macOS; its documentation is `nx-rust/docs/` of that checkout (setup, targets, caching, configuration, testing).

Changed by the maintainer on 2026-11-22 (mara-tt31bp): the plugin is not loaded from the local checkout. The repository waits for the npm release of `@convesoft/nx-rust` and depends on it from the start; mara-a4ga3m does mara-mrmg89 in the same change.

## Summary of Changes

The repository is an Nx 24.0.6 workspace. `@convesoft/nx-rust@1.0.0-rc.1` from npm makes `mara` and `mara-engine` Nx projects, and the checks the plugin does not infer are hand-written targets in the manifests' Nx metadata (`souffle`, `validate`, `test-performance`, `zban-check`, a `wasm32` configuration of `mara-engine:build`). `pnpm run checks` runs every local check of AGENTS.md and `pnpm run checks:fast` leaves out the performance test and the `wasm32` build. The cargo commands stay in AGENTS.md as what the targets run and as the fallback. ADR-NX-RUST-TASKS records the decisions. Issues: mara-a4ga3m with mara-mrmg89 (commit 7b758dd), mara-jmt2hr (commit af4b4c1); mara-tt31bp was the maintainer's choice to wait for the npm release.

"Done when", run by the orchestrator on 2026-11-22 in a fresh clone of af4b4c1 (no `target/`, no `node_modules`, empty cache), with `NX_DAEMON=false NX_USE_LOCAL=true`:

- `pnpm install --frozen-lockfile`, then `pnpm run checks`: exit 0, Soufflé built in the run, 855 passed, 0 failed, 16 ignored (823 tests and the 32 performance tests), `project validate` 0 errors and 1 warning. The seven cargo and `zban` commands give the same counts (the gate at be2bc2d).
- A second run at the same tree restored 9 of 12 tasks and the `wasm32` build; `souffle`, `lockfile` and `test-performance` are never cached and ran.
- A comment added to `crates/mara/src/main.rs` ran `mara`'s `fmt-check`, `lint`, `build`, `test` and `validate` again; a newline in `docs/index.mara.md` ran `mara:test` and `mara:validate`; a mode change of the Soufflé binary ran `mara:test` and `mara:validate`. With all three reverted, every cached task was restored.
- AGENTS.md "Local checks" names the scripts and how to run them on this machine.

No tag: the epic is no release slice. No architecture audit: the epic changed no Rust code, only `Cargo.toml` metadata and configuration. The decisions are listed in mara-c4y5ts.
