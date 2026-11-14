---
title: Daemon performance tests
status: todo
type: task
parent: mara-0mxw21
blocked_by:
  - mara-jkabqy
  - mara-xqcnqw
created_at: 2026-11-13T22:10:06Z
---

Context: docs/verification.mara.md (VER-WORK-COUNTERS); REQ-READS-DURING-VALIDATION in docs/performance.mara.md. The nightly run and the time budgets (VER-TIME-BUDGETS, REQ-TIME-BUDGETS, both Draft) wait for mara-4dr2p2.

- [ ] Counter tests against the daemon: warm, after a watched edit
- [ ] Test: a read issued during a long validation of the same project returns before the validation ends
