---
title: CI keeps the coverage baseline on main, and pull requests run the affected tests
status: todo
type: task
parent: mara-c8scn7
blocked_by:
  - mara-7wysb0
  - mara-1avrq4
created_at: 2026-11-22T18:57:11Z
---

Goal: a pull request's CI run selects its tests against a baseline that the run on `main` collected, and the full suite runs once per merged commit.

Context: only if the nextest issue of this epic ends with "switch"; otherwise cancel this issue with that reason. The maintainer decided on 2026-11-22 that `main` is fast-forwarded and that the slice after rc.1 integrates by pull requests. nx-rust's `docs/ci.mara.md`, "The coverage baseline", gives the arrangement and a worked example for GitHub Actions; `ADR-NX-RUST-TASKS` and the CI Design item of this epic hold the decisions.

- [ ] The run on `main` (and on the integration branch until the fast-forward) restores the baseline's two files from the CI cache, runs the full `test`, runs `coverage-baseline` with the options and environment of the test runs, and saves the two files under the commit
- [ ] The pull-request run restores the files of its base commit, or the newest older ones, and runs with `NX_RUST_TEST_MODE=affected`; each task's output says what it ran and why
- [ ] Collection's parallelism is capped so that it fits the runner's memory, and the same `NEXTEST_` values are set in both runs
- [ ] The cost is in the closing summary from real runs: the time of the run on `main` with and without collection, the size of the saved files, and the time of a pull-request run for one representative change against a full run. If collection costs more than selection saves, say so and recommend
- [ ] A baseline is never restored without its record, and a run that finds none runs the full suite

Not part of this issue: the baseline of a developer's machine and of worktrees (the nextest issue).
