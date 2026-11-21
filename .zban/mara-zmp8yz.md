---
title: Starting and cancelling Soufflé is safe in the daemon
status: in-progress
type: bug
parent: mara-0mxw21
created_at: 2026-11-20T20:22:51Z
---

Goal: starting a Soufflé child and stopping the daemon neither allocates between `fork` and `exec`, nor panics a worker thread, nor leaves a private evaluation directory behind.

Context: Found by the maintainer's code audit of 2026-11-20 (at `a5f8ec0`, then `a5f8ec0..c6a9706`), filed at the maintainer's request. Locations were checked at `c6a9706`. Items: DES-RULE-EXECUTION and DES-SOUFFLE-SANDBOX (docs/rules/evaluation.mara.md), DES-DAEMON-LIFECYCLE (docs/daemon/index.mara.md). The interrupt flag shared by concurrent evaluations is backlog issue mara-vday9w and not part of this issue.

- `crates/mara/src/rules.rs:352`: inside `pre_exec`, `io::Error::other("Mara ended before Soufflé started")` allocates on the heap, which is not async-signal-safe. `rules/sandbox.rs` already reports its steps with `io::Error::from_raw_os_error`, without allocating.
- `crates/mara/src/rules/explain.rs:114` and `:123`: `Exchange::start` calls `std::thread::spawn` for its writer and its reader. Every other spawn the daemon can reach uses `thread::Builder::new().spawn` and handles the error; this one panics the worker thread when the system cannot start a thread, and the client sees `daemon-lost`.
- `crates/mara/src/rules.rs:144-148` and `explain` in `crates/mara/src/rules/explain.rs`: `cancel::Running::begin()` is followed by `private_dir(run)?`, which creates `$TMPDIR/mara-*` and writes the fact files, and `cancel::cancelled()` is checked only later in `spawn_and_wait`. If the daemon's shutdown calls `cancel::all()` and sees no running evaluation just before `Running::begin()`, it can call `process::exit` while `private_dir` is writing, and the directory stays, because `TempDir`'s drop does not run.

Done when the three hold and the local checks pass.

- [ ] No heap allocation in `pre_exec`
- [ ] `Exchange::start` reports a thread that cannot be started as a fault of the request, without a panic
- [ ] A cancelled evaluation creates no private directory: `cancelled()` is checked right after `Running::begin()` and before `private_dir`, in `evaluate` and in `explain`; a test stops the daemon at that point and finds no `mara-*` directory left
