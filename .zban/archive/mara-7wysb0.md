---
title: "Tests run with nextest and affected selection, and a full run only where it is needed: measure, then switch"
status: done
type: task
parent: mara-c8scn7
blocked_by:
  - mara-8mxw9c
related:
  - mara-c4y5ts
created_at: 2026-11-22T18:44:35Z
closed_at: 2026-11-22T22:02:20Z
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

- Nearly every change of this project also changes a page under `docs/`, and `mara:test` declares all of `docs/` and `.mara/` as inputs: a change to a non-Rust input selects the whole suite of `mara`, so selection gains nothing until mara-8mxw9c is done
- The baseline's record holds a hash of every environment input of the `test` tasks. `mara:test` declares `CI`, `TMPDIR` and `MARA_NO_DAEMON`, among others: a baseline collected in CI, or under another agent's `TMPDIR` (`~/.tmp/claude`, `~/.tmp/codex`), makes every affected run take the full suite
- Most integration tests of `mara` start the `mara` binary; whether selection follows code that runs in the child process decides the gain
- A collection in which a test fails writes no record; the timing tests of `tests/performance.rs` and the daemon tests run under instrumentation there
- nextest runs each test in a process of its own: fixtures shared within a test binary are built once per test
- Collection's memory: the machine has 14 GB for every agent

The tests that read the repository's docs leave `mara:test` first: mara-8mxw9c, which blocks this issue (the maintainer: they "are not source code tests").

- [x] Measured and reported, with the commands: the full suite with `cargo test` and with nextest (time, results equal: 824 Cargo-reported passes plus `diagnostic_memory`, with 14 ignored at `5a5a6b7`; at `dcc7297` it is 828 Cargo-reported passes and 23 files that clear the child environment, since `12fcd1a` added four tests and one such file); a full `coverage-baseline` (time, peak memory, size of `target/affected/coverage.db`); for four representative changes (a parser function in `mara-engine`, a CLI command in `mara`, a docs page other than the five, one of the five pages) the tests selected and the time of an affected run against a full one; `collect --diff` after each; how many of the last 30 commits on the integration branch touched `docs/`
- [x] A recommendation with the numbers: switch, switch the runner only, or neither. If the recommendation is not "switch", the issue ends here with the findings in `ADR-NX-RUST-TASKS` and anything about the plugin reported
- [x] Not applicable, no switch. If switching: `testRunner = "nextest"` and `affectedTests = true` for the workspace, `llvm-tools` in `rust-toolchain.toml`, the options of `coverage-baseline` equal to those of the test runs; the release workflow and the run on `main` are in mode `full`; `pnpm run checks` and `pnpm run checks:fast` select when run on a developer's machine; the performance configuration still runs its one test target
- [x] Not applicable, no switch. If switching: the environment inputs of `mara:test` are reviewed one by one, so that a baseline collected by one agent is usable by another on the same machine wherever the variable does not change what the tests do; a variable that must stay is named in the ADR with its effect on the baseline
- [x] Not applicable, no switch. If switching: a script in the repository gives a fresh worktree what it needs from the checkout it was cut from: the baseline's two files (`target/affected/coverage.db` and `nx-rust-context.json`, never one without the other), the built Soufflé, and `pnpm install --frozen-lockfile`. The maintainer wires it into T3 Code's worktree setup, which runs it automatically ("there is a dedicated functionality in t3code"); the script changes nothing outside the new worktree and says what it did not find
- [x] Not applicable, no switch. If switching: when the baseline is regenerated and by whom (after each integration on the integration checkout, on a clean tree; `--diff` or full, as the target decides), in `ADR-NX-RUST-TASKS` and one line of AGENTS.md "Local checks"
- [x] Not applicable, no switch. If switching: no result of an affected run is ever restored for a full run (shown by a probe), and a missing tool fails with the plugin's message

Not part of this issue: the baseline that CI keeps for pull requests (the next issue of the epic), installing tools, T3 Code's or any other machine configuration.

## Summary of Changes

Recommendation: **switch the runner only**. nextest halves the full suite; selection with `affectedTests` gains nothing on this repository, and its coverage baseline cannot be collected here. The findings are in `ADR-NX-RUST-TASKS`; the runner switch is mara-rw6rhp, and the collection blockers are mara-vxeweg (backlog). No code or configuration changed in this branch.

Measured on 2026-11-22 on the maintainer's machine (32 threads, 14 GB shared with other agents), at 5a5a6b7 with temporary scratch commits since removed: cargo-nextest 0.9.152, cargo-affected 0.4.1, `llvm-tools` of 1.100.0. GNU `time` is not installed (`/usr/bin/time` is missing), so each command ran inside a transient cgroup scope, `systemd-run --user --scope --quiet -p MemoryAccounting=yes bash -c '<script>' measure <command...>`, whose script read `date +%s.%N` before and after the command for the wall time and then `cat /sys/fs/cgroup$(cut -d: -f3 /proc/self/cgroup)/memory.peak` for the peak memory of the scope: the whole process tree, page cache included. Each test run was warmed first (`cargo test`/`cargo nextest run --workspace --no-run --locked`) and ran through `berth`; collections ran under `berth --exclusive`. All `nx` commands ran with `NX_DAEMON=false NX_USE_LOCAL=true`.

### Full suite

| Runner | Command | Wall | Peak | Result |
| --- | --- | --- | --- | --- |
| `cargo` (today) | `pnpm nx run-many -t test --skip-nx-cache --output-style=static` | 158.7 s | 2.44 GiB | 824 passed, 0 failed, 14 ignored (sum of `test result:` lines) |
| `nextest` (`testRunner = "nextest"` in `[workspace.metadata.nx]`) | same | 83.7 s | 2.27 GiB | `825 tests run: 825 passed (1 slow), 14 skipped` |

- Results are equal: the 825th is `mara-engine`'s `harness = false` binary `diagnostic_memory`, for which cargo prints no `test result:` line. The 14 are the timing tests of `tests/performance.rs` that a debug build ignores.
- nextest first failed with status 104: `diagnostic_memory` ignores its arguments and does not answer `--list` ("did not end with the string `: test`"). A 10-line `main` that answers libtest's `--list --format terse` fixed it; it belongs to mara-rw6rhp.
- The nextest run's critical path is one test, `mara::performance work_counters::cold_counts`, flagged `SLOW [> 60.000s]`.
- The known flaky test (`client::after_a_daemon_is_killed_...`, mara-g663ez) passed in every run here (2 full nextest runs, 8 `nx affected` runs, 4 collections).

### Coverage baseline (`nx run cargo-workspace:coverage-baseline`, `testRunner = "nextest"`, `affectedTests = true`, `llvm-tools` in `rust-toolchain.toml`)

- At `5a5a6b7`, with the nextest listing fix: the run took 92.8 s at a peak of 8.60 GiB, then **Nx crashed** with `RangeError: Invalid string length` in `node-child-process.js` while joining the task's output. Run directly (`cargo affected collect -- --workspace --no-fail-fast --locked`): 57.1 s, peak 6.34 GiB, exit 100, **494 of 825 tests failed**, and 1.6 GB of standard error, 33 assertion lines holding file contents. Two causes:
  - 22 test files start `mara` with `env_clear()`, so the instrumented binary loses `LLVM_PROFILE_FILE` and writes `default_<m>_<p>.profraw` into the test project, which breaks assertions that compare the whole project tree.
  - `mara` finds Soufflé beside its binary or at `../souffle-2.5/souffle` (`DES-SOUFFLE-BUILD`); cargo-affected's binary lives in `target/affected/build/debug/`, where neither exists.
- With both worked around on a scratch commit (forwarding `LLVM_PROFILE_FILE` in the 22 files; a link `target/affected/build/souffle-2.5 -> ../../souffle-2.5`), run directly: 168.7 s, peak 10.20 GiB, 824 passed and 1 failed: `programs::faults::a_program_past_the_memory_limit_is_stopped_before_the_time_limit` hit the 10 s time limit before the memory limit under instrumentation. 6 tests that interrupt or kill `mara` leave corrupt profiles ("file header is corrupt") and produce no coverage.
- With that test also left out by a scratch `.config/nextest.toml` `default-filter`, through the plugin target: **156.3 s, peak 10.06 GiB, `coverage.db` 156 MB, `target/affected/build` 7.7 GB**, record written ("collects the coverage of every test into an empty database ... because the baseline has no record").
- Daemon tests under instrumentation: they pass in a full collection, but in the `collect --diff` of change 1, `daemon::caller::stop_before_a_report_creates_its_directory_leaves_none` failed ("the daemon outlived `stop`"), so that collection left no record.

### Four representative changes

Each change was committed on top of the collected commit; then `NX_RUST_TEST_MODE=full|affected berth -- pnpm nx affected -t test --base=<collected> --head=HEAD --skip-nx-cache --output-style=static`, then `pnpm nx run cargo-workspace:coverage-baseline` (the target chose `collect --diff` each time), with the database and record restored between changes.

| Change | Full | Affected | Selected (plugin's lines) | `collect --diff` |
| --- | --- | --- | --- | --- |
| 1. `is_item_table` in `mara-engine/src/document/region.rs` | 84.0 s, 824 tests | 83.6 s, 631 tests | `mara-engine`: 90 of 222; `mara`: 338 selected "and every integration test of mara, because ... is an input of mara, whose integration tests can start its binary" (534 run); `docs-checks`: all 7, same reason | 142.1 s, peak 10.01 GiB, **failed** (the daemon test above) |
| 2. `schema_validate` in `mara/src/cli/validate.rs` | 83.3 s, 824 | 83.8 s, 543 | `mara-engine`: none; `mara`: 9 and every integration test (536 run); `docs-checks`: all 7 | 27.5 s, peak 8.40 GiB, 8 tests, passed |
| 3. `docs/identity.mara.md` (not one of the five) | 82.2 s, 602 | 12.1 s, 13 | `docs-checks`: all 7 ("no Rust source"); `mara`: 7 by coverage (6 run) | 19.9 s, **failed**: "coverage extraction yielded no ranges for any of the 6 completed tests ... refusing to overwrite stored coverage" |
| 4. `docs/project.mara.md` (one of the five) | 83.0 s, 602 | 12.7 s, 13 | same as 3 | 18.8 s, **failed**, same message |

- For changes 3 and 4, `nx affected` picks `mara:test` by the project graph, and the uncached full runs above ran it. mara-8mxw9c showed that a docs-only edit reruns only `docs-checks:test` with warm test results. The isolated, uncached `docs-checks:test` took 1.8 s (`NX_RUST_TEST_MODE=full pnpm nx run docs-checks:test --skip-nx-cache`). Selection thus saves nothing over a warm cache for a page edit, and nothing for a Rust change, because the 60 s `cold_counts` is an integration test of `mara` and always runs.
- Last 30 commits of `chore/clear-worktree-for-rewrite`, from `git log -30 db12a91 --format=%h --name-only` grouped by commit: 8 touched `docs/`, 2 of them also a `.rs` file; 22 touched neither (mostly `.zban/`).

### Checks

- `berth --exclusive -- pnpm run checks` (after merging db12a91): exit 0; 860 passed, 0 failed, 16 ignored by the sum of `test result:` lines.
- `cargo run --quiet --bin mara -- --no-daemon project validate`: valid, 0 errors, 0 warnings.

## Decisions made

- `affectedTests` stays off and each uncached `test` task runs its full suite everywhere, in `ADR-NX-RUST-TASKS`, with the measurements as the rejected alternative. The issue ends here, as its second item says; mara-azya4r, which depended on "switch", is cancelled.
- The runner switch is a follow-up (mara-rw6rhp), not part of this branch: the issue ends at a recommendation other than "switch".
- Peak memory came from a cgroup's `memory.peak`, because GNU `time` is not installed.
