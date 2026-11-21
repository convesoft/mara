---
title: Run the rename's conflict, interruption and retry cases through the daemon
status: todo
type: task
priority: deferred
tags:
  - deferred
  - backlog
related:
  - mara-j6vpbt
  - mara-71nbb6
discovered_from: mara-9fphtg
created_at: 2026-11-21T20:15:29Z
---

Goal: the cases of VER-CODE-EDITS in which a rename that rewrites markers meets a conflicting save, is interrupted, or is retried also run with the write served by a daemon the test starts.

Context: findings of the review of mara-9fphtg (Codex, at `282f9ae`, 2026-11-21; log `~/.tmp/codex/review-mara-9fphtg-282f9ae/findings.md`), left out of that issue by the orchestrator: VER-CODE-EDITS (docs/code/verification.mara.md) asks "the rename through `mara mcp`, and through the daemon", which mara-9fphtg runs; it does not ask each of its cases through the daemon. In `crates/mara/tests/code/edits.rs` the pause-driven conflict test (near 359) and the interruption test (near 392) start their writes outside `Project::json`, so the daemon comparison of `crates/mara/tests/code/main.rs` covers the reads after them, not the writes or the retries. Writes through a daemon with pauses are covered for items in `crates/mara/tests/daemon/writes.rs`.

- [ ] The conflict, interruption and retry cases of the rename run with the write served by the test's daemon (`served` rises), with the same files and results as direct mode
- [ ] The two grammar fallback tests of `crates/mara/tests/code/grammar.rs` (near 260) also run the other code readers after the grammar was removed or damaged, if VER-GRAMMAR-MARKERS is read to ask it
