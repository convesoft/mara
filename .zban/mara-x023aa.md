---
title: No test exhausts the memory limit by the gradual growth of a relation
status: todo
type: task
priority: deferred
tags:
  - deferred
  - backlog
discovered_from: mara-4xsamc
created_at: 2026-11-23T08:37:43Z
---

Goal: a test that shows an evaluation whose memory runs out gradually, through many small allocations of a growing relation, is stopped at the memory limit and reports it, not the time limit.

Context: mara-4xsamc replaced the program of the memory-limit case of VER-PROGRAM-FAULTS (`crates/mara/tests/programs/faults.rs`) with one that fails a single large `cat` allocation, because growing a relation to 1 GiB took 3 s on a fast machine and over 10 s on a loaded `ubuntu-22.04` runner. The remaining gradual-growth tests (`crates/mara/tests/programs/time_limit.rs:89`, `programs_that_never_end_are_stopped_by_a_limit`) accept a stop by either limit, so they can pass without the memory-fault path ever running. Soufflé's handling of `std::bad_alloc` deep in a B-tree insert is therefore untested.

A test of it must not depend on the machine's speed again. Options:
- A test-only memory limit far below 1 GiB, through `crates/mara/src/hooks.rs` (release builds must not contain `MARA_TEST_`), so that a growing relation reaches it in a small fraction of the time limit; the product limits stay as DES-RULE-EXECUTION records them.
- Or a relation whose tuples allocate much per insert (for example distinct long symbols), measured to reach 1 GiB in well under 1 s on a fast machine, with evidence of a margin on the slowest supported runner under load.

- [ ] Choose an approach and record it in VER-PROGRAM-FAULTS
- [ ] Add the test; it passes repeatedly on `ubuntu-22.04` x64 under load
