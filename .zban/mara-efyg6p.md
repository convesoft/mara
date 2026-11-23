---
title: Session input tests with a clock an hour ahead fail on ubuntu-22.04 x64
status: in-progress
type: bug
priority: high
parent: mara-c8scn7
related:
  - mara-agdnwh
discovered_from: mara-a3aq0n
created_at: 2026-11-23T07:43:48Z
---

Goal: the unit tests in `crates/mara/src/inputs.rs` that pass `now = SystemTime::now() + 3600 s` ("Late enough that every check settles what it read") pass on every image that runs `pnpm run checks`, the release's `ubuntu-22.04` included.

Context: the release workflow's dry run (mara-a3aq0n) runs `pnpm run checks` on `ubuntu-22.04` and `ubuntu-22.04-arm` without caches (DES-RELEASE-WORKFLOW). Run https://github.com/convesoft/mara/actions/runs/45129288351 on `d185c3c` failed `Checks (ubuntu-22.04)` twice, each time one test of this family, a different one each attempt; arm64, CI's `ubuntu-24.04` images and a local run pass:

- Attempt 1: `inputs::tests::an_older_scan_keeps_a_file_a_newer_one_added`, `inputs.rs:1982`, `left: []`, `right: ["src/lib.rs"]`: the file rewritten with contents of the same size was not read again.
- Attempt 2: `inputs::tests::a_code_file_that_left_is_forgotten`, `inputs.rs:1919`, `left: None`, `right: Some(0)`: after `.mara/config.toml` was rewritten, `code_file("src/e.rs")` did not see the new configuration.

- Attempt 3: three of them, `a_code_file_that_left_is_forgotten`, `an_older_scan_keeps_a_file_a_newer_one_added` and `an_older_scan_does_not_bring_back_a_file_a_newer_one_dropped` (`inputs.rs:1960`, `left: ["src/lib.rs", "src/other.rs"]`, `right: ["src/lib.rs"]`: a deleted file was kept). So the failures are systematic on that image, not rare; the release cannot pass its Checks there until this is fixed.

Ubuntu 22.04 is the oldest platform that DES-PACKAGES supports, so whether the product misses changes there matters, not only the tests.

Likely cause, unconfirmed: with a clock an hour ahead, every stat counts as settled, so a rewrite in the same coarse timestamp tick as the previous check, of the same size, is invisible; with the real clock such a file is unsettled and read again. Whether the second failure has the same cause is not shown.

- [ ] Find the cause on `ubuntu-22.04` x64 (for example a job that runs `cargo nextest run -p mara --lib inputs::` in a loop on that image) and say whether the product or only the test is wrong
- [ ] Fix it; the tests pass repeatedly on that image
