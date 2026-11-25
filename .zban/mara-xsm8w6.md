---
title: Soufflé in the browser worker
status: todo
type: feature
parent: mara-xzfeyj
blocked_by:
  - mara-jptfbq
created_at: 2026-11-13T22:10:06Z
---

Context: DES-RULE-EXECUTION in docs/rules-evaluation.mara.md; DES-SOUFFLE-BUILD in docs/distribution.mara.md.

- [ ] Emscripten build of the pinned Soufflé by the build script, with the documented patch (CI waits for mara-4dr2p2)
- [ ] Soufflé in a worker of its own; a fresh instance per evaluation on an in-memory filesystem
- [ ] Tests: the browser rows of VER-FACT-ROWS, VER-EXPLANATIONS, VER-PROGRAM-FAULTS, VER-EVALUATION-TIME-LIMIT and VER-PROGRAM-REACH-CASES pass (an evaluation that hits the time limit loses no unexported edit of the workspace)
- [ ] Explanations: the console input is supplied through the `stdin` hook (DES-RULE-EXECUTION, EVD-SOUFFLE-TRIAL)
- [ ] Tests: the row of VER-HOST-COMPARISON (docs/verification.mara.md) for every query, view, gate and report in a browser workspace passes, the Soufflé module row of VER-UNAVAILABLE-NOT-PARTIAL and the program part of the caller-write row of VER-BROWSER-WORKSPACE (docs/verification-browser.mara.md), and the browser workspace rows of VER-WORK-COUNTERS marked mara-xsm8w6

Context found in the review of mara-jptfbq:
- `Build::of` in `crates/mara/src/session.rs` (near line 316) calls `std::fs::metadata` on the Soufflé path, which on `wasm32` always fails, so every evaluation bypasses `Evaluated`'s cache there.
- `mara_call` of `crates/mara-wasm` is synchronous in the workspace's worker, while Soufflé must run in a worker of its own with a time limit: an evaluation needs an asynchronous hand-off between the module and `packages/mara-browser/worker.js` (a pending evaluation returned to JavaScript and its result fed back), without requiring cross-origin isolation.
