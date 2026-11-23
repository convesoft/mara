---
title: CI runs on the release's build images, and the release has no check jobs of its own
status: done
type: task
parent: mara-c8scn7
related:
  - mara-9r3bqr
created_at: 2026-11-23T11:31:49Z
closed_at: 2026-11-23T12:07:35Z
---

Goal: CI runs the local checks on `ubuntu-22.04` and `ubuntu-22.04-arm`, the release's build images, and the release workflow has no check jobs of its own: a green `main` means the checks passed where the binaries are built.

Context: decided by the maintainer on 2026-11-23, before `0.4.0-rc.1` is published ("Why checks in Release when we have them in CI already so main is always green? like we have in nx-rust"). CI runs on every pull request and every update of `main`. The release had check jobs only because CI ran on `ubuntu-24.04` while the release builds on `ubuntu-22.04` for the glibc 2.35 floor (`ADR-LINUX-BUILD-IMAGE`); two test defects showed only on 22.04 (mara-efyg6p, mara-4xsamc). The Build jobs took Soufflé from the check jobs' artifact; they now build it themselves through `cargo-workspace:souffle`, since the release restores no cache (`DES-RELEASE-WORKFLOW`, `ADR-DISPATCHED-RELEASE`). The shared setup action keeps installing cargo-nextest (mara-b86nfg covers trimming it). Items: `DES-CI-WORKFLOW`, `DES-RELEASE-WORKFLOW`, `ADR-CI-CHECKS`, `ADR-DISPATCHED-RELEASE`, `ADR-LINUX-BUILD-IMAGE`.

- [x] `ci.yml`: the matrix is `ubuntu-22.04` and `ubuntu-22.04-arm`
- [x] `release.yml`: the `checks` job removed; Pack dispatcher no longer needs it; each Build job builds Soufflé
- [x] Decide whether Prepare checks CI's result on the recorded commit or relies on `main`'s required checks, and record it in `ADR-DISPATCHED-RELEASE`
- [x] The five Mara items and mara-9r3bqr updated; AGENTS.md and README checked
- [x] The pull request's CI run passes on both images, with the expected test counts
- [x] A dry dispatch of the release on the branch passes
- [x] `project validate` and `zban check` pass

## Summary of Changes

- `ci.yml` runs on `ubuntu-22.04` and `ubuntu-22.04-arm`; the jobs are `Checks (ubuntu-22.04)` and `Checks (ubuntu-22.04-arm)`.
- `release.yml` has no `checks` job: Pack dispatcher needs only Prepare, and each Build job builds Soufflé from source through `pack-native`'s dependency `cargo-workspace:souffle`.
- `DES-CI-WORKFLOW`, `DES-RELEASE-WORKFLOW`, `ADR-CI-CHECKS`, `ADR-DISPATCHED-RELEASE`, `ADR-LINUX-BUILD-IMAGE` and `ADR-NX-RUST-TASKS` (which named the release's checks) updated; mara-9r3bqr now lists only the release build, the packing and the install verification as unchecked at a merge. AGENTS.md and README needed no change.

Checks: PR #76's CI run https://github.com/convesoft/mara/actions/runs/45168168187 passed on both images (830 passed, 14 skipped in the workspace batches; 32 passed, 2 skipped in the performance run; `project validate` 0 errors, 0 warnings). Dry dispatch https://github.com/convesoft/mara/actions/runs/45166328048 passed in 11 minutes (about 30 before): Prepare, Pack dispatcher, both Builds (Soufflé 8 min 47 s on x64, 6 min 44 s on arm64, all 13 install-verification cases), Dry run; Publish, Registry check and Tag skipped. Locally: `project validate` 0 errors, 0 warnings; `zban check` clean.

## Decisions made

- Prepare does not read CI's check runs for the recorded commit; the workflow relies on `main`'s ruleset (pull requests only, required checks, no bypass), as nx-rust's release does. Rejected: a `checks: read` lookup in Prepare, which repeats the required check names beside the ruleset and fails a dispatch made while `main`'s push run is still running. Known limit recorded: the ruleset does not require an up-to-date branch, so a publication is dispatched after CI's push run on `main` passed.
- The release builds Soufflé through `pack-native`'s existing dependency, with no new step; the shared setup action stays as it is (mara-b86nfg).
