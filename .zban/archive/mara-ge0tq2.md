---
title: Unchanged code fails the release performance test under memory-bandwidth-heavy load
status: done
type: bug
related:
  - mara-98y7dd
discovered_from: mara-kx7fhh
created_at: 2026-11-15T09:33:40Z
closed_at: 2026-11-15T10:04:45Z
---

Goal: the release performance test (`cargo test --package mara --test performance --release --locked`) passes unchanged code also while other processes saturate memory bandwidth, or the maintainer rules that such load is outside what the local check must withstand.

Context: found in mara-kx7fhh on an AMD Ryzen 9 9950X3D (32 CPUs, 14 GiB). Interference: 24 `yes > /dev/null` processes and 8 `dd if=/dev/zero of=/dev/null bs=64M` processes, beside another worker (load 34–52).

- 3 of 3 runs of the release performance test failed in `per_row_growth_per_doubling`, all on `mara project validate on RepeatedEnum` from 4× to 16×, with 60 runs per size: 2.32× (medians 6.49, 19.86, 107.21 ms), 2.36× (6.36, 20.90, 116.23 ms), 2.31× (6.47, 21.37, 114.50 ms). Two runs were on 7470fff. One run had only mara-kx7fhh's hub corpus change, which does not touch `RepeatedEnum`. In the third run the interference stopped partway through (load 46.7 to 29.0). Each run took 95–197 s.
- Under the same load, the hub case on unchanged code grew 2.27× and 2.10× from 4× to 16× in two runs, and 2.26× from 1× to 4× in one.
- Under 32 `yes` processes alone (load 50–58), 6 of 6 runs passed in 13–60 s.
- CPU time (ADR-SCALING-CPU-TIME, docs/decisions-runtime.mara.md) leaves out time spent waiting for a CPU. It still counts the extra cycles that cache and memory-bandwidth contention cost, and those slow the larger sizes more, because their working sets leave the caches.

Constraints: VER-SCALING's limit, sizes and subject, and the judging rule of mara-98y7dd, are fixed.

- [x] Decide with the maintainer whether the local check must pass under memory-bandwidth-heavy load: it need not. The limit stays 2.3, such load is out of scope, and a failure there is rerun (decided by the maintainer)
- [x] Record in DES-PERFORMANCE-HARNESS (docs/performance.mara.md) what load the scaling check holds under, and that it is not defined under memory-bandwidth saturation, where a failure is rerun

## Summary of Changes

No code changes. The maintainer decided that the local check need not pass under memory-bandwidth-heavy load. The limit stays 2.3 per doubling.

- DES-PERFORMANCE-HARNESS (docs/performance.mara.md) records two things. The scaling test holds on a machine shared with other builds and tests, or with a CPU-bound process on every hardware thread: unchanged code passed beside a looping 16-job release build (load 14–31), other projects' tests (load 9–20) and 32 spinning processes (load 50–58). It is not defined for a machine whose memory bandwidth other processes saturate, and a failure there is rerun without that load.
- ADR-SCALING-CPU-TIME (docs/decisions-runtime.mara.md) records the rejected alternative, raising the limit to 2.4, as rejected by the maintainer.
- The archived mara-kx7fhh lists the decision under `## Decisions made`.

Checks: `cargo fmt --all --check`, clippy with warnings denied, `cargo test --workspace --locked`, `project validate` with the worktree's binary, the `wasm32` engine build and `zban check`. The release performance test was not rerun, since only documents and issue files changed.

## Decided by the maintainer

Question put to the maintainer: "The number is the allowed growth of an operation's CPU time when the project doubles in size (REQ-NEAR-LINEAR-SCALING, Accepted: at most 2.3× per doubling from 1× to 16×). Linear work is 2.0, quadratic is 4.0. Correct code measures 1.9–2.2 normally and 2.27–2.36 under the memory-heavy load. Planted faults measure 2.55–2.97 (the weakest was 2.37–2.46 before today's hub change, which is in review). At 2.4, correct code under that load would pass by 0.04–0.09 and the weakest fault would be caught by 0.15 instead of 0.25. Which do you want?"

Answer: "Keep 2.3, load out of scope (Recommended) — The limit stays. DES-PERFORMANCE-HARNESS says the check is not defined for a machine saturated on memory bandwidth and is rerun there. mara-ge0tq2 closes with this. Widest margin to the planted faults; a failure under such load is a rerun, not a bug."

Rejected: relaxing the limit to 2.4.
