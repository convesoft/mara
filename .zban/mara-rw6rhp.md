---
title: Tests run with nextest
status: todo
type: task
parent: mara-c8scn7
related:
  - mara-7wysb0
discovered_from: mara-7wysb0
created_at: 2026-11-22T21:55:41Z
---

Goal: every `test` task runs its crate's tests with cargo-nextest (`testRunner = "nextest"`), which ran the full suite in 84 s against 159 s with `cargo test` on the maintainer's machine, with equal results (mara-7wysb0, `ADR-NX-RUST-TASKS`). `affectedTests` stays off: selection gained nothing on this repository.

Context: nx-rust 1.0.0-rc.2 documents the runner in `node_modules/@convesoft/nx-rust/docs/testing.mara.md` ("Options", "One test target") and `DES-TEST-RUN` there. cargo-nextest 0.9.152 is installed on the maintainer's machine; CI (`DES-CI-WORKFLOW`) and the release (`DES-RELEASE-WORKFLOW`) run `pnpm run checks` and need it installed (a pinned version, as the other tools there are).

What mara-7wysb0 found, to carry over:

- `crates/mara-engine/tests/diagnostic_memory.rs` has `harness = false` and ignores its arguments, so nextest exits with 104 ("creating test list failed ... did not end with the string `: test`"). Its `main` must answer libtest's `--list --format terse` with `diagnostic_memory: test`, print nothing when `--ignored` is also given, and run on any other arguments (https://nexte.st/docs/design/custom-test-harnesses/). With that change the run passed: 825 passed, 14 skipped.
- nextest counts 825 tests where cargo's `test result:` lines count 824: cargo prints no such line for the `harness = false` binary. The 14 ignored are the timing tests of `tests/performance.rs` that a debug build ignores.
- Output of a full run with nextest: each task of the `nx` command prints the same line for the whole run, `     Summary [  81.945s] 825 tests run: 825 passed (1 slow), 14 skipped`, then `<project>:test: nextest ran <n> tests of <project>, as its JUnit report shows.`, then the doctest step's `test result: ok. 0 passed; ...` line (no crate has doctests). Require the Nx command to exit 0. Read counts from one workspace `Summary` line and count the separate performance run once. `mara:test:performance` (a `test` option) runs `cargo nextest run --no-tests=pass -p <spec> --test performance` with the release profile (`DES-TEST-RUN`, "One test target") and prints its own `Summary` line.
- One test exceeds nextest's 60 s slow mark in debug: `mara::performance work_counters::cold_counts`; it is the critical path of the run.

- [ ] `testRunner = "nextest"` for the workspace; `diagnostic_memory` answers `--list`
- [ ] `pnpm run checks` passes; `mara:test:performance` still runs its one test target with the release profile, alone and never restored
- [ ] `ADR-NX-RUST-TASKS` (the `test` and `mara:test:performance` rows), AGENTS.md "Local checks" and the CI and release Design items name nextest where they name the commands or tools
- [ ] The pinned nextest install goes into the CI toolchain step that the release workflow already references (`DES-CI-WORKFLOW`, `DES-RELEASE-WORKFLOW`)
- [ ] The orchestrator's gate before a squash merge reads nextest's `Summary` line; the closing summary gives a sample of a full run's output

Not part of this issue: `affectedTests` and the coverage baseline (`ADR-NX-RUST-TASKS`, mara-7wysb0).
