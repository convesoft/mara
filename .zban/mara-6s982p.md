---
title: code_add_counts expects 3 reads of the configuration whatever its age, and fails when it settles during the run
status: in-progress
type: bug
parent: mara-xzfeyj
created_at: 2026-11-25T11:56:28Z
---

## Goal

`code_add::code_add_counts` (`crates/mara/tests/performance/code_add.rs`) asserts counts that do not depend on the runner's speed: it puts the configuration in each state it measures and asserts the count DES-WRITE-PROCEDURE and DES-INPUTS-AND-KEYS give for that state.

## Context

- CI run https://github.com/convesoft/mara/actions/runs/45606937271, job "Checks (ubuntu-22.04)" (x64), on a pull request head that changes no Rust code (merged with `main` at `05ea32f`); the arm job passed, and `main` at `05ea32f` passed on both images. One test of 843 failed: `mara code add at 16×`, left `file_reads: 2, hashes: 2`, right `file_reads: 3, hashes: 3`.
- The test expects the configuration read and hashed for the edit, at step 2 and before its rename. Reads 2 and 3 are conditional: `tracked::check` keeps what a check found when the stat is unchanged and that check came at least 2 s (`SETTLE`) after the file's mtime and ctime (DES-INPUTS-AND-KEYS). The corpus generator writes the configuration before the `.git` directory, the read-back of every document and `results.json`, so at 16× on a slow runner the configuration can be close to 2 s old when `code add` reads it.
- Not known at filing: which of the conditional reads was skipped on the runner, and whether the product is right in that state (a finding against DES-WRITE-PROCEDURE would be a data-safety item for the maintainer).

## Tasks

- [ ] Find which read is conditional and on what, and reproduce the count of 2 deterministically on this machine.
- [ ] Decide by the docs whether the test or the product is wrong.
- [ ] Make the test fix the configuration's state for each count it asserts, and align VER-CODE-WORK-COUNTERS if it must say more.
- [ ] Check the other exact-count tests of `crates/mara/tests/performance/` for the same dependence on a file's age; fix those that have it.
