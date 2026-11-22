---
title: "Nx workspace: the repository's tasks run through Nx with nx-rust"
status: todo
type: epic
created_at: 2026-11-21T23:23:01Z
---

Outcome: this repository is an Nx workspace in which `@convesoft/nx-rust` makes the two crates Nx projects, and one Nx command runs the local checks of AGENTS.md with the results the plain cargo commands give. Cargo stays the build tool, and the cargo commands keep working.

Done when, in a fresh worktree after `pnpm install`, one documented `nx` command runs every local check and passes; a second run at the same tree restores the cached targets; a change to a file a check reads (a crate's source, a page under `docs/`, the Soufflé binary) runs that check again; and AGENTS.md describes it.

Asked by the maintainer on 2026-11-21, before the alpha.6 slice starts: "Before we start next milestone could we move this repo on this tooling? https://github.com/convesoft/nx-rust … Let it be a small epic because we need to initialize nx workspace for example and other routine". The slice mara-xzfeyj (Node library and browser) is blocked by this epic: its JS packages and its napi and WASM artifacts are to be built in this workspace.

Decided by the maintainer: until `@convesoft/nx-rust` is on npm, the repository uses the plugin from the maintainer's local checkout by path, loaded from its TypeScript source without a build (`/home/araketski/Work/github.com/convesoft/nx-rust/nx-rust`; mara-a4ga3m has what that needs), which works on this machine only; the dependency becomes the npm version when the package is published.

Decided by the orchestrator, provisional (review issue of this epic): the Nx targets are added beside the cargo commands, which stay documented as the fallback, and the orchestrator's gate runs both until a run has shown the same results; worktrees do not share an Nx cache, so the gate executes its checks itself.

nx-rust: Nx 24, Cargo 1.91 or later, Linux and macOS; its documentation is `nx-rust/docs/` of that checkout (setup, targets, caching, configuration, testing).

Changed by the maintainer on 2026-11-22 (mara-tt31bp): the plugin is not loaded from the local checkout. The repository waits for the npm release of `@convesoft/nx-rust` and depends on it from the start; mara-a4ga3m does mara-mrmg89 in the same change.
