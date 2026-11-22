---
title: "CI: GitHub Actions runs the local checks on Linux x64 and arm64"
status: done
type: task
parent: mara-c8scn7
blocked_by:
  - mara-a57pzn
related:
  - mara-4dr2p2
created_at: 2026-11-22T18:44:35Z
closed_at: 2026-11-22T21:09:57Z
---

Goal: a push to the integration branch or to `main`, and a pull request, run the local checks in GitHub Actions on Linux x64 and arm64, and a failure there is a failure a developer can reproduce with `pnpm run checks`.

Context: the Design item for CI that the spec issue of this epic adds (docs/distribution.mara.md or where it puts it), `ADR-NX-RUST-TASKS` (the checks are Nx targets), `DES-SOUFFLE-BUILD`. Reference: zban's `.github/workflows/ci.yml` and nx-rust's workflows (read-only). The first item of mara-4dr2p2 and the native Linux part of its Soufflé item moved here.

- [x] A workflow runs the checks through Nx (`pnpm install --frozen-lockfile`, then the targets of `pnpm run checks` that the Design item names) on `ubuntu-24.04` and `ubuntu-24.04-arm`, with the toolchain that `rust-toolchain.toml` pins and the pnpm and Node versions the repository pins
- [x] The pinned Soufflé is built by `scripts/build-souffle.sh` in the job and kept in the CI cache under a key of the script and the platform, so that a run with an unchanged script does not rebuild it
- [x] `mara:validate` has the pinned grammar it needs: the job obtains it as AGENTS.md says a machine does (`code add rust`), within what `DES-GRAMMAR-ASSETS` allows
- [x] Cargo's build directory is cached by the CI system and Nx's results are not shared between runs unless the Design item says so; `NX_DAEMON=false`; locked mode stays on
- [x] The workflow is shown to pass on both platforms by a run on a pushed branch; the run's link and duration are in the closing summary. A test that fails only in CI is fixed here if the cause is the test's assumption about the machine, and filed otherwise
- [x] No job publishes, tags or writes to the repository; permissions are read-only

Not part of this issue: the release workflow, the nightly runs and budgets (mara-4dr2p2), macOS (mara-ag4ees), the Emscripten build of Soufflé (mara-4dr2p2).

## Summary of Changes

- `.github/workflows/ci.yml`: on each push to `chore/clear-worktree-for-rewrite` and `main` and on each pull request, the jobs `Checks (ubuntu-24.04)` and `Checks (ubuntu-24.04-arm)` (`fail-fast: false`, `contents: read` at workflow and job level, 60-minute timeout) run, in order: checkout with full history and tags; `rustup toolchain install`; pnpm of `packageManager` and Node 24; `pnpm install --frozen-lockfile`; restore `target/souffle-2.5` (key: image, arch, SHA-256 of `scripts/build-souffle.sh`, no fallback); `actions/cache` of `target` without `target/souffle-*` plus Cargo's registry (key: image, arch, `rust-toolchain.toml`, `Cargo.lock`; fallback: newest of the same image and arch); `cargo run --locked --quiet --bin mara -- --no-daemon code add rust`; `pnpm run checks` with `NX_DAEMON=false`; save Soufflé, also after a failed check; `git status --porcelain` must print nothing. Nx's cache is never saved or restored. Actions are pinned by major version, as in the references (checkout@v7, pnpm/action-setup@v6, setup-node@v7, cache@v6).
- `@convesoft/zban` 1.0.0-rc.1 is an exact devDependency (`package.json`, `pnpm-lock.yaml`). pnpm's minimum release age refused it, so `minimumReleaseAgeExclude` now lists that exact version and its four native packages.
- `cargo-workspace:zban-check` (root `Cargo.toml`): the key now looks up `zban` with `node_modules/.bin` first, as Nx's command does, so the key stays true when `nx` runs without `pnpm`, which leaves `node_modules/.bin` off the hasher's PATH. Probe on a private `NX_CACHE_DIRECTORY`, with `node_modules/.bin/nx` started directly: run, then restored; a changed shim ran again; the reverted shim was restored.
- Docs: `DES-CI-WORKFLOW` (no build tools to install, the zban devDependency and its release-age exemption, the new "Soufflé saved" step), `ADR-CI-CHECKS` (the worker's choices below), the `zban-check` row of `ADR-NX-RUST-TASKS`.

Evidence:
- Local `pnpm run checks` (berth --exclusive) at 1ceff82 plus this change: exit 0, 855 passed, 0 failed, 16 ignored, validate 0 errors, 0 warnings. After merging 5a5a6b7: exit 0, 856 passed, 0 failed, 16 ignored, validate 0 errors, 0 warnings.
- CI, cold caches: https://github.com/convesoft/mara/actions/runs/45066263079, 17m49s, both images green, 855 passed, 0 failed, 16 ignored each; Soufflé built and saved on each.
- CI, warm caches and merged tip: https://github.com/convesoft/mara/actions/runs/45068832866, 11m0s, both images green, 856 passed, 0 failed, 16 ignored each; Soufflé restored, so `build-souffle.sh` only checked it; Cargo restored from the fallback key and saved under the new `Cargo.lock` key. Performance test: 1m43s on x64, 2m2s on arm64. No test failed only in CI. `mara-g663ez`'s flaky test did not fail.
- Cache entries: Soufflé 3 MiB, Cargo 1.3 GiB (arm64) and 1.5 GiB (x64).

## Decisions made

- No step installs build tools: both images have curl, tar, sha256sum, cmake, c++, flex, bison, readelf and ninja (checked in the first run). Recorded in `DES-CI-WORKFLOW`.
- Soufflé's entry is saved by `actions/cache/restore` + `actions/cache/save` after the checks, also when a check failed. The cache action's own post-job save would skip a failed job, so the next run would build Soufflé again. Recorded in `ADR-CI-CHECKS` and `DES-CI-WORKFLOW`.
- Each push run gets its own concurrency group (`github.run_id`); a pull request's runs share `github.ref` and cancel older runs. With one group per ref, a newer push run would cancel an older one that is still pending. Recorded in `ADR-CI-CHECKS`.
- Actions are pinned by major version, as zban's and nx-rust's workflows are; neither pins by SHA. Recorded in `ADR-CI-CHECKS`.
- Job timeout 60 minutes, over three times a cold run. Recorded in `ADR-CI-CHECKS`.
- The `zban-check` key looks up `zban` with `node_modules/.bin` first. Recorded in the root `Cargo.toml` comment, `DES-CI-WORKFLOW` and `ADR-NX-RUST-TASKS`.
- The pnpm store is not cached: `DES-CI-WORKFLOW` names only the Soufflé and Cargo caches, and the install takes seconds.
