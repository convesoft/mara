---
title: CI runs on the release's build images, and the release has no check jobs of its own
status: in-progress
type: task
parent: mara-c8scn7
related:
  - mara-9r3bqr
created_at: 2026-11-23T11:31:49Z
---

Goal: CI runs the local checks on `ubuntu-22.04` and `ubuntu-22.04-arm`, the release's build images, and the release workflow has no check jobs of its own: a green `main` means the checks passed where the binaries are built.

Context: decided by the maintainer on 2026-11-23, before `0.4.0-rc.1` is published ("Why checks in Release when we have them in CI already so main is always green? like we have in nx-rust"). CI runs on every pull request and every update of `main`. The release had check jobs only because CI ran on `ubuntu-24.04` while the release builds on `ubuntu-22.04` for the glibc 2.35 floor (`ADR-LINUX-BUILD-IMAGE`); two test defects showed only on 22.04 (mara-efyg6p, mara-4xsamc). The Build jobs took Soufflé from the check jobs' artifact; they now build it themselves through `cargo-workspace:souffle`, since the release restores no cache (`DES-RELEASE-WORKFLOW`, `ADR-DISPATCHED-RELEASE`). The shared setup action keeps installing cargo-nextest (mara-b86nfg covers trimming it). Items: `DES-CI-WORKFLOW`, `DES-RELEASE-WORKFLOW`, `ADR-CI-CHECKS`, `ADR-DISPATCHED-RELEASE`, `ADR-LINUX-BUILD-IMAGE`.

- [ ] `ci.yml`: the matrix is `ubuntu-22.04` and `ubuntu-22.04-arm`
- [ ] `release.yml`: the `checks` job removed; Pack dispatcher no longer needs it; each Build job builds Soufflé
- [ ] Decide whether Prepare checks CI's result on the recorded commit or relies on `main`'s required checks, and record it in `ADR-DISPATCHED-RELEASE`
- [ ] The five Mara items and mara-9r3bqr updated; AGENTS.md and README checked
- [ ] The pull request's CI run passes on both images, with the expected test counts
- [ ] A dry dispatch of the release on the branch passes
- [ ] `project validate` and `zban check` pass
