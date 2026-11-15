---
title: Hub case catches a per-edge row scan with a thin margin on a busy machine
status: done
type: task
priority: low
related:
  - mara-000f9q
discovered_from: mara-98y7dd
created_at: 2026-11-14T20:58:04Z
closed_at: 2026-11-15T09:48:44Z
---

Goal: the scaling test catches a per-edge scan of the target's rows, the fault the hub case of DES-PERFORMANCE-HARNESS (docs/performance.mara.md) was added for in mara-000f9q, by a clear margin also on a busy machine.

Context: in mara-98y7dd the fault was planted in a copy under `$TMPDIR` (`snapshot.item(target).value("id")` per edge in `Checker::edge` instead of the collected facts). It failed the release performance test in 10 of 10 runs, always on `mara project validate on a hub` from 4× to 16×. At load 30–70 the measured growth was 2.36–2.52 per doubling with the CPU-time harness of ADR-SCALING-CPU-TIME (docs/decisions-runtime.mara.md), and 2.34–2.54 with the previous wall-clock harness under the same load; mara-000f9q measured 2.54 on a quieter machine. The limit is 2.3 (VER-SCALING). The scan adds 1000 × size row comparisons per edge, which is small next to the rest of a hub validation, whose own time grows 2.1–2.2 per doubling through cache effects. The hub's shape (1000 × size items and late rows) is part of the Accepted DES-PERFORMANCE-HARNESS.

- [x] Decide whether the hub's shape should make a per-edge row scan dominate its time, for example more late rows per incoming edge, and record it in DES-PERFORMANCE-HARNESS
- [x] If the shape changes: generator, exact hub counts in `hub_counts`, and the fault planted again in a copy, failing by a clear margin at load 30 or more

## Summary of Changes

- `crates/mara/tests/performance/corpus.rs`: the hub's 1000 × size other rows now alternate between `Tags` and `No`, as long as `Type` and `ID`; before, all were `Tags`. Both are repeatable text fields in the hub's schema. Items, edges, documents, row count and work counts are unchanged, so `hub_counts` and its expected counts stay as they are and it passes. `cases()`, `check_growth`, `LIMIT`, `ROUNDS`, `MARGIN`, `RUNS` and the other cases are untouched.
- Docs: the DES-PERFORMANCE-HARNESS hub bullet (docs/performance.mara.md) states the alternating keys. A new Decision item, ADR-HUB-ROW-KEYS (docs/decisions-runtime.mara.md, Justifies DES-PERFORMANCE-HARNESS), records the choice, the measurements and the rejected shapes.

Cause: Rust compares the lengths of two keys before their bytes. With only `Tags` rows (length 4), a per-edge scan for `id` stopped at the length, about 0.4 ns per row. A scan for `type` (length 4) compared bytes, about 1.0 ns per row. That made the ID scan small next to the rest of a hub validation.

Evidence (AMD Ryzen 9 9950X3D, 32 CPUs). Each fault was planted in its own copy of the workspace under `$TMPDIR`, with its own target directory, and the copies were deleted afterwards. The ID scan is `snapshot.item(target).value("id")` per edge in `Checker::edge`. A Type scan (`label_name(snapshot.item(target).value("type"))` in `target_type`) was added so the new shape would not weaken the other read of late metadata. Growth from 4× to 16×, before → after. "Quiet" is 9 interleaved rounds at load 1.4–2.6. Every other figure is the real gate (`check_growth` on the hub alone), whose failing pair runs 60 runs per size.

| Condition | Correct code (both pairs) | ID scan | Type scan |
| --- | --- | --- | --- |
| Quiet, load 1.4–2.6 | 2.04/2.11 → 1.94/2.19 | 2.46, 2.57 → 2.86 | 2.98 → 2.81 |
| Other projects' tests only, load 9–29 | — | 2.37 → 2.57 | 2.67 → 2.55 |
| 32 `yes > /dev/null`, load 19–48 | 2.00/2.12, 2.02/2.18 → 2.02/2.01, 1.95/2.08 (pass) | 2.46 → 2.71, 2.61 | — → 2.62 |
| 24 `yes` + 8 `dd if=/dev/zero of=/dev/null bs=64M`, load 34–52 | 2.14/2.27, 2.16/2.10 → 2.26/2.24 (pass) | 2.41 → 2.63, 2.62, 2.65 | 2.70 → 2.56 |

- Every fault run failed the gate on the 4×→16× pair. Under load, the smaller margin of the two scans grew from 0.07–0.16 to 0.25–0.31. The ID scan's margin grew from 0.07–0.16 to 0.27–0.41.
- Interference was started by me, bounded with `timeout`, and stopped when done. The load average includes another worker and another project's test suite.
- Correct code's hub CPU time at 16×, quiet: 166 ms before, 163 ms after.
- Time of `cargo test --package mara --test performance --release --locked`, before and after runs alternated (before from an unchanged copy). Under 32 `yes` processes: before 36.7, 44.7 and 13.0 s; after 59.6, 35.1 and 26.3 s; all passed. Without my interference (load 9–20): before 81.2 and 62.2 s; after 45.9 and 54.3 s. The spread comes from how many pairs run to 60 runs; there is no systematic difference. A first quiet run before the change took 55.3 s at load 1.5–7. In an instrumented run at load 6–8, the hub case took 14 of the test's 37 s with both pairs at 60 runs, which a shape with more rows would have lengthened.

Checks, on the working tree: `cargo fmt --all --check`, clippy with warnings denied, `cargo test --workspace --locked` (247 passed, 4 ignored), `project validate` with the worktree's binary (valid, 0 errors, 0 warnings), the release performance test (9 passed, 2 ignored, 45.5 s at load 15), the `wasm32` engine build and `zban check` all exit 0.

Rejected approaches (also in ADR-HUB-ROW-KEYS):

- Leaving the shape: the ID scan exceeded the limit by only 0.06–0.22 under load.
- Four times as many `Tags` rows: ID scan 2.92 at load 10, but the hub took 60% more CPU time per run (198 → 316 ms at 16×).
- A block of `Tags` rows, then a block of as many `No` rows: both scans 3.15 quiet, but the hub took 11% more CPU time per run, and correct code failed the gate in 2 of 2 runs under the memory-writing load (2.35, 2.36).
- Only `No` rows: ID scan 3.09 quiet, but the Type scan fell to 2.54.

## Decisions made

- The hub's 1000 × size other rows alternate between `Tags` and `No`, keys as long as `Type` and `ID`, with the number of rows unchanged (DES-PERFORMANCE-HARNESS, ADR-HUB-ROW-KEYS).
- The rationale and measurements are in a new Decision item, ADR-HUB-ROW-KEYS, not in DES-PERFORMANCE-HARNESS, because AGENTS.md keeps rationale in Decision items. DES-PERFORMANCE-HARNESS states only the shape.
- Decided by the maintainer (mara-ge0tq2): the limit stays 2.3 per doubling, and the scaling check is not defined for a machine whose memory bandwidth other processes saturate; a failure there is rerun without that load (DES-PERFORMANCE-HARNESS). Raising the limit to 2.4 was rejected (ADR-SCALING-CPU-TIME): correct code under that load would pass by only 0.04–0.09, and the weakest planted fault would fail by 0.15 instead of 0.25.

Filed: mara-ge0tq2. Under 24 spinning and 8 memory-writing processes (load 34–52), unchanged code failed the release performance test in 3 of 3 runs, on `RepeatedEnum` (2.31–2.36 from 4× to 16×).
