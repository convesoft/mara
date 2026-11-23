---
title: The memory-limit test reaches the time limit first on slow runners
status: done
type: bug
priority: high
parent: mara-c8scn7
related:
  - mara-vxeweg
discovered_from: mara-a3aq0n
created_at: 2026-11-23T07:43:48Z
closed_at: 2026-11-23T08:30:40Z
---

Goal: `programs::faults::a_program_past_the_memory_limit_is_stopped_before_the_time_limit` (`crates/mara/tests/programs/faults.rs`) passes on every image that runs `pnpm run checks`, the release's `ubuntu-22.04` included.

Context: run https://github.com/convesoft/mara/actions/runs/45129288351 attempt 1 (mara-a3aq0n's release dry run, `Checks (ubuntu-22.04)`, no caches) failed it with `".mara/rules/p.dl: the evaluation was stopped at its time limit of 10 s" lacks "memory limit of 1024 MiB"` after 19.5 s; attempt 2 passed it. It also hit the time limit first under coverage instrumentation (mara-7wysb0, mara-vxeweg). Its program grows `p` by 10^10 pairs and relies on Soufflé reaching `RLIMIT_AS` of 1 GiB within 10 s, so it depends on the machine's speed.

- [x] Make the test reach the memory limit with enough margin under load on supported runners (for example a program that allocates faster, or a test-only time limit far above the memory limit's reach), keeping what it verifies
- [x] It passes on `ubuntu-22.04` x64

## Summary of Changes

- The test's program now reaches the memory limit by copying memory, not by inserting tuples: a symbol doubled by `cat` 24 times to 16 MiB, then one `cat` of 128 copies of it (2 GiB, twice the limit), still with one output row per item through a `count`. Soufflé alone, under `RLIMIT_AS` of 1 GiB, takes 0.2 s here to `std::bad_alloc`; the previous program took 3.0 s (CPU-bound B-tree inserts).
- VER-PROGRAM-FAULTS: the memory-limit case names the new program; its expectation is unchanged.
- No product limit, order of limits or product code changed; no retry, skip or hook.

Checks:
- Locally: the test 60 of 60 passes, about 1.0 s per test (4 evaluations); 3 runs of the whole `programs` binary, 69 passed each; no core dump (`coredumpctl list`).
- `ubuntu-22.04` x64, run https://github.com/convesoft/mara/actions/runs/45136139463 (temporary workflow, removed): 20 runs alone at 2.93–3.03 s per test; 10 runs inside the whole `programs` suite at 3.69–3.87 s; 10 runs while the whole suite ran beside it at 3.41–3.92 s. All passed; the four commands averaged under 1 s each, and the test checks each command against the 10 s limit.
- `berth --exclusive -- pnpm run checks`: exit 0; 829 passed, 14 skipped; performance 32 passed, 2 skipped; `project validate` 0 errors, 0 warnings.

## Decisions made

- The memory-limit case of VER-PROGRAM-FAULTS uses a program that reaches the limit by copying memory (a doubled symbol and a `cat` of 128 copies) instead of a `count` over all pairs of a `range`, so that it reaches the memory limit well before the time limit on the tested runners; recorded in the case row of VER-PROGRAM-FAULTS.
