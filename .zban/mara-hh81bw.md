---
title: Measure the snapshot rebuild after one edit against the warm read budget
status: todo
type: task
priority: deferred
tags:
  - deferred
  - backlog
blocked_by:
  - mara-17gykj
related:
  - mara-p27z9t
discovered_from: mara-8d6cnw
created_at: 2026-11-14T16:40:30Z
---

Goal: a warm daemon read after a one-document edit stays within the warm budget of REQ-TIME-BUDGETS (docs/performance.mara.md, Draft: 20 ms at 16×), or the snapshot build is made cheaper until it does.

Context: after one edit, a host that keeps state re-parses one document and rebuilds the snapshot in one linear pass (DES-SNAPSHOT, docs/engine.mara.md). Measured on the maintainer's machine when mara-8d6cnw landed (release build, generated corpus, best of 9): `SnapshotInputs::build` takes 0.34 ms at 1×, 1.3 ms at 4×, 11 ms at 16× and 53 ms at 64×. About 80% of it is merging the documents' search words into the token index: one hash lookup per document word, so it is linear in words, but it slows from about 70 ns to 165 ns per word once the index no longer fits the CPU caches (between 4× and 16× here). The cold path is dominated by parsing (37 ms of 45 ms at 16×) and grows 1.95× and 2.13× per doubling, within the 2.3× limit of REQ-NEAR-LINEAR-SCALING. Tried: reserving the word table up front (no gain); 8-byte postings (`u32` item index) gave about 20% at 16× and 64× and was not kept, to keep `usize` indices in the API.

- [ ] Time a warm read after one edit through the daemon at 16× and 64× (with mara-p27z9t)
- [ ] If it exceeds the budget, make the token-index merge cheaper (smaller postings, fewer allocations per new word, or reuse of the unchanged documents' part of the index) and measure again

## Moved to the backlog, 2026-11-15

One-time sweep under the maintainer's amended rules of 2026-11-15: low priority, the budget it measures against is Draft, and mara-p27z9t (daemon performance tests) times the warm read anyway. Nothing here starts without the maintainer.
