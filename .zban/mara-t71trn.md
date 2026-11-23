---
title: Review the decisions made in 0.4.0-rc.1
status: todo
type: task
tags:
  - maintainer
created_at: 2026-11-22T18:44:35Z
---

Goal: the maintainer reviews what the orchestrator and the workers decided on their own in the rc.1 epic.

## Decided by the maintainer

- 2026-11-22: the rewrite is published before 0.4.0, as `0.4.0-rc.1` under the npm dist-tag `next`, and the Node library and browser slice (mara-xzfeyj) becomes `0.4.0-rc.2`. A small epic resolves CI and publishing, with the zban and nx-rust repositories as references. rc.1 is to be used to migrate other repositories from the 0.3.0 format.
- 2026-11-22: tests are to run with nextest and affected selection for rapid local development, after measurements ("Agree better to gather numbers"); the full suite runs only where it is really necessary; the tests are to be decoupled from `docs/` where that is possible; a script that T3 Code runs for a new worktree copies the baseline into it; cargo-nextest and cargo-affected may be installed on the machine ("Feel free to install them").
- 2026-11-22: the tests that read this repository's docs "are not source code tests": they run beside `mara:validate`, as a project of their own with only the docs as their concern (mara-8mxw9c).
- 2026-11-22: `main` will be fast-forwarded to the integration branch, and the slice after rc.1 (mara-xzfeyj) integrates by pull requests (mara-bkqvgd).

- 2026-11-22: the npm package names are the ones 0.3.0 published (`@convesoft/mara`, `@convesoft/mara-linux-x64-gnu`, `@convesoft/mara-linux-arm64-gnu`) and the publishing jobs use the existing GitHub environment `release` ("check npm plz they were existing already").
- 2026-11-22: the CI and release workflows are specified and built in the form they keep for 0.4.0, without gates that hold only for a slice ("the CI should be almost final version unless we discover something is missing"). The rules that a real dispatch must use `next` and that `latest` is refused until mara-chjvk9 were removed from `DES-RELEASE-WORKFLOW`.

## Decided by the orchestrator

One line per decision made on the maintainer's behalf: what was decided, what it rests on, how to undo it. "Provisional" marks a decision that is the maintainer's to confirm and stands until they answer.

- 2026-11-22, provisional: rc.1 is for Linux x64 and arm64; macOS arm64 stays with mara-ag4ees and does not block rc.1. Rests on the macOS build of Soufflé never having been tried. Undo: block the release issue of the epic by mara-ag4ees.
- 2026-11-22, provisional: the rc.1 native package holds the CLI and `souffle` only; the Node addon and the browser package come with rc.2. Rests on neither existing yet.
- 2026-11-22: the full suite is needed on `main` after each merge, in the release workflow, where the plugin falls back to it, and on demand; `pnpm run checks` selects on a developer's machine once the switch is made. Until pull requests carry the integration, the orchestrator's check before a squash merge stays a full run. Rests on the maintainer's "run full tests only when it is really necessary" and on an affected run never standing in for a full one. Undo: `NX_RUST_TEST_MODE=full` in the `checks` script.
- 2026-11-22: cargo-nextest 0.9.152, cargo-affected 0.4.1 (with `cargo install --locked` into the Cargo home) and the `llvm-tools` component were installed on the maintainer's machine by the orchestrator, with the maintainer's permission. Undo: `cargo uninstall cargo-nextest cargo-affected`.

- 2026-11-22, provisional (mara-a57pzn): the published Linux binaries are built on `ubuntu-22.04` and `ubuntu-22.04-arm`, so the minimum glibc is 2.35, as 0.3.0 (`ADR-LINUX-BUILD-IMAGE`). Rests on keeping Debian 12 and Ubuntu 22.04. Undo: change the build images in the release workflow and the decision.
- 2026-11-22 (mara-a57pzn): `mara --version` is added to `DES-OPERATIONS` and built by mara-e8921x. Rests on the epic's "Done when" and on 0.3.0 having it. Undo: drop the line and the issue.
- 2026-11-22 (mara-a57pzn): one version and tag rule stays in the release workflow because it holds after 0.4.0 too: a version with a prerelease part is refused under `latest`. It is not in zban's workflow. Undo: remove the sentence from `DES-RELEASE-WORKFLOW`.
- 2026-11-22 (mara-a57pzn): CI runs `pnpm run checks` on `ubuntu-24.04` and `ubuntu-24.04-arm`, keeps no Nx cache between runs, caches Soufflé and Cargo's build directory, and runs the performance test on shared runners without a retry (`ADR-CI-CHECKS`). Undo: the decision and the workflow.
- 2026-11-22 (mara-a57pzn): the version is set by hand in `Cargo.toml`, not by `nx release version`; publication runs only from `main`, a dry run from any ref and on every push to `main` and the integration branch (`ADR-DISPATCHED-RELEASE`).
- 2026-11-22 (mara-a57pzn review): a P3 of the re-check is left for the packages issue: `DES-PACKAGES` says `latest` stays 0.3.0 "on every package", which a package first published with rc.2 cannot satisfy.

- 2026-11-22 (mara-8mxw9c): the test total is 856 passed (855 before): the comparison of `project init` with this repository became a test of its own in `docs-checks`. `docs-checks:test` reads the pages, `.mara/` and the templates at run time and keys `TMPDIR` only, because it starts neither the binary nor Soufflé (`ADR-NX-RUST-TASKS`). Two findings of its review are older than the change and stay in the backlog (mara-6z7bjs, mara-5c83v4).
- 2026-11-22: filed mara-jbkcxe (the workspace version becomes `0.4.0-rc.1`), which the epic had no issue for; it blocks the release workflow issue.

- 2026-11-22 (mara-e8921x): until the parser has read a complete operation name, `--version` in option position is the top-level flag (`mara project --version` is rejected); the performance harness gets no case for it; the code carries no `@mara implements` marker, because the flag is outside the document operation set (`DES-OPERATIONS`, `DES-PERFORMANCE-HARNESS`). Undo: the item text and `crates/mara/src/cli.rs`.

- 2026-11-22 (mara-397g3g): the migration steps tell the user to delete the files the conversion leaves for 0.3.0, in a commit of its own after the conversion commit, so that every state can be restored from Git (docs/migration.mara.md, "Steps"). Rests on 0.3.0 otherwise reading the converted project as one without items (read in the 0.3.0 source, not run). Undo: make step 8 optional.
- 2026-11-22 (mara-397g3g): the README installs the prerelease with `--save-exact`, and agent setups pin the MCP server and every command of this version to the exact prerelease version through one launcher; a 0.3.0 skill belongs to the 0.3.0 projects, not to the user (docs/agents.mara.md). Undo: the page text.

- 2026-11-22 (mara-7wysb0): `affectedTests` stays off and every uncached `test` task runs its full suite; the runner switches to nextest in mara-rw6rhp, which stays in this epic and blocks the release workflow issue (`ADR-NX-RUST-TASKS`). Rests on the measurements in the issue: 84 s against 159 s for the full suite with equal results; an affected run saves nothing for a Rust change (83.6 s against 84.0 s) because `mara` has a binary; the coverage baseline does not collect on this repository (494 of 825 tests fail under instrumentation; 10 GiB peak with the causes worked around). mara-azya4r (the baseline in CI) is cancelled, as the maintainer said for this outcome. Undo: mara-vxeweg (backlog) makes collection pass, then `affectedTests = true`.
- 2026-11-22, provisional (mara-svq540, question mara-6qcfn5): the licence notices of the crates in the native packages come from a generator in the repository over `cargo metadata`, not from cargo-about.

- 2026-11-22 (mara-svq540): the dispatcher runs a native package only from a `node_modules` directory on its own ancestor path and only at exactly its own version; anything else counts as not installed (`DES-PACKAGES`). It ends by the child's signal where Node can re-raise it and exits 128 + signal otherwise (mara-6y6hnn, backlog, for core-dumping signals).
- 2026-11-22 (mara-svq540): the packer reads the version and the artifact directory from `cargo metadata`, so the Pack dispatcher job of the release installs the pinned Rust toolchain though it compiles nothing; `pack-native` runs as `mara-npm:pack-native:release`, refuses `CARGO_BUILD_TARGET` and cannot see a `build.target` of a Cargo configuration file (`DES-RELEASE-WORKFLOW`). The three targets are not part of `pnpm run checks`.
- 2026-11-22 (mara-svq540): not shown before the release workflow runs: the arm64 package, the `GLIBC_` floor on the `ubuntu-22.04` image (the check fails on this machine by design), the registry mode of the install check. The dry run of mara-a3aq0n is their first evidence.

- 2026-11-22 (mara-1avrq4): CI caches the three directories cargo writes under `target/` and Cargo's registry, by name, so that the Soufflé directory cannot be in that cache; Soufflé has its own entry keyed by its build script, without a fallback (`DES-CI-WORKFLOW`, `ADR-CI-CHECKS`). `zban` is an exact devDependency, and `zban-check` keys the version and the file of the `zban` it runs. A warm run takes about 10 minutes on each image, a cold one about 17.
- 2026-11-22: reviewers' findings dropped as exotic: a local Git ignore rule that hides a zban warning from the key of `zban-check` (mara-1avrq4); a missing deadline on the MCP read of the version test (mara-e8921x).

- 2026-11-23 (mara-ezzbnc): the check names for branch protection are `Checks (ubuntu-24.04)` and `Checks (ubuntu-24.04-arm)`; no job of the release workflow is recommended as a required check, because its dry run on pull requests starts only when release inputs change and a required check that never runs blocks the merge. The maintainer sets branch protection.

- 2026-11-23 (mara-rw6rhp): the tests run with cargo-nextest 0.9.152, which CI installs from the prebuilt release archive with a pinned SHA-256 per architecture (`ADR-NX-RUST-TASKS`, `ADR-CI-CHECKS`). The full `checks` now counts 861 passed and 16 skipped (cargo: 860 and 16; the extra one is the `harness = false` binary `diagnostic_memory`). Undo: remove `testRunner` from the root `Cargo.toml` and the install step.
- 2026-11-23 (mara-rw6rhp review): two gaps in the key of `test` under nextest are accepted and stated in `ADR-NX-RUST-TASKS`, because this repository has no `.config/nextest.toml` and ignores none: an ignored `.config/nextest.toml` and the contents of nextest's user configuration are not hashed (the plugin's input declaration; backlog mara-dt2aez; reported to the maintainer as a finding about nx-rust).

- [ ] The maintainer has read the list
