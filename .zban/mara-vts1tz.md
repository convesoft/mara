---
title: Two requests that scan code with the same grammar parse one at a time
status: todo
type: task
priority: deferred
tags:
  - deferred
  - backlog
discovered_from: mara-xqcnqw
created_at: 2026-11-20T19:54:41Z
---

Found by the re-check of mara-xqcnqw at `d1295e7`: two requests that use code links and the same pinned grammar at once wait for each other, because `Grammar::parse` holds one lock per grammar (`crates/mara/src/grammar.rs`). In the re-check the second request waited 1.16 s behind a parse that the debug stall hook held. mara-s9gak4 chose this on purpose (ADR-GRAMMAR-WATCHDOG-THREAD: parses of one grammar run one at a time, so one request's time limit never counts another request's work), and results are correct. Document reads and programs without code links are not affected.

Who reaches it: two simultaneous code-link requests on a large source tree of one language, for example two agents running `query gate` on a cold daemon. It costs time, never correctness, so it is in the backlog.

- [ ] Decide whether a grammar gets more than one store or thread, or whether requests share one scan of a file (ADR-GRAMMAR-WATCHDOG-THREAD, DES-DAEMON-SESSIONS)
- [ ] A test with two code-link requests at once shows the second is not held for the length of the first's parse
