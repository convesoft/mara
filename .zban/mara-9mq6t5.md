---
title: Run the daemon's concurrency and scaling cases on macOS
status: todo
type: task
priority: deferred
tags:
  - deferred
  - backlog
discovered_from: mara-p27z9t
created_at: 2026-11-21T13:47:07Z
---

Goal: the macOS variants of VER-READS-DURING-VALIDATION (docs/daemon/verification.mara.md) and of the daemon cases of VER-SCALING (docs/verification.mara.md) run.

Context: found by the review of mara-p27z9t (at `086560b`). `crates/mara/tests/daemon/reads.rs` and `crates/mara/tests/performance/warm_scaling.rs` are compiled on Linux only: they find processes through `/proc`, and the daemon's CPU time is read there; DES-PERFORMANCE-HARNESS (docs/performance.mara.md) names `proc_pid_rusage` for macOS, which is not implemented. File watching does not exist on macOS either (backlog mara-txbf8x). No macOS machine ran the rewrite's tests so far.

- [ ] The daemon's CPU time on macOS, as DES-PERFORMANCE-HARNESS says
- [ ] The concurrency cases and the daemon scaling cases compile and pass on macOS
