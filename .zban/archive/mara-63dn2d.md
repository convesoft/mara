---
title: Decide which issue asserts the cold counter rows, since mara-8d6cnw has no CLI operation to run
status: done
type: task
tags:
  - maintainer
related:
  - mara-8d6cnw
  - mara-000f9q
  - mara-bmc9gc
discovered_from: mara-csmk0c
created_at: 2026-11-14T14:12:43Z
closed_at: 2026-11-14T14:38:38Z
---

Goal: the cold rows of VER-WORK-COUNTERS (docs/verification.mara.md) are asserted through the real CLI by an issue that has an operation to run.

Context: mara-8d6cnw's last item asks that the cold rows pass through the real CLI. No CLI operation exists before `project validate` (mara-000f9q) and the read operations (mara-bmc9gc), and both are blocked by mara-8d6cnw. The harness from mara-csmk0c (DES-PERFORMANCE-HARNESS, `crates/mara/tests/performance/`) takes one case per operation; until then it runs only `mara --debug-stats` without an operation.

Options:

- A (recommended): move the cold-row item from mara-8d6cnw to mara-000f9q and mara-bmc9gc, each asserting the rows for its own operations. mara-8d6cnw counts file reads, hashes, parses and snapshot builds where they happen, and tests them below the CLI.
- B: mara-8d6cnw adds the first operation (for example `item list` from mara-bmc9gc) to assert the rows. Moves scope between issues.
- C: reorder the issues so that an operation lands before mara-8d6cnw closes.

- [x] Option chosen and the affected issues' items updated

## Summary of Changes

Decided by the maintainer on 2026-11-14: option A, chosen as "A: move to operations (Recommended)": "Move the cold-row item from mara-8d6cnw to mara-000f9q and mara-bmc9gc, each asserting the rows for its own operations through the real CLI. mara-8d6cnw counts reads, hashes, parses and snapshot builds where they happen and tests them below the CLI. Still within alpha.1."

The orchestrator changed the items with `zban`: mara-8d6cnw's last item now asks for counting where the work happens, tested below the CLI; mara-000f9q and mara-bmc9gc each gained an item for the cold rows of VER-WORK-COUNTERS through the real CLI. No document changed: VER-WORK-COUNTERS names no issue. Check: `zban check`.
