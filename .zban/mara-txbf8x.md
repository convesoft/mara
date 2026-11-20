---
title: Watch inputs on macOS
status: todo
type: task
priority: deferred
tags:
  - deferred
  - backlog
discovered_from: mara-xqcnqw
created_at: 2026-11-20T13:42:55Z
---

Goal: a daemon on macOS re-derives its sessions in the background after a change, as on Linux (DES-DAEMON-SESSIONS: watch mode is on by default).

Context: mara-xqcnqw watches with inotify through `libc` on Linux only (`crates/mara/src/watch.rs`). Elsewhere `Watcher::start` returns `None`: every request still checks the inputs on disk, so answers are never stale, but each request after a change does the derivation itself.

- [ ] Watch the sessions' directories with FSEvents or kqueue, debounced as on Linux, with a full rescan on a dropped event
- [ ] Test it as `crates/mara/tests/daemon/fresh.rs` does, once a macOS run exists (mara-4dr2p2)
