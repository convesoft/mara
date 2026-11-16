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
- [ ] Test: native and WASM outputs are identical for the engineering template's programs
