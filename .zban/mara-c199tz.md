---
title: Set the workspace version to 0.4.0-rc.2
status: todo
type: task
parent: mara-xzfeyj
created_at: 2026-11-25T10:18:35Z
---

Goal: the binary and the packages built from `main` report version `0.4.0-rc.2`, so that the release workflow's dry dispatches of this slice run: a dispatch refuses a version that npm has, and npm has `0.4.0-rc.1`.

Context: `DES-RELEASE-WORKFLOW` (docs/distribution.mara.md) sets the version by hand: one commit changes `[workspace.package].version` in `Cargo.toml` and the workspace crates' entries in `Cargo.lock`. mara-jbkcxe did the same for rc.1. A trivial chore: no reviewer; the orchestrator checks the result. It comes first in the slice, because mara-23j2h9, mara-gn76br and every later issue that changes a release input owe a dry dispatch on their branch.

- [ ] `version` in the workspace `Cargo.toml` is `0.4.0-rc.2`, and `Cargo.lock` follows for every workspace package
- [ ] Anything that states the current version as a fact about the build (a test, a doc sentence) follows; mentions of `0.4.0-rc.1` as the published prerelease stay
- [ ] `mara --version` prints `mara 0.4.0-rc.2`, the local checks pass, and the diff holds nothing but the version
