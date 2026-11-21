---
title: "A grammar dropped while another request reloads it: waiters stay blocked and the replacement still takes parses"
status: todo
type: bug
priority: deferred
tags:
  - deferred
  - backlog
discovered_from: mara-gqvffn
created_at: 2026-11-21T11:04:58Z
---

Goal: a grammar that is dropped for the process while another request reloads it after a cache change is not used again, and no request waits longer for it than its own limits.

Context: found by the review of mara-gqvffn (at `249a65e`), which rated both P2; the orchestrator moved them to the backlog, because both need the grammar's cache files to change while a parse of the old instance stalls. `crates/mara/src/grammar.rs`, ADR-GRAMMAR-WATCHDOG-THREAD and DES-GRAMMAR-ASSETS (docs/code/index.mara.md).

- When an old instance's parse times out while another request reloads the same language and checksum, the key is dropped, but callers that already hold the slot stay blocked until the reload releases its lock, and a reload that is still checking the files can start its thread after the key was dropped.
- A scan can hold the replacement instance across files while the old instance times out. Abandonment sets the old instance's flag; the replacement checks only its own flag before it sends work, so the scan can still get grammar-level results, or wait for another watchdog limit, after the process dropped the key.

- [ ] A reload that finds its key dropped starts no thread, and its waiters are released at once
- [ ] An instance checks the process's record of dropped grammars, not only its own flag, before it takes a parse
