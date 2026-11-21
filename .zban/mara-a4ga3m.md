---
title: "Initialize the Nx workspace with nx-rust: the crates are projects and their inferred targets pass"
status: in-progress
type: feature
parent: mara-1fab1q
created_at: 2026-11-21T23:23:19Z
---

Goal: after `pnpm install` in a checkout, `nx show projects` lists `mara` and `mara-engine` and the workspace project, and the targets nx-rust infers for them (`fmt-check`, `lint`, `check`, `build`, `test`) pass, are cached, and run again when something they read changes.

Context: the epic mara-1fab1q, with the maintainer's decisions. AGENTS.md ("Engineering") lets no build system in that `docs/` does not specify unless the maintainer approves it; the maintainer asked for this one, and this issue writes the decision into the docs. nx-rust's own documentation is in the maintainer's checkout, `/home/araketski/Work/github.com/convesoft/nx-rust/nx-rust/docs/` (`setup.mara.md`, `targets.mara.md`, `caching.mara.md`, `configuration.mara.md`, `testing.mara.md`): read-only for this work. The package is not on npm: the dependency is the build output `/home/araketski/Work/github.com/convesoft/nx-rust/dist/nx-rust`, by path. That folder is rewritten by the nx-rust project's own builds and end-to-end runs, so the dependency should be a copy taken at install (`file:`), not a link, unless that fails.

What the plugin cannot see and the cache keys must still cover (found by the orchestrator at `46ccb4a`; check for more):
- Files outside the crates that their code and tests read: `docs/format.mara.md` and `docs/rules/index.mara.md` by `include_str!` in `crates/mara-engine/src`, and `crates/mara/tests/documents.rs` reads `docs/` whole (`fileInputs`).
- The Soufflé binary the tests run, `target/souffle-2.5/souffle`, built by `scripts/build-souffle.sh`: a file pattern may not point into Cargo's artifact directory, so it is a command input (`runtimeInputs`) or Soufflé moves out of `target/`.
- Environment variables that change what the tested client does: `CI` and `MARA_NO_DAEMON` (`crates/mara/src/cli/client.rs`); look for others (`envInputs`).
- The pinned Rust grammar in the per-user cache, for anything that validates this repository's own project.

- [ ] The decision is an item in `docs/` (a Decision in docs/decisions.mara.md, or where the docs keep such choices): Nx with nx-rust runs the repository's tasks, cargo stays the build tool, the interim dependency by path and what replaces it, with the rejected alternatives one line each
- [ ] Root files: a private `package.json` with a pinned package manager and Nx 24, `pnpm-lock.yaml`, `nx.json` with the plugin registered as its `init` generator does, and `.gitignore` entries for `node_modules` and Nx's own folders; nothing is written into the crates but `[package.metadata.nx]` or `[workspace.metadata.nx]` tables
- [ ] The inputs above are declared, and the summary shows for each that changing it runs the affected target again and that an unchanged tree restores from the cache
- [ ] `fmt-check`, `lint` (with warnings denied, as the local check does), `build` and `test` (locked) pass through Nx and give the results the cargo commands give; the cargo commands still pass
- [ ] AGENTS.md says what a fresh worktree needs (`pnpm install`) and how agents on this machine run Nx (no daemon, the local Nx), in the fewest lines; "Local checks" keeps its cargo commands until the next issue
