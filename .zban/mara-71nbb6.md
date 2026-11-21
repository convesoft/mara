---
title: "Split the daemon tests: direct-mode and argument tests, and the harness"
status: todo
type: task
priority: deferred
tags:
  - deferred
  - backlog
  - audit
blocked_by:
  - mara-j6vpbt
discovered_from: mara-0mxw21
created_at: 2026-11-21T16:35:21Z
---

Goal: each file under `crates/mara/tests/daemon/` holds one suite, and the harness is a module of its own.

Context: Found by the architecture audit after 0.4.0-alpha.4 (Gemini, at `2fa2c1b`, 2026-11-21); locations are as of that commit. Wave: split. `tests/daemon/client.rs` (about 750 lines) holds dispatch tests, code-operation tests, direct-mode tests and argument tests: move the last two to `tests/daemon/direct.rs`. `tests/daemon/main.rs` (about 1,000 lines) holds the shared `Test` and `Run` harness and every start, stop, status, lock and runtime-directory test: move the harness to `tests/daemon/harness.rs`.

- [ ] Both moves made; the same tests run
