---
title: Build the performance harness
status: todo
type: task
parent: mara-t1rffa
blocked_by:
  - mara-0ztabc
created_at: 2026-11-13T22:10:06Z
---

Goal: the work-counter and scaling tests exist before the first operation, so every operation ships with them.

Context: docs/performance.mara.md (DES-PERFORMANCE-HARNESS, REQ-WORK-FOLLOWS-CHANGE, REQ-NEAR-LINEAR-SCALING); docs/verification.mara.md (VER-WORK-COUNTERS, VER-SCALING). The audit tools named in EVD-PREVIOUS-IMPLEMENTATION-AUDIT show a working scale-corpus generator for the old format.

- [ ] Seeded corpus generator for 1×, 4×, 16× and 64×, writing into temporary directories
- [ ] Work counters in engine and hosts, exposed by a debug stats option
- [ ] Counter test and scaling test as ordinary Rust integration tests, run in CI
- [ ] `[profile.profiling]` as described under Profiling
