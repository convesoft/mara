---
title: "Tests run with nextest and affected selection, and a full run only where it is needed: measure, then switch"
status: todo
type: task
parent: mara-c8scn7
related:
  - mara-c4y5ts
created_at: 2026-11-22T18:44:35Z
---

Goal: while working, and in pull requests, `test` runs only the tests a change can affect, with nextest as the runner; the full suite runs where it is needed and nowhere else. The switch happens only if measurements on this repository show a gain.

Context: asked for by the maintainer on 2026-11-22: "I wanted to switch to nextest and rely on affected functionality for rapid local development + also we'll need to regenerate coverage", "my point is to run full tests only when it is really necessary", and, on measuring first, "Agree better to gather numbers". nx-rust 1.0.0-rc.2 documents it in `node_modules/@convesoft/nx-rust/docs/testing.mara.md` ("Options", "Affected tests", "Why the cache stays honest") and `docs/ci.mara.md` ("The coverage baseline"). `ADR-NX-RUST-TASKS` holds the decisions on the test targets. cargo-nextest 0.9.152, cargo-affected 0.4.1 and the `llvm-tools` component were installed on the maintainer's machine on 2026-11-22, with the maintainer's permission ("Feel free to install them"); they are the versions the plugin's docs name.

Where the full suite is needed (the design to measure and then build; the maintainer also decided that `main` is fast-forwarded and that the slice after rc.1 integrates by pull requests):

- on `main` after each merge, in CI, followed by the collection of the coverage baseline (mara-azya4r);
- in the release workflow;
- wherever the plugin itself falls back to it (no usable baseline, another context, a change coverage cannot follow);
- on demand: `NX_RUST_TEST_MODE=full`.

Everywhere else, `pnpm run checks` and `pnpm run checks:fast` included, a run selects. A failure that only the full suite finds then shows on `main` after the merge. Until pull requests carry the integration, the orchestrator's check before a squash merge stays a full run.

What is known to matter here, to check first:

- Nearly every change of this project also changes a page under `docs/`, and `mara:test` declares all of `docs/` and `.mara/` as inputs: a change to a non-Rust input selects the whole suite of `mara`, so selection gains nothing until the tests are decoupled from the docs (next section)
- The baseline's record holds a hash of every environment input of the `test` tasks. `mara:test` declares `CI`, `TMPDIR` and `MARA_NO_DAEMON`, among others: a baseline collected in CI, or under another agent's `TMPDIR` (`~/.tmp/claude`, `~/.tmp/codex`), makes every affected run take the full suite
- Most integration tests of `mara` start the `mara` binary; whether selection follows code that runs in the child process decides the gain
- A collection in which a test fails writes no record; the timing tests of `tests/performance.rs` and the daemon tests run under instrumentation there
- nextest runs each test in a process of its own: fixtures shared within a test binary are built once per test
- Collection's memory: the machine has 14 GB for every agent

Why the tests read the repository's docs today, and how to decouple them (the maintainer: "Could we decouple tests from them?"):

- `crates/mara/tests/documents.rs` walks every page under `docs/` at run time and checks that the parser loses no item, also with lone-CR line endings. It is the only test that needs all of `docs/`. `mara:validate` already reads every page as a target of its own
- `crates/mara/tests/init.rs` compares what `project init` writes with files of this repository (the engineering template's source)
- Tests in seven places take documented examples from five pages with `include_str!` (`docs/format.mara.md`, `docs/rules/index.mara.md`, `docs/project.mara.md`, `docs/code/index.mara.md`, `docs/template/vocabulary.mara.md`), so that an example on a page cannot drift from the code; these pages are the crates' `fileInputs`
- Recommended, the smallest change: the corpus check of `documents.rs` leaves `mara:test` (a target of its own beside `mara:validate` with `docs/` as its input, or a fixed corpus under `tests/fixtures`), so that `mara:test` keeps only `.mara/` and what `init.rs` reads as run-time inputs and the five pages as compile-time inputs. A separate crate for every test that reads the repository's docs is the larger alternative, to choose only if the five pages change often enough to matter in the measurements

- [ ] Measured and reported, with the commands: the full suite with `cargo test` and with nextest (time, results equal: 823 passed, 14 ignored in the workspace run); a full `coverage-baseline` (time, peak memory, size of `target/affected/coverage.db`); for four representative changes (a parser function in `mara-engine`, a CLI command in `mara`, a docs page other than the five, one of the five pages) the tests selected and the time of an affected run against a full one, before and after the decoupling; `collect --diff` after each; how many of the last 30 commits on the integration branch touched `docs/`
- [ ] A recommendation with the numbers: switch, switch the runner only, or neither. If the recommendation is not "switch", the issue ends here with the findings in `ADR-NX-RUST-TASKS` and anything about the plugin reported
- [ ] If switching: the tests are decoupled from `docs/` as far as the measurements justify, without losing a check (each test that moved or changed still fails when the thing it guards breaks), and the inputs of `mara:test` shrink with them
- [ ] If switching: `testRunner = "nextest"` and `affectedTests = true` for the workspace, `llvm-tools` in `rust-toolchain.toml`, the options of `coverage-baseline` equal to those of the test runs; the release workflow and the run on `main` are in mode `full`; `pnpm run checks` and `pnpm run checks:fast` select when run on a developer's machine; the performance configuration still runs its one test target
- [ ] If switching: the environment inputs of `mara:test` are reviewed one by one, so that a baseline collected by one agent is usable by another on the same machine wherever the variable does not change what the tests do; a variable that must stay is named in the ADR with its effect on the baseline
- [ ] If switching: a script in the repository gives a fresh worktree what it needs from the checkout it was cut from: the baseline's two files (`target/affected/coverage.db` and `nx-rust-context.json`, never one without the other), the built Soufflé, and `pnpm install --frozen-lockfile`. The maintainer wires it into T3 Code's worktree setup, which runs it automatically ("there is a dedicated functionality in t3code"); the script changes nothing outside the new worktree and says what it did not find
- [ ] If switching: when the baseline is regenerated and by whom (after each integration on the integration checkout, on a clean tree; `--diff` or full, as the target decides), in `ADR-NX-RUST-TASKS` and one line of AGENTS.md "Local checks"
- [ ] If switching: no result of an affected run is ever restored for a full run (shown by a probe), and a missing tool fails with the plugin's message

Not part of this issue: the baseline that CI keeps for pull requests (the next issue of the epic), installing tools, T3 Code's or any other machine configuration.
