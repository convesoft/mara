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

- [ ] The maintainer has read the list
