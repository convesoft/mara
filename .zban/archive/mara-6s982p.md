---
title: code_add_counts expects 3 reads of the configuration whatever its age, and fails when it settles during the run
status: done
type: bug
parent: mara-xzfeyj
created_at: 2026-11-25T11:56:28Z
closed_at: 2026-11-25T12:05:19Z
---

## Goal

`code_add::code_add_counts` (`crates/mara/tests/performance/code_add.rs`) asserts counts that do not depend on the runner's speed: it puts the configuration in each state it measures and asserts the count DES-WRITE-PROCEDURE and DES-INPUTS-AND-KEYS give for that state.

## Context

- CI run https://github.com/convesoft/mara/actions/runs/45606937271, job "Checks (ubuntu-22.04)" (x64), on a pull request head that changes no Rust code (merged with `main` at `05ea32f`); the arm job passed, and `main` at `05ea32f` passed on both images. One test of 843 failed: `mara code add at 16×`, left `file_reads: 2, hashes: 2`, right `file_reads: 3, hashes: 3`.
- The test expects the configuration read and hashed for the edit, at step 2 and before its rename. Reads 2 and 3 are conditional: `tracked::check` keeps what a check found when the stat is unchanged and that check came at least 2 s (`SETTLE`) after the file's mtime and ctime (DES-INPUTS-AND-KEYS). The corpus generator writes the configuration before the `.git` directory, the read-back of every document and `results.json`, so at 16× on a slow runner the configuration can be close to 2 s old when `code add` reads it.
- Not known at filing: which of the conditional reads was skipped on the runner, and whether the product is right in that state (a finding against DES-WRITE-PROCEDURE would be a data-safety item for the maintainer).

## Tasks

- [x] Find which read is conditional and on what, and reproduce the count of 2 deterministically on this machine.
- [x] Decide by the docs whether the test or the product is wrong.
- [x] Make the test fix the configuration's state for each count it asserts, and align VER-CODE-WORK-COUNTERS if it must say more.
- [x] Check the other exact-count tests of `crates/mara/tests/performance/` for the same dependence on a file's age; fix those that have it.

## Summary of Changes

**Cause: the test, not the product.** `code add` reads the configuration three times at most, through `tracked::check`:

- Read 1, `Basis::read` before the edit: unconditional (no previous record).
- Read 2, step 2 of DES-WRITE-PROCEDURE (`Target::check` in `apply`): only when read 1 came less than 2 s after the configuration's mtime or ctime (`Stat::settled_at`); otherwise it keeps read 1's record.
- Read 3, the check before the rename (`Target::replace`): only when the check before it, read 2 or the record read 1 left, came less than 2 s after that change.

So the count is 1 when the configuration last changed at least 2 s before `code add` started, 3 when step 2 still comes less than 2 s after it, and 2 when the 2 s mark falls between read 1 and step 2, a span that holds the grammar fetch and its store in the cache. The corpus generator writes the configuration before the `.git` directory, the read-back of every document and `results.json`, so at 16× on the slower x64 runner it was nearly 2 s old when `code add` started: CI's count of 2 is exactly this middle case. In it, step 2's read comes 2 s or more after the change, so it settles the file and the check before the rename rightly keeps it (DES-INPUTS-AND-KEYS); DES-WRITE-PROCEDURE holds in all three cases. A mtime set into the past cannot bring the case about, since that sets the ctime to now.

**Reproduction on this machine** (with a probe case that was later removed). The case drove `code add` with `MARA_TEST_PAUSE`, rewrote the configuration just before the run, and held the writer at the pause before step 2 until 2.1 s after the configuration's change. With the old expectation of 3 it failed as CI did: `mara code add at 1×, settled at step 2`, left `file_reads: 2, hashes: 2`, right `file_reads: 3, hashes: 3`.

**Fix.** `code_add_counts` (`crates/mara/tests/performance/code_add.rs`) now sets the configuration's state for each count it asserts, at each size of `PULL_REQUEST_SIZES`:

- settled, after `restore::settle`: 1 read and 1 hash (the write row of VER-WORK-COUNTERS);
- unsettled at every check, the file rewritten with its mtime an hour ahead: 3 and 3;

Each run must have written the pin, and the server must count exactly 2 connections per run. When the check before the rename was removed from `write.rs` as a trial (reverted), the unsettled case failed with 2 instead of 3. VER-CODE-WORK-COUNTERS's `code add` row states the two tested states with their counts, and notes that the state between them gives 2 and is not tested. No file under `crates/mara/src/` changed.

**Other tests checked, independent of a file's age:**

- `init.rs`, `prime.rs` and `daemon.rs` assert 0 reads, which no file's age changes.
- `convert.rs`, `work_counters.rs` (writes; `moved_hub_fix_counts`) and `warm_scaling.rs` settle before each write.
- `snapshot_counts` passes its own clock 3 s ahead. Its edit changes the file's size, and its changes to undeclared files expect 0.
- `code_get.rs`: the cold cases count first reads only. The warm cases settle before the daemon starts. The changed code file is read only by the request, since a background derivation reads no code.
- `warm.rs` and `warm_code.rs`: every warm row starts settled. A row that counts reads after a change uses `derived::Case`, or counts a file that only the request reads, or asserts parses, builds and program runs only.

**Checks at 85d0b93**: `NX_DAEMON=false NX_USE_LOCAL=true berth --exclusive -- pnpm run checks` with a fresh `NX_CACHE_DIRECTORY` exited 0. Workspace: 843 tests run, 843 passed, 14 skipped. `project validate`: 0 errors, 0 warnings. `mara-engine:build:wasm32` built. Performance: 32 tests run, 32 passed, 2 skipped.

**Checks after the review round, merged with `origin/main` at `361f00b`**: `pnpm run checks` exited 0. Workspace: 843 tests run, 843 passed, 14 skipped. `mara-npm:test` (`node --test`): 84 passed. `project validate`: 0 errors, 0 warnings. `mara-engine:build:wasm32` built. Performance: 32 tests run, 32 passed, 2 skipped.

## Decisions made

- VER-CODE-WORK-COUNTERS's `code add` row gives the configuration's read and hash counts for two states that the test sets itself, independent of the runner's speed: last changed at least 2 s before the run (1), and a modification time an hour ahead of the request's clock, which keeps the record unsettled at each check (3). A past mtime cannot settle a file, because setting it makes the ctime now. A configuration that settles between the first read and step 2 gives 2. The row says so in a note, and no test asserts it.
- The orchestrator decided on review to remove the debug-only "settled at step 2" case: its 2 s cutoff still depended on the runner's speed, and it could not detect a missing step-2 check.
