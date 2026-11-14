---
title: Scaling tests fail intermittently when the machine is busy
status: in-progress
type: bug
priority: high
parent: mara-t1rffa
related:
  - mara-csmk0c
  - mara-000f9q
  - mara-bmc9gc
  - mara-37qtf2
created_at: 2026-11-14T19:32:13Z
---

Goal: the scaling tests of the performance harness pass or fail on how the code grows, not on what else the machine is doing, so that `cargo test --package mara --test performance --release --locked` is a reliable local check.

Context: DES-PERFORMANCE-HARNESS (docs/performance.mara.md) and VER-SCALING (docs/verification.mara.md); the tests are `crates/mara/tests/performance/scaling.rs` (`growth_per_doubling`, `per_row_growth_per_doubling`, and the generic `check_growth`). VER-SCALING's limit is 2.3× per doubling from 1× to 16×. Found by the orchestrator and by the reviewer of mara-37qtf2 while other builds and reviews ran on the same machine (load 16 to 20 on 32 CPUs):

- On 5914428 plus a change to configuration and documents only (01d06e0): 2 failures in 6 runs in one session and 2 in a row in another. `mara project validate on RepeatedRelation grows 2.31× per doubling from 4× to 16×; medians [(1, 2.431154ms), (4, 7.39335ms), (16, 39.396893ms)]`; `RepeatedRelation` 2.31× with medians 7.041 and 37.585 ms at 4× and 16×; `ManyTargets` 2.35× with medians 17.649 and 97.364 ms.
- On the read-operations branch (52934a5): 1 failure in 6 runs, `mara search <every 9-letter word> grows 2.32× per doubling from 4× to 16×; medians [(1, 2.577193ms), (4, 8.844767ms), (16, 47.704112ms)]`.
- The same cases measure 1.97× to 2.18× on a quieter machine, and reviewers' runs under synthetic CPU load passed five times in a row, so the failures come from interference, not from super-linear code. Real faults measured so far fail well above the limit: quadratic work 2.86×, a per-edge row scan 2.54× to 2.66×, the repeated-ID scan 2.97× to 3.04×.

Constraints: VER-SCALING and REQ-NEAR-LINEAR-SCALING are Accepted: the limit, the sizes and what is measured (a cold run through the real CLI) stay. How the harness measures is DES-PERFORMANCE-HARNESS's to say (today: one warm-up round, five rounds alternating sizes, medians of neighbouring sizes compared). Candidates, not decisions: the child process's CPU time (user plus system) instead of wall-clock time; the minimum instead of the median; more rounds; measuring again when a result is close to the limit. A planted super-linear fault must still fail every time.

- [ ] Failure rate before the change recorded: the release performance test run repeatedly while other builds run on the machine
- [ ] Measurement made robust to that interference without changing VER-SCALING's limit, sizes or subject; the method written into DES-PERFORMANCE-HARNESS
- [ ] 50 consecutive runs under the same interference pass, and each planted super-linear fault (quadratic work; a per-edge row scan; the repeated-ID scan) fails every time in 10 runs
- [ ] The per-change check run is not noticeably slower
