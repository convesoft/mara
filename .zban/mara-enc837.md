---
title: Decide whether the daemon watches code directories steadily or never
status: todo
type: task
priority: deferred
tags:
  - deferred
  - backlog
discovered_from: mara-9fphtg
created_at: 2026-11-21T19:35:56Z
---

Goal: the watches of directories that only code files can be in follow one rule, and cost no inotify calls between requests.

Context: found while closing mara-9fphtg. A session watches the directories its last inventory listed (DES-DAEMON-SESSIONS in docs/daemon/index.mara.md, `Session::directories` in crates/mara/src/session.rs). Only a request that lists the code files lists a directory that only code files can be in. So such a directory is watched from that request until the next background derivation or request that lists no code, which removes the watch; the next code request adds it again. A background derivation reads no code, so the watch saves no work of a request. The cost is one `inotify_add_watch` and one `inotify_rm_watch` per code directory and cycle, plus a derivation that does nothing after a code edit. It never gives a stale answer: every request checks the code files on disk. DES-DAEMON-SESSIONS states the behaviour as of mara-9fphtg.

- [ ] Decide: never watch a directory that only code can be in (watch the documents walk's directories), or watch every directory of the last listing of the code files (`State::ancestors` in crates/mara/src/inputs.rs holds them since mara-9fphtg), or keep the current rule
- [ ] Update DES-DAEMON-SESSIONS and `tests/daemon/watches.rs` to the decision
