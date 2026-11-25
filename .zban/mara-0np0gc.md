---
title: Split crates/mara/tests/daemon/browser.rs (982 lines) by concern
status: todo
type: task
priority: deferred
tags:
  - deferred
  - backlog
  - audit
created_at: 2026-11-25T18:30:35Z
---

From the post-`mara-xzfeyj` architecture audit at `4cbcf26` (Finding #12, wave: `split`).

`crates/mara/tests/daemon/browser.rs` (982 lines) mixes the raw HTTP client and `Browser` fixture, serving/lifecycle/idle/pause-answer tests, and the unauthorized refusal matrix and preflight checks.

- [ ] Split into `crates/mara/tests/daemon/browser/mod.rs`, `browser/serving.rs`, and `browser/refusals.rs`
