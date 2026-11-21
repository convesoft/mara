---
title: Split the session's outcome store and the CLI's argument parsing into modules of their own
status: todo
type: task
priority: deferred
tags:
  - deferred
  - backlog
  - audit
blocked_by:
  - mara-fwy8zs
  - mara-sb5d25
discovered_from: mara-0mxw21
created_at: 2026-11-21T16:35:21Z
---

Goal: `session.rs` holds sessions and their derivation; `cli.rs` holds the operations and their dispatch.

Context: Found by the architecture audit after 0.4.0-alpha.4 (Gemini, at `2fa2c1b`, 2026-11-21); locations are as of that commit. Wave: split. More than two thirds of `crates/mara/src/session.rs` (about 870 lines) is the bounded store of evaluation and report outcomes (`Evaluated`, `Kept`, `Outcome`, `Build`, the size estimates and their tests), independent of `Sessions`, `Session` and `Deriving`: move it to `session/evaluated.rs`. `crates/mara/src/cli.rs` (about 850 lines) mixes the operation table and `run` with argument parsing: move `parse`, `requested_json`, `describe_arguments`, `usage` and their tests to `cli/arguments.rs`.

- [ ] Both moves made; no behaviour changes
