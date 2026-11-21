---
title: Daemon performance tests
status: in-progress
type: task
parent: mara-0mxw21
blocked_by:
  - mara-jkabqy
  - mara-xqcnqw
  - mara-mg711b
  - mara-zmp8yz
  - mara-8hw038
created_at: 2026-11-13T22:10:06Z
---

Context: docs/verification.mara.md (VER-WORK-COUNTERS); REQ-READS-DURING-VALIDATION in docs/performance.mara.md. The nightly run and the time budgets (VER-TIME-BUDGETS, REQ-TIME-BUDGETS, both Draft) wait for mara-4dr2p2.

- [ ] The warm rows of VER-WORK-COUNTERS pass against the daemon, edits made while it watches included
- [ ] Test: VER-READS-DURING-VALIDATION (docs/verification-daemon.mara.md)
- [ ] Test: the daemon cases of VER-SCALING (docs/verification.mara.md), timed as DES-PERFORMANCE-HARNESS says
