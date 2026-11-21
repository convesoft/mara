---
title: Warm counts after a watched change and after a write
status: in-progress
type: task
parent: mara-0mxw21
related:
  - mara-p27z9t
discovered_from: mara-xqcnqw
created_at: 2026-11-20T13:42:55Z
---

Goal: the warm rows of VER-WORK-COUNTERS that follow a change hold against the daemon (mara-p27z9t checks them).

Context: found while building sessions (mara-xqcnqw). DES-DAEMON-SESSIONS, DES-INPUTS-AND-KEYS, DES-PERFORMANCE-HARNESS.

- The watcher re-derives about 50 ms after a change, so its check comes less than 2 s after the file's mtime and does not settle it: the next request reads and hashes that file again. The rows "One document edited …" and "The request after a write …" also run with the request sent 2 s after the watcher re-derived and must give the same counts, so the session reads the file twice where the row allows once.
- A write served by a session does not hand its read-back parses to the session, so the request after it parses the written files again, where the row "The request after a write" expects 0 parses.
- No evaluation is kept between requests, where the row "A request whose evaluations an earlier request ran with the same keys" expects 0 program runs.

- [ ] Decide how a session counts an unsettled file that the watcher reports unchanged since its check (for instance: trust a watch that was in place before the check), and record it in DES-DAEMON-SESSIONS
- [ ] A write served by a session leaves its read-back parses and the written files' stats in the session
- [ ] Evaluations kept per session by key (DES-RULE-EXECUTION), within the memory the daemon allows
