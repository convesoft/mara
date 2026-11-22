---
title: Every local check is an Nx target, and package.json scripts combine them
status: in-progress
type: feature
parent: mara-1fab1q
blocked_by:
  - mara-a4ga3m
created_at: 2026-11-21T23:23:32Z
---

Goal: the checks that nx-rust does not infer are custom Nx targets, and `package.json` scripts combine the usual checks, so that one command runs everything AGENTS.md asks before a branch is handed over.

Context: the epic mara-1fab1q; mara-a4ga3m set up the workspace and the inferred targets. The maintainer, on 2026-11-21: "all the things plugin don't handle could be still done through custom nx targets which call zban for example, and we can then combine usual checks we need in package.json scripts". The local checks of AGENTS.md and what they become:

| Local check | Under Nx |
| --- | --- |
| `cargo fmt --all --check`, clippy with warnings denied, `cargo test --workspace --locked` | the inferred targets (mara-a4ga3m) |
| `scripts/build-souffle.sh`, which the tests need | a custom target the tests depend on |
| `cargo run --package mara … project validate` | a custom target that depends on the `mara` build; it reads `docs/`, `.mara/`, every Rust file of the crates and the pinned grammar in the per-user cache |
| `cargo test --package mara --test performance --release --locked` | a custom target: the plugin has no option to choose one test binary; it times a release build, so it runs alone (no other task beside it) |
| `cargo build --package mara-engine --target wasm32-unknown-unknown --locked` | the inferred `build` with a target triple, or a custom target |
| `zban check` | a custom target |

A custom target is cached only if its inputs are declared completely; one that cannot name its inputs is not cached. A timing test is no candidate for a result restored from another machine state: say in the summary what was decided for it and why.

- [ ] Each row above is an Nx target with its inputs and its dependencies on other targets, and each gives the result of the cargo or `zban` command it replaces
- [ ] `package.json` has scripts that combine them: at least one that runs every local check, and the fast subset a developer runs while working
- [ ] AGENTS.md "Local checks" names the script as the way to run them, and keeps the cargo commands as what the targets run and as the fallback (they stay until the maintainer says otherwise); the file stays within 120 lines, not counting the managed Nx block
- [ ] ADR-NX-RUST-TASKS: the custom targets and their inputs are recorded there, and its Warnings bullet lists each rejected alternative on its own line (left over from the re-check of mara-a4ga3m)
- [ ] The summary shows: a full run in a fresh worktree; a second run at the same tree and what it restored; and for one change each to a crate's source, a page under `docs/` and an issue file under `.zban/`, which targets ran again
