---
title: "Tests run with nextest and affected selection while working: measure, then switch"
status: todo
type: task
parent: mara-c8scn7
related:
  - mara-c4y5ts
created_at: 2026-11-22T18:44:35Z
---

Goal: `pnpm run checks:fast` runs only the tests a change can affect, with nextest as the runner, if measurements on this repository show a gain; `pnpm run checks` still runs the full suite.

Context: asked for by the maintainer on 2026-11-22: "I wanted to switch to nextest and rely on affected functionality for rapid local development + also we'll need to regenerate coverage". nx-rust 1.0.0-rc.2 documents it in `node_modules/@convesoft/nx-rust/docs/testing.mara.md` ("Options", "Affected tests", "Why the cache stays honest") and `docs/ci.mara.md` ("The coverage baseline"), which says to measure before switching it on. `ADR-NX-RUST-TASKS` holds the decisions on the test targets. The tools (cargo-nextest, cargo-affected, the `llvm-tools` component) are the maintainer's to install on the machine; for the measurements use a private `CARGO_HOME` under `$TMPDIR`.

What is known to matter here, to check first:

- The baseline's record holds a hash of every environment input of the `test` tasks. `mara:test` declares `CI`, `TMPDIR` and `MARA_NO_DAEMON`, among others: a baseline collected in CI, or under another agent's `TMPDIR`, makes every affected run take the full suite
- A change to a non-Rust input runs all tests of the crate, and `mara:test` declares `docs/` and `.mara/`: every docs change selects the whole suite of `mara`
- Most integration tests of `mara` start the `mara` binary; whether selection follows code that runs in the child process decides the gain
- A collection in which a test fails writes no record; the timing tests of `tests/performance.rs` and the daemon tests run under instrumentation there
- Each worktree has its own `target/`, so a fresh worktree has no baseline
- nextest runs each test in a process of its own: fixtures shared within a test binary are built once per test
- Collection's memory: the machine has 14 GB for every agent

- [ ] Measured and reported, with the commands: the full suite with `cargo test` and with nextest (time, results equal: 823 passed, 14 ignored in the workspace run); a full `coverage-baseline` (time, peak memory, size of `target/affected/coverage.db`); for three representative changes (a parser function in `mara-engine`, a CLI command in `mara`, a docs page) the tests selected and the time of an affected run against a full one; `collect --diff` after each
- [ ] A recommendation with the numbers: switch, switch the runner only, or neither. If the recommendation is not "switch", the issue ends here with the findings in `ADR-NX-RUST-TASKS` and anything about the plugin reported
- [ ] If switching: `testRunner = "nextest"` and `affectedTests = true` for the workspace, `llvm-tools` in `rust-toolchain.toml`, the options of `coverage-baseline` equal to those of the test runs; `pnpm run checks` and the release and CI workflows run in mode `full`, and only `checks:fast` selects; the performance configuration still runs the one test target
- [ ] If switching: how a developer and a fresh worktree get a baseline (collected on a clean tree at the integration tip; whether the database and the record may be copied into a new worktree), when it is regenerated, and what the environment inputs must be for it to be used, in `ADR-NX-RUST-TASKS` and one line of AGENTS.md "Local checks"
- [ ] If switching: no result of an affected run is ever restored for a full run (shown by a probe), and a missing tool fails with the plugin's message

Not part of this issue: a baseline kept by CI for pull requests (the project integrates by local squash; file it if the measurements show a use), installing tools on the machine.
