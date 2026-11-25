---
title: "CI: check workflows, Soufflé builds and nightly budgets"
status: todo
type: task
tags:
  - deferred
parent: mara-qzp1gd
created_at: 2026-11-14T11:05:37Z
---

Goal: the checks, builds and measurements that the docs assign to CI run in CI.

The nightly checks and the Emscripten build of Soufflé are deferred to this issue (maintainer's decision, 2026-11-14, narrowed by the rc.1 epic): the nightly 64× run of the counter test, the `hyperfine` budgets and the Emscripten build. CI on every push and pull request and the release workflow belong to the rc.1 epic (mara-1avrq4, mara-a3aq0n). The maintainer's tooling also replaces `git-cliff` for the changelog.

Until this issue starts:

- The rc.1 epic adds CI and the release workflow (DES-CI-WORKFLOW, DES-RELEASE-WORKFLOW in docs/distribution.mara.md); no nightly workflow exists.
- Every check that an issue or a Verification item assigns to a nightly run is run locally: by the worker before it hands over its branch, and again before the branch is integrated.
- Soufflé is built natively (`scripts/build-souffle.sh`) locally, in CI and in the release workflow, and with Emscripten (`scripts/build-souffle-wasm.sh`) locally and in CI (mara-xsm8w6).
- Time budgets are not measured. REQ-TIME-BUDGETS and VER-TIME-BUDGETS stay Draft.

Tasks:

Moved to mara-1avrq4 (rc.1 epic): the workflows for format, lint, tests, the `wasm32` build of the engine crate and `mara project validate` on `docs/`, and the native Linux builds of Soufflé.
- [ ] The 64× run of the counter test (`--ignored`) nightly (VER-WORK-COUNTERS, DES-PERFORMANCE-HARNESS); the counter and scaling tests already run in CI (DES-CI-WORKFLOW)
Moved to mara-xsm8w6 (orchestrator's decision for the rc.2 slice, 2026-11-25): the pinned Soufflé built in CI with Emscripten (DES-SOUFFLE-BUILD, DES-CI-WORKFLOW).
- [ ] Nightly `hyperfine` run on a reference machine, which the maintainer names, including a read during a long validation (VER-TIME-BUDGETS; moved from mara-p27z9t)
- [ ] Budgets set from the first measurements; REQ-TIME-BUDGETS and VER-TIME-BUDGETS moved to Accepted
Moved: changelog and publishing are in mara-a3aq0n for prereleases (rc.1 epic) and in mara-chjvk9 for 0.4.0.
