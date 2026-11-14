---
title: Sessions, watching and concurrency
status: todo
type: feature
parent: mara-0mxw21
blocked_by:
  - mara-17gykj
created_at: 2026-11-13T22:10:06Z
---

Context: docs/daemon.mara.md (REQ-NEVER-STALE, DES-DAEMON-SESSIONS); REQ-READS-DURING-VALIDATION in docs/performance.mara.md.

- [x] Decide new-file detection, the idle period and session eviction (DES-INPUTS-AND-KEYS, DES-IDLE-SESSIONS)
- [ ] Idle sessions are dropped, and the daemon exits with its last session (DES-IDLE-SESSIONS)
- [ ] One session per canonical project root
- [ ] Watching with debounce and full rescan on overflow; stat check on every request
- [ ] Lock-free reads, per-project write queue, cancellable worker threads
