---
title: A request run without a worker thread sends no heartbeats
status: todo
type: bug
priority: deferred
tags:
  - deferred
  - backlog
discovered_from: mara-xqcnqw
created_at: 2026-11-20T14:53:32Z
---

Goal: a live daemon never has a request reported `daemon-lost`, also when it cannot start a thread.

Context: the review of mara-xqcnqw. `answer_working` in `crates/mara/src/cli/daemon/server.rs` runs an operation request on a worker thread while the connection's thread sends a heartbeat every second (DES-DAEMON-LIFECYCLE). When creating the worker thread fails, the request runs on the connection's thread without heartbeats, so a request longer than the client's 15 s silence bound is reported `daemon-lost` while the daemon still runs it.

- [ ] Keep heartbeats when no worker thread can be created (for instance, send them from a thread created ahead, or refuse the request so that the client serves it in-process)
- [ ] Test with a thread limit that leaves the connection's thread but no worker
