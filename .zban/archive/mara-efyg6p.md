---
title: Session input tests with a clock an hour ahead fail on ubuntu-22.04 x64
status: done
type: bug
priority: high
parent: mara-c8scn7
related:
  - mara-agdnwh
discovered_from: mara-a3aq0n
created_at: 2026-11-23T07:43:48Z
closed_at: 2026-11-23T08:17:20Z
---

Goal: the unit tests in `crates/mara/src/inputs.rs` that pass `now = SystemTime::now() + 3600 s` ("Late enough that every check settles what it read") pass on every image that runs `pnpm run checks`, the release's `ubuntu-22.04` included.

Context: the release workflow's dry run (mara-a3aq0n) runs `pnpm run checks` on `ubuntu-22.04` and `ubuntu-22.04-arm` without caches (DES-RELEASE-WORKFLOW). Run https://github.com/convesoft/mara/actions/runs/45129288351 on `d185c3c` failed `Checks (ubuntu-22.04)` twice, each time one test of this family, a different one each attempt; arm64, CI's `ubuntu-24.04` images and a local run pass:

- Attempt 1: `inputs::tests::an_older_scan_keeps_a_file_a_newer_one_added`, `inputs.rs:1982`, `left: []`, `right: ["src/lib.rs"]`: the file rewritten with contents of the same size was not read again.
- Attempt 2: `inputs::tests::a_code_file_that_left_is_forgotten`, `inputs.rs:1919`, `left: None`, `right: Some(0)`: after `.mara/config.toml` was rewritten, `code_file("src/e.rs")` did not see the new configuration.

- Attempt 3: three of them, `a_code_file_that_left_is_forgotten`, `an_older_scan_keeps_a_file_a_newer_one_added` and `an_older_scan_does_not_bring_back_a_file_a_newer_one_dropped` (`inputs.rs:1960`, `left: ["src/lib.rs", "src/other.rs"]`, `right: ["src/lib.rs"]`: a deleted file was kept). So the failures are systematic on that image, not rare; the release cannot pass its Checks there until this is fixed.

Ubuntu 22.04 is the oldest platform that DES-PACKAGES supports, so whether the product misses changes there matters, not only the tests.

Likely cause, unconfirmed: with a clock an hour ahead, every stat counts as settled, so a rewrite in the same coarse timestamp tick as the previous check, of the same size, is invisible; with the real clock such a file is unsettled and read again. Whether the second failure has the same cause is not shown.

- [x] Find the cause on `ubuntu-22.04` x64 (for example a job that runs `cargo nextest run -p mara --lib inputs::` in a loop on that image) and say whether the product or only the test is wrong
- [x] Fix it; the tests pass repeatedly on that image

## Summary of Changes

**Cause: the tests, not the product.** These are observed facts from probe runs on the temporary branch `probe/mara-efyg6p`. The workflow file and probe code exist only on that branch, never on this one.

- **Image.** https://github.com/convesoft/mara/actions/runs/45136788334
  - `ubuntu-22.04` x64: `6.8.0-1070-azure`, `CONFIG_HZ=1000`, work directory and `/tmp` on ext4 (`/dev/sda1 ext4 rw,relatime,…`). Two shell writes got `w1 …08:12:54.869057438`, `w2 …08:12:54.870057428`. A Rust test writing a file twice got the same mtime in 10 of 10 pairs (`difference=Ok(0ns)`), with the mtimes stepping by 1 ms. In 0.3 s about 1500 writes got about 300 distinct mtimes.
  - `ubuntu-22.04-arm`: the same kernel and tick (`difference=Ok(0ns)` in 9 of 10).
  - `ubuntu-24.04` (Linux 7.0) gave distinct mtimes to each write of the Rust pairs and of the Python file and directory loops (`difference=Ok(60.04µs)` and similar), as on this machine (7.2).
- **The three tests, instrumented, unfixed, 30 runs each image.** Each run printed, at each step where a test changes a path, the stat the session kept of it, the stat on disk and the `now` of the next check.
  - `ubuntu-22.04`: 4 of 30 runs failed. `ubuntu-22.04-arm`: 23 of 30 failed. `ubuntu-24.04`: 0 of 30.
  - In every failed run, the failing tests are exactly those whose stat at that step was unchanged. No failure came without one.
  - In each such step the kept and the disk stat are equal in size, mtime, ctime and inode. The check's `now` is 3600 s after that mtime, so the stat counts as settled (DES-INPUTS-AND-KEYS) and is not read or listed again.
  - Per test, all of the same cause:
    - `an_older_scan_keeps_a_file_a_newer_one_added`, the same-size rewrite of `src/lib.rs` (x64 run 1): `unchanged=true kept={size 26 m …671.453227245 c …671.453227245 ino 4686940} disk={size 26 m …671.453227245 …}`. Then `left: []`, `right: ["src/lib.rs"]`: the rewrite was not read.
    - `a_code_file_that_left_is_forgotten`, the rename of `src/c.rs` to `src/e.rs` (x64 run 18): `src unchanged=true kept={size 4096 m …677.842171791 …} disk={size 4096 m …677.842171791 …}`. Then `left: None`, `right: Some(0)`: `src/` was not listed again, so `src/e.rs` was not found. The configuration rewrite in that step changes the file's size, so it is always seen; it was not the cause.
    - `an_older_scan_does_not_bring_back_a_file_a_newer_one_dropped`, the new `src/.gitignore` (arm run 2): `src unchanged=true kept={size 4096 m …653.19720501 …} disk={size 4096 m …653.19720501 …}`. Then `left: ["src/lib.rs", "src/other.rs"]`: `src/` was not listed again, so the `.gitignore` and the drop it causes were not seen.
- **The real clock.** Run https://github.com/convesoft/mara/actions/runs/45135672701 shows what a real request would see. A probe test made each change right after a request's check, 200 times per kind: a same-size rewrite, an add, a rename, a new `.gitignore`, and `code_file` after a rename.
  - On `ubuntu-22.04`, 29–68 % of the changes landed in the tick of the stat the check saw. With the clock an hour ahead, every one of those was missed and no other change was: `same_tick=57 missed=57`, `same_tick=129 missed=129`, …
  - With the real clock, as a request passes it: `missed=0` for all 1000 changes, among them 126–134 same-tick changes per kind. The 2 s rule caught them.
  - Same result on `ubuntu-22.04-arm`.
- **Before the fix, the unchanged tests** failed on 14 of 30 runs on `ubuntu-22.04` and 25 of 30 on `ubuntu-22.04-arm`. Both 24.04 images: 0 of 30. Unlike the issue's context said, the arm image fails too.

**Fix.** `crates/mara/src/inputs.rs` gains the test helper `visibly(path, change)`. It runs the change, then sets `path`'s mtime one second past the one it had before.
- The three tests call it around each change that a stat may miss: the same-size rewrite of `src/lib.rs`, and `src/` after an add, rename, delete or new `.gitignore`.
- No assertion changed. The tests whose rewrites change the size are unchanged.
- No product code or docs changed: the product does what DES-INPUTS-AND-KEYS asks.

**Local checks** at the commit, `NX_DAEMON=false NX_USE_LOCAL=true berth --exclusive -- pnpm run checks`, exit 0: workspace `829 tests run: 829 passed, 14 skipped`; `project validate` `valid: 0 errors, 0 warnings`; `mara-engine:build:wasm32` ran; performance `32 tests run: 32 passed, 2 skipped`.

**After the fix.** Run https://github.com/convesoft/mara/actions/runs/45135998852 had 0 of 30 failing runs of `inputs::tests` on each of the four images. During that run, 50–88 % of the probe's changes on the 22.04 images still fell in one tick.

## Decisions made

- Tests that check with a clock an hour ahead make each change visible to a stat by setting the changed path's mtime explicitly (`visibly`, documented on the helper). No Mara item covers how unit tests fake time, so nothing else records it. Rejected alternatives:
  - sleeping past a tick, which guesses the tick;
  - another size, which does not exist for a directory;
  - the real clock in these tests, which would make their read counts depend on timing.
- No debug re-run of the release dry run's failed job: the probe on the same image answered the question.

## Review round 1

- **Three integration tests with the same latent defect.** These tests check with a clock at least 3 s ahead and then need a directory's new entries seen. Each now wraps that change in `visibly`, from `crates/mara/tests/support/stat.rs`, which `tests/inputs.rs` and `tests/write.rs` share:
  - `inputs::a_moved_document_is_read_but_not_parsed_again`: `docs/` and `docs/sub/` around the rename.
  - `inputs::an_added_document_is_parsed_and_a_removed_one_dropped`: `docs/` around the add and around the removal.
  - `write::a_written_item_loads_and_the_project_stays_valid`: `docs/` around `write::apply`, which creates `docs/sub dir/`.

  The unit-test copy of `visibly` in `src/inputs.rs` stays, since a library unit test cannot use the integration tests' support code. No assertion changed.
- **Shown on the image** with the three tests and the tick tests, 40 runs each on `ubuntu-22.04` and `ubuntu-22.04-arm` (Linux 6.8.0-1070-azure, ext4):
  - Before: https://github.com/convesoft/mara/actions/runs/45139026849, 0 of 40 failing on both. The collision did not show: in these tests, a whole first request, reading and parsing several documents, runs between the setup's last change to the directory and the test's change, which outlasts the 1 ms tick.
  - After: https://github.com/convesoft/mara/actions/runs/45139050563, 0 of 40 on both.
- **Equal-stat directory test.** `tracked::tests::an_entry_added_in_the_tick_of_the_listing_check_is_found_later` runs through `tracked::list`, the listing every request's walk uses. It gives a directory with a new entry a previous listing with the same stat and no entries:
  - checked 1 ms after the directory's last change, the directory is listed again and the entry found;
  - checked 2 s after, the old listing is kept.

  Without the 2 s rule in `list`, the test fails (`left: []`, `right: ["added.mara.md"]`, checked locally).
- **Audit of `crates/`** for a check with a clock at least 2 s ahead, then a same-size or directory-entry change, then a check that must see it. The binary has no clock hook: the CLI, daemon and MCP take `SystemTime::now()`, so process-level tests (daemon, CLI, performance warm rows) run on the real clock. Only tests that call the library with their own `now` can fake it. Those that remain are safe:
  - `src/inputs.rs` unit tests: the rest change sizes (`kept_contents_are_chosen_by_stat_not_by_time`, `a_warm_scan_copies_nothing_it_does_not_change`), use the real clock (`a_scan_that_a_change_overtook_is_not_kept`, `requests_with_the_same_time_each_check_the_configuration`), or set `visibly`.
  - `src/tracked.rs` tests: they pass the stat explicitly.
  - `tests/inputs.rs`:
    - These change a file's size: the edited document, the schema, the `.gitignore` edit, the configuration rewrite, the two-roles schema.
    - These expect nothing seen: `undeclared_ignored_and_git_files_cause_no_work`, and the unchanged rerun of `a_gitignore_the_crate_cannot_compile_keeps_its_directory_out`.
    - `touched_files_are_hashed_but_nothing_is_derived_again` sets an mtime explicitly.
    - The 2 s rule tests use the real clock.
  - `tests/write.rs`:
    - `refuses_an_edit_whose_file_changed_and_writes_nothing` changes sizes or a file's existence, which the write procedure checks itself.
    - `checks_read_a_file_again_only_when_the_two_second_rule_says_so` sleeps 20 ms before its same-bytes save, longer than a tick at `HZ` ≥ 100.
    - The other tests snapshot once.
  - `tests/write_pauses.rs`: the other program's writes change size (`the other program's contents`), or create a file whose basis is `Absent`.
  - `tests/performance/work_counters.rs` `snapshot_counts_at`: the edited document grows, and the undeclared writes expect no work.
- **Docs**, checked against the code:
  - DES-INPUTS-AND-KEYS: the 2 s rule now says it assumes timestamps advance on the checker's wall clock and a timestamp tick is shorter than 2 s (`Stat::settled_at` compares mtime and ctime with the request's `now`).
  - DES-INPUTS-AND-KEYS: a directory is listed again when its size, mtime, ctime, device or inode changed (`tracked::list` compares the whole `Stat`).
  - DES-DAEMON-SESSIONS: code files are read again when their stat changed or their last check left them unsettled (`tracked::check`).
- **Filed** mara-8sdrab (backlog): a file system whose clock runs behind the checker's can make an unsettled record look settled.
- **Corrected above:** the x64 Rust pairs were 10 of 10, and the late-clock same-tick share of run 45135672701 was 29–68 %.
