---
title: A session's kept outcomes and a Soufflé executable replaced in place or between its stat and its start
status: todo
type: bug
priority: deferred
tags:
  - deferred
  - backlog
discovered_from: mara-8hw038
created_at: 2026-11-21T12:16:40Z
---

Goal: an outcome a session keeps is never given for another Soufflé executable than the one that produced it, also in the cases below.

Context: left open by the re-check of mara-8hw038 (at `de80f59`, integrated as `1a3c6c5`), rated P2 there; the orchestrator moved it to the backlog, because each case needs the executable to change in a precise way while a session lives. A session keys its kept evaluation outcomes by the evaluation key and by the Soufflé executable's path, size and modification time (`session::Build` in `crates/mara/src/session.rs`; DES-DAEMON-SESSIONS in docs/daemon/index.mara.md, DES-RULE-EXECUTION in docs/rules/evaluation.mara.md).

- The host takes the executable's identity before the evaluation and then starts it by path (`crates/mara/src/rules.rs`): an executable replaced between the two has its outcome stored under the identity of the one it replaced.
- A replacement that keeps path, size and modification time, or an executable whose execute permission was revoked, still gives the kept outcome, where an evaluation would now differ or fail.

- [ ] Decide whether the identity needs more (for example the inode and change time, or the identity taken from the file the process was started from), and record it in DES-DAEMON-SESSIONS
