---
title: Split the engine's program/resolve.rs and finish splitting the warm and code test files
status: todo
type: task
priority: deferred
tags:
  - deferred
  - backlog
  - audit
blocked_by:
  - mara-5d6pcs
  - mara-j6vpbt
  - mara-kvyteb
related:
  - mara-71nbb6
  - mara-pqhtf6
  - mara-ar4aht
discovered_from: mara-779tms
created_at: 2026-11-21T21:32:55Z
---

Goal: three files that grew past what is safe to change are split by concern, with no change of behaviour.

Context: found by the architecture audit after 0.4.0-alpha.5 (Gemini, at `368fad1`, 2026-11-21); wave: split. Sizes are as of that commit.
- `crates/mara-engine/src/program/resolve.rs`, 1,285 lines: `.include` loading (1–147), namespaces, components and types (148–774), clause, aggregate and plan checking (776–1285, `ClauseChecker`, `components_of_recursion`). Move the last part to its own module.
- `crates/mara/tests/performance/warm.rs`, 891 lines: `warm_code.rs` (mara-9fphtg) started the split by subject; the rule, evaluation and session rows can follow into their own file beside the `Warm` harness.
- `crates/mara/tests/code/main.rs`, 660 lines: the `Project` fixture, the daemon comparison (`Daemon`), `restore`, the MCP client (`Mcp`) and the pause helpers. `Daemon` and `Mcp` belong in `tests/support/` (mara-j6vpbt).
- `crates/mara/tests/daemon/parity.rs`, 776 lines: its code part can become its own case, as long as one check still asserts that every MCP tool that is not in-process was sent (`Comparison::finish`, near 180–198).
The audit's note on mara-pqhtf6: move `Evaluated` and its tests out of `session.rs` (about 510 lines), not `Deriving`, which belongs with `Session::derive_in_background`.

- [ ] `program/resolve.rs` is split at the clause checker
- [ ] `warm.rs` and `tests/code/main.rs` are split as above, after the shared helpers exist
- [ ] `parity.rs` is split only with the coverage assertion kept
