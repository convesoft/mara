---
title: Report an output write that the Soufflé sandbox stopped as a fault
status: todo
type: task
priority: deferred
tags:
  - deferred
  - backlog
discovered_from: mara-ptq6xc
created_at: 2026-11-16T21:47:35Z
---

Goal: an evaluation whose Soufflé tried to write outside the private directory, and was stopped by the sandbox, is a fault instead of a passing result.

Context: DES-SOUFFLE-SANDBOX (`docs/rules/evaluation.mara.md`) denies every write outside `out/`. Soufflé 2.5 skips an output file it cannot open without a message and exits 0 (EVD-SOUFFLE-PROGRAM-REACH, row of 2026-11-16), so a planted `.output` to a path outside is stopped and the evaluation returns the program's rows. Only a program past the admission check can hold such a directive (DES-PROGRAM-PROFILE refuses every input and output directive). Landlock returns `EACCES` and has no mode that ends the process; seeing the denial needs kernel audit records, ptrace or seccomp user notification. Tested in `planted_directives_cannot_read_or_write_outside` (`crates/mara/src/rules/sandbox/tests.rs`).

- [ ] Decide whether a stopped write must fail the evaluation, and by which mechanism
