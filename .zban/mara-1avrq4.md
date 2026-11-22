---
title: "CI: GitHub Actions runs the local checks on Linux x64 and arm64"
status: todo
type: task
parent: mara-c8scn7
blocked_by:
  - mara-a57pzn
related:
  - mara-4dr2p2
created_at: 2026-11-22T18:44:35Z
---

Goal: a push to the integration branch or to `main`, and a pull request, run the local checks in GitHub Actions on Linux x64 and arm64, and a failure there is a failure a developer can reproduce with `pnpm run checks`.

Context: the Design item for CI that the spec issue of this epic adds (docs/distribution.mara.md or where it puts it), `ADR-NX-RUST-TASKS` (the checks are Nx targets), `DES-SOUFFLE-BUILD`. Reference: zban's `.github/workflows/ci.yml` and nx-rust's workflows (read-only). The first item of mara-4dr2p2 and the native Linux part of its Soufflé item moved here.

- [ ] A workflow runs the checks through Nx (`pnpm install --frozen-lockfile`, then the targets of `pnpm run checks` that the Design item names) on `ubuntu-24.04` and `ubuntu-24.04-arm`, with the toolchain that `rust-toolchain.toml` pins and the pnpm and Node versions the repository pins
- [ ] The pinned Soufflé is built by `scripts/build-souffle.sh` in the job and kept in the CI cache under a key of the script and the platform, so that a run with an unchanged script does not rebuild it
- [ ] `mara:validate` has the pinned grammar it needs: the job obtains it as AGENTS.md says a machine does (`code add rust`), within what `DES-GRAMMAR-ASSETS` allows
- [ ] Cargo's build directory is cached by the CI system and Nx's results are not shared between runs unless the Design item says so; `NX_DAEMON=false`; locked mode stays on
- [ ] The workflow is shown to pass on both platforms by a run on a pushed branch; the run's link and duration are in the closing summary. A test that fails only in CI is fixed here if the cause is the test's assumption about the machine, and filed otherwise
- [ ] No job publishes, tags or writes to the repository; permissions are read-only

Not part of this issue: the release workflow, the nightly runs and budgets (mara-4dr2p2), macOS (mara-ag4ees), the Emscripten build of Soufflé (mara-4dr2p2).
