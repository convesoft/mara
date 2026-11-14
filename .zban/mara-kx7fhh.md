---
title: Hub case catches a per-edge row scan with a thin margin on a busy machine
status: todo
type: task
priority: low
related:
  - mara-000f9q
discovered_from: mara-98y7dd
created_at: 2026-11-14T20:58:04Z
---

Goal: the scaling test catches a per-edge scan of the target's rows, the fault the hub case of DES-PERFORMANCE-HARNESS (docs/performance.mara.md) was added for in mara-000f9q, by a clear margin also on a busy machine.

Context: in mara-98y7dd the fault was planted in a copy under `$TMPDIR` (`snapshot.item(target).value("id")` per edge in `Checker::edge` instead of the collected facts). It failed the release performance test in 10 of 10 runs, always on `mara project validate on a hub` from 4× to 16×. At load 30–70 the measured growth was 2.36–2.52 per doubling with the CPU-time harness of ADR-SCALING-CPU-TIME (docs/decisions-runtime.mara.md), and 2.34–2.54 with the previous wall-clock harness under the same load; mara-000f9q measured 2.54 on a quieter machine. The limit is 2.3 (VER-SCALING). The scan adds 1000 × size row comparisons per edge, which is small next to the rest of a hub validation, whose own time grows 2.1–2.2 per doubling through cache effects. The hub's shape (1000 × size items and late rows) is part of the Accepted DES-PERFORMANCE-HARNESS.

- [ ] Decide whether the hub's shape should make a per-edge row scan dominate its time, for example more late rows per incoming edge, and record it in DES-PERFORMANCE-HARNESS
- [ ] If the shape changes: generator, exact hub counts in `hub_counts`, and the fault planted again in a copy, failing by a clear margin at load 30 or more
