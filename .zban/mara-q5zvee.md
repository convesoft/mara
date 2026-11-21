---
title: A failed kill skips reaping when an exchange thread cannot start, and the stop tests accept two answers
status: todo
type: bug
priority: deferred
tags:
  - deferred
  - backlog
discovered_from: mara-zmp8yz
created_at: 2026-11-21T10:08:41Z
---

Goal: the two loose ends the review of mara-zmp8yz found (at `938af7b`, integrated as `2efef0d`) are closed. Neither is reached by a realistic setup.

Context: DES-RULE-EXECUTION (docs/rules/evaluation.mara.md), DES-DAEMON-LIFECYCLE (docs/daemon/index.mara.md).

- `crates/mara/src/rules.rs`, `spawn_and_wait`: when `explain::Exchange::start` cannot start its writer or reader thread, the cleanup is `child.kill()?; child.wait()?; return Err(error)`. If `kill` itself fails (it needs an outer process policy that denies it), `wait` is skipped, the child is not reaped, and the request reports the kill error instead of the thread-start fault.
- `crates/mara/tests/daemon/caller.rs`, the two tests that stop the daemon at `MARA_TEST_PAUSE_EVALUATION`: DES-DAEMON-LIFECYCLE says a request ended by the stop fails with `daemon-lost`, and the server does that, but the tests also accept a returned `the daemon stops` fault.

- [ ] Reap the child whether or not `kill` succeeded, and report the thread-start fault
- [ ] The two stop tests require `daemon-lost`
