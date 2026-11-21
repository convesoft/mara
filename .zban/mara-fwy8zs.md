---
title: Each operation declares once whether it writes and whether the client runs it; WorkCounts fields are listed once
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

Goal: adding an operation or a counter needs one edit, and cannot leave a second table behind.

Context: Found by the architecture audit after 0.4.0-alpha.4 (Gemini, at `2fa2c1b`, 2026-11-21); locations are as of that commit. Wave: seam. `crates/mara/src/cli/client.rs` has `IN_PROCESS` and `WRITES` (lines 42-55), lists of operation words apart from `OPERATIONS` in `crates/mara/src/cli.rs`; `crates/mara/tests/daemon/parity.rs` repeats `IN_PROCESS`. A write operation missing from `WRITES` is not waited for when the daemon stops (DES-DAEMON-LIFECYCLE). `client.rs` (lines 283-294) names the eight `WorkCounts` fields to decode them, where encoding uses `work.fields()`; `crates/mara/tests/performance/warm.rs` repeats the eight. `client.rs` also parses each reply frame twice (`is_heartbeat`, then the reply), as the server did before mara-ef6ybb.

- [ ] An operation's entry in `cli.rs` says whether it writes and whether it runs in the client; `client.rs` and the tests read it
- [ ] `WorkCounts` offers its fields for decoding; the client parses each frame once
