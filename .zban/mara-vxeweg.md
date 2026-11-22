---
title: Coverage collection passes on this repository's tests
status: todo
type: task
priority: deferred
tags:
  - deferred
  - backlog
discovered_from: mara-7wysb0
created_at: 2026-11-22T21:55:57Z
---

Goal: `cargo affected collect -- --workspace --no-fail-fast` (nx-rust's `coverage-baseline`) passes on this repository, so that `affectedTests` could be measured again. Only worth doing if selection could gain: in mara-7wysb0 it did not, because a change to `mara` or `mara-engine` runs every integration test of `mara`, because those tests can start its binary (`ADR-NX-RUST-TASKS`).

Context: measured in mara-7wysb0 on 2026-11-22 with cargo-affected 0.4.1, nextest 0.9.152 and nx-rust 1.0.0-rc.2. The observations below include collection failures and skipped profiles. A collection with no usable ranges fails, and a failed collection leaves no baseline record, so every affected run takes the full suite.

- [ ] 494 of 825 tests fail: 22 test files start `mara` with `env_clear()`, so the instrumented binary loses cargo-affected's `LLVM_PROFILE_FILE` and writes `default_<m>_<p>.profraw` into the test project, which breaks assertions that compare the whole project tree. Forwarding `LLVM_PROFILE_FILE` where it is set fixed them in an experiment, and attributed the child's coverage to the test (222,179 ranges against 58,233).
- [ ] `mara` finds Soufflé beside its binary or at `../souffle-2.5/souffle` from it (`DES-SOUFFLE-BUILD`); cargo-affected builds into `target/affected/build/`, where neither exists, so every evaluation in a test fails. A link `target/affected/build/souffle-2.5 -> ../../souffle-2.5` fixed it in the experiment; a lasting answer must not contradict `DES-SOUFFLE-BUILD`.
- [ ] Under instrumentation and collection's parallelism, `programs::faults::a_program_past_the_memory_limit_is_stopped_before_the_time_limit` hit the time limit first, and `daemon::caller::stop_before_a_report_creates_its_directory_leaves_none` failed with "the daemon outlived `stop`" in a `collect --diff`.
- [ ] Six tests that interrupt or kill `mara` leave truncated profiles ("file header is corrupt"); cargo-affected skips them in a full collection, and a `collect --diff` that re-collects only them fails with "coverage extraction yielded no ranges ... refusing to overwrite stored coverage", as after a page edit.
- [ ] The failing collection's 1.6 GB of test output (assertions that print file contents) made Nx 24.0.6 crash with `RangeError: Invalid string length` instead of failing the task.

Cost of a collection that passed with the experiment: 156 s, a cgroup peak of 10.06 GiB (page cache included) on a 14 GB machine, a 156 MB `coverage.db` and 7.7 GB of instrumented build in `target/affected/build`.
