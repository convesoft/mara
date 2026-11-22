---
title: "Specify the rc.1 slice: prerelease versions on npm, CI, Linux packages and the release workflow"
status: in-progress
type: task
parent: mara-c8scn7
created_at: 2026-11-22T18:44:35Z
---

Goal: the Mara documents say what `0.4.0-rc.1` is, how it is built, checked and published, and how each of that is verified, so that the other issues of the epic implement Accepted items.

Context: the epic (parent) gives the maintainer's decision and the references. Read zban's `release.yml`, `ci.yml`, `scripts/package-npm.mjs` and `npm/zban.cjs`, and nx-rust's workflows, as the pattern the maintainer already uses: a dispatcher package with `optionalDependencies` on one native package per platform, a `workflow_dispatch` release with a dist-tag and a dry run that is the default, npm Trusted Publishing with provenance, publication of the native packages before the dispatcher, a registry smoke test, and the tag and GitHub release last. Carry over understanding, not text. Items to change or add: `DES-PACKAGES`, `DES-SOUFFLE-BUILD`, `REQ-RUN-WITHOUT-TOOLCHAIN` and its verification (docs/distribution.mara.md, docs/verification.mara.md), `ADR-NX-RUST-TASKS` where CI runs the Nx targets, and AGENTS.md.

- [ ] `DES-PACKAGES`: prereleases of 0.4.0 are published to npm under the dist-tag `next`, starting with `0.4.0-rc.1`; `latest` moves only with 0.4.0. The slices are named: rc.1 (this epic), rc.2 (the Node library and browser, mara-xzfeyj). What an rc.1 native package holds (the CLI and `souffle`, with their licences; no Node addon) and that there is no browser package before rc.2
- [ ] The platforms of rc.1: Linux x64 and arm64 (glibc). macOS arm64 stays with mara-ag4ees. The oldest glibc the binaries run on follows from the CI image they are built on: name the image and the resulting minimum, and how a too old system fails (a message the user can act on, as far as the dispatcher can tell)
- [ ] A Design item for CI: which workflow runs which Nx targets on which events and branches, that the integration branch is checked on push until the rewrite is on `main`, how Soufflé is built and cached per platform (`DES-SOUFFLE-BUILD` already says CI builds it), which checks CI leaves to the nightly job of mara-4dr2p2 (the 64× scaling run, `hyperfine` budgets), and whether the performance test runs on shared runners
- [ ] A Design item for the release workflow: its inputs (dist-tag, dry run by default), the branch it may run from, that every package is built from one commit after the checks, the order of publication, what is verified before and after publishing (packed file lists, a local install without lifecycle scripts, a registry smoke test), how the version is set (`nx release version`, as zban, or by hand) and who creates the tag. Publication happens only when the maintainer dispatches the workflow without the dry run
- [ ] A Verification item for the install: on each rc.1 platform, a packed dispatcher and native package installed with `--ignore-scripts` into an empty directory run `--version`, validate a small project, evaluate a rule program with the bundled `souffle`, and answer an MCP `initialize`; no Rust toolchain, C++ compiler or other Soufflé is on `PATH` in that job
- [ ] AGENTS.md: the lines "One epic per release slice, `0.4.0-alpha.1` to `0.4.0`", "No CI exists until the last slice (mara-4dr2p2), and no workflow file is added before it" and the tag rule agree with the new items; the file stays within its size limit
- [ ] What the maintainer must do outside the repository is listed in the closing summary, not decided: the npm package names to create and their Trusted Publisher settings, the GitHub environment for publishing, and when `main` is fast-forwarded to the integration branch (question issue of this epic)
- [ ] `project validate` reports 0 errors

Not part of this issue: any workflow file, script or package (the other issues of the epic), the budgets of `REQ-TIME-BUDGETS`.
