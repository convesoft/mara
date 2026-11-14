---
title: Build the snapshot and track inputs
status: todo
type: feature
parent: mara-t1rffa
blocked_by:
  - mara-t6xn9z
  - mara-b52dg3
  - mara-63dn2d
created_at: 2026-11-13T22:10:06Z
---

Goal: an immutable snapshot assembled from per-file results, rebuilt only for what changed.

Context: docs/engine.mara.md (DES-INPUTS-AND-KEYS, DES-SNAPSHOT); REQ-WORK-FOLLOWS-CHANGE in docs/performance.mara.md.

- [ ] Inventory of declared inputs, filtered by the project's `.gitignore` files and without nested projects (DES-PROJECT-CONFIGURATION); stat check of files and directories; BLAKE3 hashing
- [ ] Content keys for parsed documents and the document snapshot
- [ ] Snapshot: items, ID and MID maps, forward and inverse edges, item spans, token index
- [ ] File reads, hashes, parses and snapshot builds are counted where they happen (DES-PERFORMANCE-HARNESS) and tested below the CLI. The cold rows of VER-WORK-COUNTERS are asserted through the real CLI by mara-000f9q and mara-bmc9gc, each for its own operations (decided by the maintainer, mara-63dn2d). The warm rows wait for the daemon (mara-p27z9t)
