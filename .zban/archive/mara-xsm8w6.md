---
title: Soufflé in the browser worker
status: done
type: feature
parent: mara-xzfeyj
blocked_by:
  - mara-jptfbq
related:
  - mara-ecnpg6
created_at: 2026-11-13T22:10:06Z
closed_at: 2026-11-25T14:00:02Z
---

Context: DES-RULE-EXECUTION in docs/rules/evaluation.mara.md; DES-SOUFFLE-BUILD in docs/distribution.mara.md.

- [x] Emscripten build of the pinned Soufflé by the build script, with the documented patch, locally and in CI (moved here from mara-4dr2p2 by the orchestrator for rc.2)
- [x] Soufflé in a worker of its own; a fresh instance per evaluation on an in-memory filesystem
- [x] Tests: the browser rows of VER-FACT-ROWS, VER-EXPLANATIONS, VER-PROGRAM-FAULTS, VER-EVALUATION-TIME-LIMIT and VER-PROGRAM-REACH-CASES (docs/rules/verification.mara.md) pass (an evaluation that hits the time limit loses no unexported edit of the workspace), but for the record case below
- [x] VER-EXPLANATIONS's case of a proof through a record: Soufflé's two builds number the record differently; the browser row allows that number (mara-ecnpg6, Option A adopted provisionally by the orchestrator)
- [x] Explanations: the console input is supplied through the `stdin` hook (DES-RULE-EXECUTION, EVD-SOUFFLE-TRIAL)
- [x] Tests: the row of VER-HOST-COMPARISON (docs/verification.mara.md) for every query, view, gate and report in a browser workspace passes, the Soufflé module row of VER-UNAVAILABLE-NOT-PARTIAL and the program part of the caller-write row of VER-BROWSER-WORKSPACE (docs/hosts/verification.mara.md), and the browser workspace rows of VER-WORK-COUNTERS marked mara-xsm8w6

Context found in the review of mara-jptfbq:
- `Build::of` in `crates/mara/src/session.rs` (near line 316) calls `std::fs::metadata` on the Soufflé path, which on `wasm32` always fails, so every evaluation bypasses `Evaluated`'s cache there.
- `mara_call` of `crates/mara-wasm` is synchronous in the workspace's worker, while Soufflé must run in a worker of its own with a time limit: an evaluation needs an asynchronous hand-off between the module and `packages/mara-browser/worker.js` (a pending evaluation returned to JavaScript and its result fed back), without requiring cross-origin isolation.

## Summary of Changes

- A browser workspace evaluates programs with Soufflé's WebAssembly module, in a worker of its own (`packages/mara-browser/souffle-worker.js`) that compiles the module once and runs each evaluation in a fresh instance on an in-memory filesystem; the workspace's worker starts it and terminates it at the time limit, which counts from the run's start, the worker's start and the module's load included (DES-RULE-EXECUTION).
- The hand-off is by passes (ADR-BROWSER-EVALUATION-PASSES): Mara's module records the runs a request waits for (`crates/mara/src/rules/browser.rs`) and answers them; `worker.js` makes them and supplies what each left with the call `ran`; the module runs the request again until a pass waits for none. Reading Soufflé's outcome (`rules.rs`, `rules/explain.rs`) is shared with the native host through `Left`; the console's reply parsing is `Replies`. Explanations repeat the run with one more console command per round; a run keeps its last two rounds. A run's identity holds the project paths of its program's files, as an evaluation key does. `Build::of` identifies the module in the browser, so a session keeps its outcomes (the review of mara-jptfbq).
- Limits in the browser: memory to 1 GiB with an `OOM` abort reported as the memory limit; the output-size limit through `packages/mara-browser/output-limit.js`, which replaces `FS.write` of each instance, with a size check of each output after the run; a worker that cannot be started, a module that cannot be loaded or compiled and a failing worker are faults of the host for each run.
- `scripts/build-souffle-wasm.sh` builds the pinned Soufflé with `scripts/souffle-emscripten.patch` and the Emscripten SDK 6.0.12 into `target/souffle-2.5-wasm`, and only checks an existing module with `scripts/souffle-wasm-smoke.mjs`, which also checks that the output guard stops a run; `cargo-workspace:souffle-wasm` (uncached) runs it before `mara-browser:build`, which copies `souffle.js` and `souffle.wasm` into the package (not into its `files`; mara-865a2e packs them). CI caches the module under its own key and installs the SDK only to build it (DES-SOUFFLE-BUILD, DES-CI-WORKFLOW, ADR-NX-RUST-TASKS, AGENTS.md "Local checks"). The item "The pinned Soufflé built in CI with Emscripten" of mara-4dr2p2 is marked as moved here.
- Tests in `crates/mara/tests/hosts`: `corpus.test.mjs` (the 1× corpus row of VER-HOST-COMPARISON: every query and view of `engineering`, the gate and the report), `programs.test.mjs` (VER-FACT-ROWS, VER-EXPLANATIONS with validation and the gate before and after each report, VER-PROGRAM-FAULTS, the arithmetic of VER-PROGRAM-REACH-CASES, two declared programs alike counted as two runs), `limits.test.mjs` (VER-EVALUATION-TIME-LIMIT with unexported edits kept, an aborting module, a module load that does not end, a Soufflé worker that cannot be started), and new cases in `unavailable.test.mjs` (the Soufflé module row with 404 and a truncated module; every operation that reads code only for a program), `workspace.test.mjs` (the program part of the caller-write row) and `work_counters.test.mjs` (the browser rows marked mara-xsm8w6). Every expected value comes from CLI direct mode with the native binary; 39 Node cases in Chromium. A unit test checks that a run keeps at most two rounds.
- Docs: DES-RULE-EXECUTION, ADR-BROWSER-EVALUATION-PASSES, EVD-SOUFFLE-TRIAL (the build's numbers), EVD-SOUFFLE-PROGRAM-REACH (the record number), VER-EXPLANATIONS's browser row, VER-WORK-COUNTERS's browser rows, DES-SOUFFLE-BUILD, DES-CI-WORKFLOW, ADR-NX-RUST-TASKS.
- Checks: `pnpm run checks` with a fresh Nx cache, after merging `origin/main` with #88 and #90, as reported in the pull request (#89). Probes: a change to the built `souffle.js` reran `mara-browser:build` and `mara:test --test=hosts`, reverting restored both; without the module and the SDK the script fails naming the SDK and its version, a wrong `emcc` is refused; the host tests fail naming the script without the package's `souffle.wasm`; the smoke run fails when `output-limit.js` lets a write pass. CI run 45623248178 passed on both images; dry dispatch 45623254528 passed. Emscripten build: 64 s here with 8 jobs, 6.2 MB (1.2 MB gzip); in CI the SDK installs in 20–25 s and the cold build adds at most 5.3 min (x64) and 3.2 min (arm64) to the checks.
- Fix round 1 (review of 378c2a0): a run keeps its last two rounds; Soufflé's worker start and module load count toward the time limit, and a worker that cannot be started is a fault of the run; the output guard moved to `output-limit.js`, which the smoke run checks, with a size check after the run; a run's identity holds its program's project paths; the grammar row adds `query view`, `query gate` and `item validate`; each explanation case checks validation and the gate before and after the report, and the symbol comparison has no constraint leaf; wording of DES-RULE-EXECUTION and DES-SOUFFLE-BUILD.

## Decisions made

- The hand-off by passes, with explanations by rounds (ADR-BROWSER-EVALUATION-PASSES); rejected: JSPI or Asyncify, `Atomics.wait`, an asynchronous library, the console in Soufflé's worker.
- An explanation's rounds each run within the time limit; a round stopped at it falls back to the round before, which ends the exchange as at the time limit natively (DES-RULE-EXECUTION).
- In the browser a program run counts no subprocess spawn; the result's stats count every pass (DES-RULE-EXECUTION).
- The Emscripten SDK 6.0.12, the trial's version; flags `-sEXIT_RUNTIME`, memory to 1 GiB with `-sABORTING_MALLOC`, an 8 MiB stack with `-sSTACK_OVERFLOW_CHECK=1`, no C++ exception handling (DES-SOUFFLE-BUILD); the script checks an existing module with Node alone, so a cache hit needs no SDK.
- CI installs the SDK in `ci.yml`, only when the module's entry was not restored, from emsdk's archive pinned by SHA-256, with only `upstream/emscripten` on `PATH` (DES-CI-WORKFLOW).
- `souffle-worker.js` and `output-limit.js` are in the browser package's `files`; Soufflé's `souffle.js` and `souffle.wasm` are not, until mara-865a2e.
- Decided by the orchestrator in the review: the output-size limit stays enforced by replacing `FS.write` of each instance, documented, checked by the smoke run and backed by a size check after the run (DES-RULE-EXECUTION, DES-SOUFFLE-BUILD).
- Decided by the orchestrator, provisionally for the maintainer (mara-ecnpg6, Option A): Soufflé's two builds number a record in a proof differently; VER-EXPLANATIONS's browser row allows that number and EVD-SOUFFLE-PROGRAM-REACH has the row.
- Confirmed by the orchestrator: VER-WORK-COUNTERS's browser rows were corrected to what DES-RULE-EXECUTION gives the `engineering` corpus: a query of a program that an invariant uses runs nothing after `project validate`, a view of `readiness.dl` runs once; the program written is one that no other program includes.
