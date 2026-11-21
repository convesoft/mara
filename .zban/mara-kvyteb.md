---
title: One strongly-connected-components function in the engine
status: todo
type: task
priority: deferred
tags:
  - deferred
  - backlog
  - audit
discovered_from: mara-779tms
created_at: 2026-11-21T21:32:47Z
---

Goal: the engine has one implementation of Tarjan's strongly connected components.

Context: found by the architecture audit after 0.4.0-alpha.5 (Gemini, at `368fad1`, 2026-11-21); wave: seam. `components` in `crates/mara-engine/src/validation/checks.rs` (near 742–795) and `tarjan` in `crates/mara-engine/src/program/resolve.rs` (near 964–1012) both compute the components of `&[Vec<usize>]` iteratively and differ in when they set a node's index (on push, on first pop).

- [ ] One `pub(crate)` function serves both callers, with the tests of both
