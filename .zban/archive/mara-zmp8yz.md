---
title: Starting and cancelling Soufflé is safe in the daemon
status: done
type: bug
parent: mara-0mxw21
created_at: 2026-11-20T20:22:51Z
closed_at: 2026-11-21T09:48:50Z
---

Goal: starting a Soufflé child and stopping the daemon neither allocates between `fork` and `exec`, nor panics a worker thread, nor leaves a private evaluation directory behind.

Context: Found by the maintainer's code audit of 2026-11-20 (at `a5f8ec0`, then `a5f8ec0..c6a9706`), filed at the maintainer's request. Locations were checked at `c6a9706`. Items: DES-RULE-EXECUTION and DES-SOUFFLE-SANDBOX (docs/rules/evaluation.mara.md), DES-DAEMON-LIFECYCLE (docs/daemon/index.mara.md). The interrupt flag shared by concurrent evaluations is backlog issue mara-vday9w and not part of this issue.

- `crates/mara/src/rules.rs:352`: inside `pre_exec`, `io::Error::other("Mara ended before Soufflé started")` allocates on the heap, which is not async-signal-safe. `rules/sandbox.rs` already reports its steps with `io::Error::from_raw_os_error`, without allocating.
- `crates/mara/src/rules/explain.rs:114` and `:123`: `Exchange::start` calls `std::thread::spawn` for its writer and its reader. Every other spawn the daemon can reach uses `thread::Builder::new().spawn` and handles the error; this one panics the worker thread when the system cannot start a thread, and the client sees `daemon-lost`.
- `crates/mara/src/rules.rs:144-148` and `explain` in `crates/mara/src/rules/explain.rs`: `cancel::Running::begin()` is followed by `private_dir(run)?`, which creates `$TMPDIR/mara-*` and writes the fact files, and `cancel::cancelled()` is checked only later in `spawn_and_wait`. If the daemon's shutdown calls `cancel::all()` and sees no running evaluation just before `Running::begin()`, it can call `process::exit` while `private_dir` is writing, and the directory stays, because `TempDir`'s drop does not run.

Done when the three hold and the local checks pass.

- [x] No heap allocation in `pre_exec`
- [x] `Exchange::start` reports a thread that cannot be started as a fault of the request, without a panic
- [x] A cancelled evaluation creates no private directory: `cancelled()` is checked right after `Running::begin()` and before `private_dir`, in `evaluate` and in `explain`; a test stops the daemon at that point and finds no `mara-*` directory left

## Summary of Changes

- `crates/mara/src/rules.rs`: `pre_exec` reports a parent that ended with `io::Error::from_raw_os_error(ESRCH)`, which allocates nothing. `evaluate` and `explain` begin through `Started::begin()`: it counts the evaluation (`cancel::Running`), defers the signals, then checks `cancel::cancelled()` before `private_dir`, and refuses with "cannot run Soufflé: the daemon stops". Because the count comes before the check, and the daemon sets the flag before it reads the count (both `SeqCst`), either the daemon waits for the evaluation or the evaluation creates no directory. `spawn_and_wait` kills and reaps Soufflé when `Exchange::start` fails.
- `crates/mara/src/rules/explain.rs`: `Exchange::start` returns `io::Result` and starts its writer and reader through `thread::Builder::spawn` (`mara-souffle-in`, `mara-souffle-out`). A thread that cannot be started is a host fault of the request, not a panic.
- `crates/mara/src/cli/daemon/server.rs`: `rules::cancel::all()` now runs before the socket is removed, so whoever finds the socket gone also finds the evaluations cancelled. The test relies on this.
- Debug-only pause point `MARA_TEST_PAUSE_EVALUATION`. It holds an evaluation after it is counted and before the check.
- Tests (`crates/mara/tests/daemon/caller.rs`): `stop_before_an_evaluation_creates_its_directory_leaves_none` (`query execute`) and `stop_before_a_report_creates_its_directory_leaves_none` (`query report`, provenance mode). Each stops the daemon with `mara daemon stop` at the pause point and requires the following: the daemon exits, the client fails, and inotify saw no entry created in the client's `TMPDIR`, so no directory is left. With the `cancelled()` check removed, both tests fail ("a cancelled evaluation created its directory").
- Docs: DES-DAEMON-LIFECYCLE (the ordering), DES-RULE-EXECUTION (a thread that cannot start), ADR-DEBUG-PAUSE-POINT (the new pause point).

Checks: `cargo fmt --all --check`, clippy with warnings denied, `cargo test --workspace --locked`, `project validate` (valid; its one warning in `docs/distribution.mara.md` predates this change), the release `performance` test under `berth --exclusive` (22 passed), the `wasm32` build of `mara-engine` and `zban check` all pass.

## Decisions made

- The test stops the daemon through a new debug-only pause point, `MARA_TEST_PAUSE_EVALUATION`, recorded in ADR-DEBUG-PAUSE-POINT. Its name begins with `MARA_TEST_PAUSE`, so the existing release-build check also covers it. Rejected: stopping at a random moment, which almost never reaches a window that is a few instructions wide.
- The daemon calls `cancel::all()` before it removes its socket, not after. This lets a test observe that the cancellation happened (DES-DAEMON-LIFECYCLE).
- The test watches the client's `TMPDIR` with inotify, so it also catches a directory that was created and then removed. Without the watch it would pass on the old code too, whose later check in `spawn_and_wait` removed the directory.
- `pre_exec` returns `ESRCH` for a parent that ended. No one reads this error, because Mara has already ended.
