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

Everything connected to CI is deferred to this issue on purpose (maintainer's decision, 2026-11-14): the maintainer is building dedicated tooling for CI and publishing, and it is not ready. That tooling also replaces `git-cliff` for the changelog.

Until this issue starts:

- No workflow file is added, and no issue waits for a CI run.
- Every check that an issue or a Verification item assigns to CI, to "every pull request" or to a nightly run is run locally: by the worker before it hands over its branch, and again before the branch is integrated.
- Soufflé is built by a local script, for the local platform only.
- Time budgets are not measured. REQ-TIME-BUDGETS and VER-TIME-BUDGETS stay Draft.

Tasks:

- [ ] Workflows for format, lint, tests, the `wasm32` build of the engine crate and `mara project validate` on `docs/` (moved from mara-0ztabc, mara-csmk0c and mara-37qtf2)
- [ ] Counter and scaling tests on every pull request, 64× nightly (VER-WORK-COUNTERS, VER-SCALING, DES-PERFORMANCE-HARNESS)
- [ ] The pinned Soufflé built in CI for Linux x64 and arm64, natively and with Emscripten (DES-SOUFFLE-BUILD; moved from mara-vt2ktq and mara-xsm8w6)
- [ ] Nightly `hyperfine` run on a reference machine, which the maintainer names, including a read during a long validation (VER-TIME-BUDGETS; moved from mara-p27z9t)
- [ ] Budgets set from the first measurements; REQ-TIME-BUDGETS and VER-TIME-BUDGETS moved to Accepted
- [ ] Changelog and publishing through the maintainer's release tooling
