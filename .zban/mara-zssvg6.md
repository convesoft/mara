---
title: A daemon started for a project that is gone has no session and lives its whole idle period
status: todo
type: bug
priority: deferred
tags:
  - deferred
  - backlog
discovered_from: mara-xqcnqw
created_at: 2026-11-20T17:42:24Z
---

Found by the worker of mara-xqcnqw in its fix round: a client's on-demand `mara daemon start` that runs after its project directory was removed starts a daemon that opens no session. With no session to drop, it stays until the idle period ends (30 minutes) and then exits. In the test suite this left a daemon behind in 2 of 10 loaded runs; the worker hardened the test cleanup (`crates/mara/tests/support/daemons.rs`) and did not pin down the exact race.

Who reaches it: someone who runs `mara` in a directory and deletes or moves the directory within the moment the background daemon needs to start. Harmless (one idle process for up to 30 minutes), so it is in the backlog.

- [ ] Decide what a daemon does when the project it was started for cannot be opened: exit at once, or wait a short time for a first request (DES-DAEMON-LIFECYCLE, DES-IDLE-SESSIONS)
- [ ] A test starts a daemon for a removed project and requires it to be gone within that time
