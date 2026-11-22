---
title: "Specify the rc.1 slice: prerelease versions on npm, CI, Linux packages and the release workflow"
status: done
type: task
parent: mara-c8scn7
created_at: 2026-11-22T18:44:35Z
closed_at: 2026-11-22T19:48:04Z
---

Goal: the Mara documents say what `0.4.0-rc.1` is, how it is built, checked and published, and how each of that is verified, so that the other issues of the epic implement Accepted items.

Context: the epic (parent) gives the maintainer's decision and the references. Read zban's `release.yml`, `ci.yml`, `scripts/package-npm.mjs` and `npm/zban.cjs`, and nx-rust's workflows, as the pattern the maintainer already uses: a dispatcher package with `optionalDependencies` on one native package per platform, a `workflow_dispatch` release with a dist-tag and a dry run that is the default, npm Trusted Publishing with provenance, publication of the native packages before the dispatcher, a registry smoke test, and the tag and GitHub release last. Carry over understanding, not text. Items to change or add: `DES-PACKAGES`, `DES-SOUFFLE-BUILD`, `REQ-RUN-WITHOUT-TOOLCHAIN` and its verification (docs/distribution.mara.md, docs/verification.mara.md), `ADR-NX-RUST-TASKS` where CI runs the Nx targets, and AGENTS.md.

- [x] `DES-PACKAGES`: prereleases of 0.4.0 are published to npm under the dist-tag `next`, starting with `0.4.0-rc.1`; `latest` moves only with 0.4.0. The slices are named: rc.1 (this epic), rc.2 (the Node library and browser, mara-xzfeyj). What an rc.1 native package holds (the CLI and `souffle`, with their licences; no Node addon) and that there is no browser package before rc.2
- [x] The platforms of rc.1: Linux x64 and arm64 (glibc). macOS arm64 stays with mara-ag4ees. The oldest glibc the binaries run on follows from the CI image they are built on: name the image and the resulting minimum, and how a too old system fails (a message the user can act on, as far as the dispatcher can tell)
- [x] A Design item for CI: which workflow runs which Nx targets on which events and branches, that the integration branch is checked on push until the rewrite is on `main`, how Soufflé is built and cached per platform (`DES-SOUFFLE-BUILD` already says CI builds it), which checks CI leaves to the nightly job of mara-4dr2p2 (the 64× scaling run, `hyperfine` budgets), and whether the performance test runs on shared runners
- [x] A Design item for the release workflow: its inputs (dist-tag, dry run by default), the branch it may run from, that every package is built from one commit after the checks, the order of publication, what is verified before and after publishing (packed file lists, a local install without lifecycle scripts, a registry smoke test), how the version is set (`nx release version`, as zban, or by hand) and who creates the tag. Publication happens only when the maintainer dispatches the workflow without the dry run
- [x] A Verification item for the install: on each rc.1 platform, a packed dispatcher and native package installed with `--ignore-scripts` into an empty directory run `--version`, validate a small project, evaluate a rule program with the bundled `souffle`, and answer an MCP `initialize`; no Rust toolchain, C++ compiler or other Soufflé is on `PATH` in that job
- [x] AGENTS.md: the lines "One epic per release slice, `0.4.0-alpha.1` to `0.4.0`", "No CI exists until the last slice (mara-4dr2p2), and no workflow file is added before it" and the tag rule agree with the new items; the file stays within its size limit
- [x] What the maintainer must do outside the repository is listed in the closing summary, not decided: the npm package names to create and their Trusted Publisher settings, the GitHub environment for publishing, and when `main` is fast-forwarded to the integration branch (question issue of this epic)
- [x] `project validate` reports 0 errors

Not part of this issue: any workflow file, script or package (the other issues of the epic), the budgets of `REQ-TIME-BUDGETS`.

## Summary of Changes

- docs/distribution.mara.md: `REQ-RUN-WITHOUT-TOOLCHAIN` names the platforms of each version and the CLI, MCP server, daemon and rule evaluation. `DES-PACKAGES` gives versions and dist-tags (alpha.1–5 Git tags only; rc.1 and rc.2 under `next`; `latest` stays 0.3.0 until 0.4.0), the packages with provisional names, what the dispatcher and a native package hold, the checked file lists, the platforms, glibc 2.35 from the `ubuntu-22.04` images, and the dispatcher's behaviour and failure messages. `DES-SOUFFLE-BUILD` says where CI and the release build Soufflé. New `DES-CI-WORKFLOW` and `DES-RELEASE-WORKFLOW`. Two open questions: the native package names and the GitHub environment.
- docs/verification.mara.md: new `VER-INSTALL-WITHOUT-TOOLCHAIN`, which verifies `REQ-RUN-WITHOUT-TOOLCHAIN`; the `requirement-verified` warning is gone.
- docs/decisions.mara.md: new `ADR-LINUX-BUILD-IMAGE`, `ADR-DISPATCHED-RELEASE`, `ADR-CI-CHECKS`; one bullet added to `ADR-NX-RUST-TASKS` (CI and the release run `pnpm run checks`).
- docs/hosts/index.mara.md: `DES-OPERATIONS` specifies `mara --version` (the epic's goal; 0.3.0 had it). Implemented by mara-e8921x.
- AGENTS.md: the slice, CI and tag lines follow the new items (99 lines without the Nx block). README's "Find work" row no longer lists the slices. docs/index.mara.md describes the pages.
- Stale CI lines corrected in mara-4dr2p2 and mara-ag4ees; mara-svq540 is now blocked by mara-e8921x.

Checks: `berth --exclusive -- pnpm run checks` exit 0: `project validate` 0 errors, 0 warnings; tests 855 passed, 0 failed, 16 ignored; the wasm32 build and `mara:test:performance` passed; `zban check` no problems.

## Decisions made

- The published Linux binaries are built on `ubuntu-22.04` and `ubuntu-22.04-arm`, so the minimum glibc is 2.35, as in 0.3.0 (`ADR-LINUX-BUILD-IMAGE`). The dispatcher refuses an older or missing glibc with a message.
- CI runs on `ubuntu-24.04` and `ubuntu-24.04-arm`, uses `pnpm run checks` itself, keeps no Nx cache between runs, caches Soufflé (its own entry only) and Cargo's build directory without `target/souffle-*`, fetches the grammar with `code add rust` on each run, and runs the performance test on shared runners (`ADR-CI-CHECKS`).
- `zban` becomes an exact devDependency, so that CI and local runs use the same `zban`.
- The version is set by hand in `Cargo.toml` (`ADR-DISPATCHED-RELEASE`).
- The release has the jobs Prepare, Checks, Pack dispatcher, Build, Dry run, Publish, Registry check and Tag; no package is packed before the checks pass on both platforms. Only Publish (`id-token: write`) and Tag (`contents: write`, through the job's `GITHUB_TOKEN`) can write; both run in the environment `release`, which needs the maintainer's approval, and a dry run never starts them.
- The release restores no cache. Publication runs only from `main`; every push to `main` and to the integration branch runs a dry run without a path filter. Re-running the failed jobs of the same run continues a partial publication, the GitHub release and its assets included.
- The maintainer's (2026-11-22, correcting fix round 1): both workflows have the form they keep for 0.4.0 and after, with no gate that holds only for a slice. `dist_tag` is the maintainer's input; push and pull-request dry runs use `next`. The only version–tag rule is permanent: a version with a prerelease part is never published under `latest`. The Registry check verifies every package's dist-tags after a publication. Recorded in `ADR-DISPATCHED-RELEASE`.
- The release adds three Nx targets, `pack-dispatcher`, `pack-native` and `verify-install`, none cached, and uses a `release` configuration of `mara:build`.
- The workflow file keeps 0.3.0's path, `.github/workflows/release.yml`, which its npm provenance names. GitHub generates the release notes. The package names (the 0.3.0 names) and the environment `release` were settled by the maintainer.
- The install verification also covers a daemon round trip and the dispatcher's failures, because the epic's goal names the daemon.

## For the maintainer, outside the repository

1. Confirm that the Trusted Publisher entries of `@convesoft/mara`, `@convesoft/mara-linux-x64-gnu` and `@convesoft/mara-linux-arm64-gnu` name the workflow `release.yml` and the environment `release`.
2. Fast-forward `main` to the integration branch once the release dry run has passed on its tip (mara-bkqvgd).
3. Set branch protection and the required checks (mara-ezzbnc).
4. Dispatch the release workflow without the dry run.

## Fix round 1

From the review at 3482313 and the maintainer's answers of 2026-11-22. Package names and the environment `release` are settled, so their open questions are gone. Changes:
- Both file lists include `package.json`.
- The install verification gets a private `PATH` of links to `node` and `npm`, and a short `XDG_RUNTIME_DIR` of mode 0700.
- The CI Cargo cache leaves out `target/souffle-*`.
- The dispatcher is packed in its own job after the checks on both platforms.
- The dist-tag is always `next` (withdrawn by the maintainer's correction, below).
- Every push to the integration branch runs the release dry run.
- Publish and Tag are gated by the environment `release`.
- Tag can resume a partial GitHub release.
- `DES-OPERATIONS` now says "The top-level `mara --version` invocation takes no other argument." mara-e8921x follows it.
- The REQ is scoped to native platforms, and the build-image ADR is reworded.
- The registry checks run per package, and the `npx` call passes `--ignore-scripts`.
- Rejected alternatives recorded for the workflow path, publication from `main`, the release notes, the dist-tag and the pinned toolchain.
- mara-4dr2p2 now names what stays there.

## Maintainer's correction to fix round 1

- Removed every slice-only gate from both workflows: the "`next` only" rule and the refusal of versions without a prerelease part.
- Prepare still refuses `latest` for a prerelease version.
- The push dry run runs on `main` and the integration branch with no path filter. Only the branch name goes away with the branch.
- The CI item no longer refers to issues.
