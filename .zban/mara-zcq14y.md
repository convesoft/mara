---
title: The grammar registry keeps the reason of each dropped grammar until the process ends
status: todo
type: task
priority: deferred
tags:
  - deferred
  - backlog
discovered_from: mara-mg711b
created_at: 2026-11-21T10:36:02Z
---

Goal: decide whether the grammar registry's records of dropped grammars need a bound, and bound them if so.

Context: found by the review of mara-mg711b (at `1c0b322`), which rated it P2; the orchestrator demoted it to the backlog, because no realistic setup grows it. `crates/mara/src/grammar.rs` keeps, per language name and checksum, the reason a grammar was dropped for the process (a stalled parse, and from mara-gqvffn a load that timed out), as a string that holds the path of the cache file or of the source file. `unavailable-grammar` diagnostics copy it. The record is by design (ADR-GRAMMAR-WATCHDOG-THREAD in docs/code/index.mara.md): a dropped grammar stays dropped until the process ends. It survives the session that caused it and grows only with the number of distinct pins the process ever dropped, so a daemon would need many different broken grammar pins to hold a noticeable amount. Items: ADR-GRAMMAR-WATCHDOG-THREAD, DES-IDLE-SESSIONS (docs/daemon/index.mara.md).

- [ ] Decide whether a bound is needed (for example a fixed number of records, or a reason without the path), and record it in ADR-GRAMMAR-WATCHDOG-THREAD
