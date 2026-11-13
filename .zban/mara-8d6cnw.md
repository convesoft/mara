---
title: Build the snapshot and track inputs
status: todo
type: feature
parent: mara-t1rffa
blocked_by:
  - mara-t6xn9z
  - mara-b52dg3
created_at: 2026-11-13T22:10:06Z
---

Goal: an immutable snapshot assembled from per-file results, rebuilt only for what changed.

Context: docs/engine.mara.md (DES-INPUTS-AND-KEYS, DES-SNAPSHOT); REQ-WORK-FOLLOWS-CHANGE in docs/performance.mara.md.

- [ ] Inventory of declared inputs; stat check and BLAKE3 hashing
- [ ] Content keys for parsed documents and the document snapshot
- [ ] Snapshot: items, ID and MID maps, forward and inverse edges, item spans, token index
- [ ] VER-WORK-COUNTERS rows for an unchanged request, one edited document and unrelated changes pass
