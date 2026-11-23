---
title: The memory-limit test reaches the time limit first on slow runners
status: in-progress
type: bug
priority: high
parent: mara-c8scn7
related:
  - mara-vxeweg
discovered_from: mara-a3aq0n
created_at: 2026-11-23T07:43:48Z
---

Goal: `programs::faults::a_program_past_the_memory_limit_is_stopped_before_the_time_limit` (`crates/mara/tests/programs/faults.rs`) passes on every image that runs `pnpm run checks`, the release's `ubuntu-22.04` included.

Context: run https://github.com/convesoft/mara/actions/runs/45129288351 attempt 1 (mara-a3aq0n's release dry run, `Checks (ubuntu-22.04)`, no caches) failed it with `".mara/rules/p.dl: the evaluation was stopped at its time limit of 10 s" lacks "memory limit of 1024 MiB"` after 19.5 s; attempt 2 passed it. It also hit the time limit first under coverage instrumentation (mara-7wysb0, mara-vxeweg). Its program grows `p` by 10^10 pairs and relies on Soufflé reaching `RLIMIT_AS` of 1 GiB within 10 s, so it depends on the machine's speed.

- [ ] Make the test reach the memory limit independently of the machine's speed (for example a program that allocates faster, or a test-only time limit far above the memory limit's reach), keeping what it verifies
- [ ] It passes on `ubuntu-22.04` x64
