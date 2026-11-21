---
title: A warm hit of a kept outcome clones no rows or proofs, and a report computes its roles once
status: todo
type: task
priority: deferred
tags:
  - deferred
  - backlog
  - audit
discovered_from: mara-0mxw21
created_at: 2026-11-21T16:35:20Z
---

Goal: answering from a session's kept outcome copies a pointer, not the rows and proof trees.

Context: Found by the architecture audit after 0.4.0-alpha.4 (Gemini, at `2fa2c1b`, 2026-11-21); locations are as of that commit. Wave: behavior. `crates/mara/src/rules/invariants.rs` borrows a kept outcome; `crates/mara/src/rules/consumer.rs` clones it whole on every warm hit (`(*evaluated.evaluate(...)).clone()` near line 56, `(*evaluated.explain(...)).clone()` near line 77). `report` in `crates/mara/src/cli/query.rs` runs `caused(entry, rows, item, snapshot)` inside `select` and again on the rows it gets back. The reviewer of mara-6za81b noted the same.

- [ ] `Consumed` holds the shared outcome; `report` keeps the roles and outcome it computed in `select`
- [ ] Unchanged results and work counts
