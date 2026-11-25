---
title: Set the workspace version to 0.4.0-rc.2
status: done
type: task
parent: mara-xzfeyj
created_at: 2026-11-25T10:18:35Z
closed_at: 2026-11-25T10:46:28Z
---

Goal: the binary and the packages built from `main` report version `0.4.0-rc.2`, so that the release workflow's dry dispatches of this slice run: a dispatch refuses a version that npm has, and npm has `0.4.0-rc.1`.

Context: `DES-RELEASE-WORKFLOW` (docs/distribution.mara.md) sets the version by hand: one commit changes `[workspace.package].version` in `Cargo.toml` and the workspace crates' entries in `Cargo.lock`. mara-jbkcxe did the same for rc.1. A trivial chore: no reviewer; the orchestrator checks the result. It comes first in the slice, because mara-23j2h9, mara-gn76br and every later issue that changes a release input owe a dry dispatch on their branch.

- [x] `version` in the workspace `Cargo.toml` is `0.4.0-rc.2`, and `Cargo.lock` follows for every workspace package
- [x] Anything that states the current version as a fact about the build (a test, a doc sentence) follows; mentions of `0.4.0-rc.1` as the published prerelease stay
- [x] `mara --version` prints `mara 0.4.0-rc.2`, the local checks pass, and the diff holds nothing but the version

## Summary of Changes

- `[workspace.package].version` in `Cargo.toml` is `0.4.0-rc.2`; `cargo update --workspace --offline` moved `mara`, `mara-engine` and `docs-checks` in `Cargo.lock`.
- `cargo run --quiet --bin mara -- --version` prints `mara 0.4.0-rc.2`.
- `pnpm run checks` (fresh Nx cache): 862 passed, 0 failed, 16 skipped (830/14 workspace, 32/2 performance); `project validate` 0 errors, 0 warnings.
- Left unchanged: the README's `0.4.0-rc.1` lines (they state what npm has until rc.2 is published), DES-PACKAGES and DES-RELEASE-WORKFLOW (published prerelease).

## Decisions made

- None.
