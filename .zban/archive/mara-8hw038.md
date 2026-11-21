---
title: Warm counts after a watched change and after a write
status: done
type: task
parent: mara-0mxw21
related:
  - mara-p27z9t
discovered_from: mara-xqcnqw
created_at: 2026-11-20T13:42:55Z
closed_at: 2026-11-21T10:23:27Z
---

Goal: the warm rows of VER-WORK-COUNTERS that follow a change hold against the daemon (mara-p27z9t checks them).

Context: found while building sessions (mara-xqcnqw). DES-DAEMON-SESSIONS, DES-INPUTS-AND-KEYS, DES-PERFORMANCE-HARNESS.

- The watcher re-derives about 50 ms after a change, so its check comes less than 2 s after the file's mtime and does not settle it: the next request reads and hashes that file again. The rows "One document edited …" and "The request after a write …" also run with the request sent 2 s after the watcher re-derived and must give the same counts, so the session reads the file twice where the row allows once.
- A write served by a session does not hand its read-back parses to the session, so the request after it parses the written files again, where the row "The request after a write" expects 0 parses.
- No evaluation is kept between requests, where the row "A request whose evaluations an earlier request ran with the same keys" expects 0 program runs.

- [x] Decide how a session counts an unsettled file that the watcher reports unchanged since its check (for instance: trust a watch that was in place before the check), and record it in DES-DAEMON-SESSIONS
- [x] A write served by a session leaves its read-back parses and the written files' stats in the session
- [x] Evaluations kept per session by key (DES-RULE-EXECUTION), within the memory the daemon allows

## Progress (worker, first pass)

- Done: a write served in a session hands its read-back parses and post-rename stats to the session and derives the snapshot (1 hash per file written, no read, no further parse); the background derivation no longer re-reads a file whose stat is unchanged and that it would still leave unsettled; evaluations are kept per session by key (rows and compile errors only, at most 64 MiB, oldest dropped first). Rows that now hold: the write rows, "The request after a write" (at once and 2 s later), "A request whose evaluations an earlier request ran with the same keys", "One program file edited, then `project validate`". Tests: `daemon::warm::*`, `session::tests::only_outcomes_that_the_key_decides_are_kept_within_the_budget`.
- Open, for the maintainer (first checklist item): "One document edited … sent 2 s later, after the watcher re-derived" still counts 2 file reads and 2 hashes. The watcher reads the changed file 50 ms after its mtime, which cannot settle it, so the request re-reads it. Meeting the row needs trusting the watcher (inotify misses same-stat edits through a hard link in an unwatched directory, from another machine on a network file system, through an mmap within the tick: a loosening of never-stale) or deferring the watcher's derivation until the file settles (contradicts the Accepted 50 ms debounce of DES-DAEMON-SESSIONS). Recommended: change the row of VER-WORK-COUNTERS to count the watcher's read too (2 reads, 2 hashes, 1 parse, 1 snapshot build for the 2 s variant). Recorded under `## Open questions` in docs/daemon/index.mara.md.

## Summary of Changes

- A write served in a session (`item create/update/delete`, `relation add/remove`, `item move/rename`, `project fix`) hands its read-back parses and post-rename stats to the session and derives the snapshot (`Inputs::written`; `WrittenFile.document` in the engine): 1 hash per file written, no read, no further parse.
- The background derivation does not re-read a file whose stat is unchanged and that a check now would still leave unsettled (`Inputs::rederive`, `Walk::check`).
- Evaluations kept per session by key (`session::Evaluated`, read through `Generation.evaluated` by `rules/consumer.rs` and `rules/invariants.rs`).
- VER-WORK-COUNTERS: the edited-document row counts 1 read and hash when no derivation ran first, 2 when the watcher's did (always in the 2 s run); the write row keeps the same counts in both runs. DES-DAEMON-SESSIONS records the decisions; the open question is removed.
- Tests: `daemon::warm::{an_edited_document_is_read_by_the_watcher_and_again_by_the_request_that_settles_it, a_write_leaves_its_parses_and_the_next_request_reads_only_the_written_file, a_request_reads_the_evaluations_that_an_earlier_request_ran}`, `session::tests::only_outcomes_that_the_key_decides_are_kept_within_the_budget`.
- Checks at the merge of `594e9a0`: fmt, clippy (warnings denied), `cargo test --workspace --locked`, `project validate` (0 errors, the pre-existing warning in docs/distribution.mara.md), release performance test (22 passed, 2 ignored), wasm32 build, `zban check`: all exit 0.

## Decisions made

- By the orchestrator for the maintainer: a file that the watcher's derivation read less than 2 s after its mtime or ctime is read and hashed again by the next request's check; a watch is never proof that a file is unchanged. VER-WORK-COUNTERS changed instead of the session. Rejected: trusting a watch in place before the check (loosens REQ-NEVER-STALE); deferring the derivation until files settle (loses the 50 ms warm start). DES-DAEMON-SESSIONS, VER-WORK-COUNTERS.
- A write served in a session counts 1 hash per file written and no read; its files count as checked when the request began, before the renames, so the next check reads them again. DES-DAEMON-SESSIONS.
- A background derivation skips re-reading an unchanged-stat file it could not settle. DES-DAEMON-SESSIONS.
- The evaluation store keeps rows and compile errors only (never limit, runtime or host faults), at most 64 MiB per session by estimate (provisional), oldest dropped first; evaluations run without holding the store. DES-DAEMON-SESSIONS.

## Rejected approaches

- Keeping `rules::Evaluations` across requests: its map is private to `rules.rs` (another worker's file) and cannot be bounded from outside.

## Review fixes (review of `b7eece1`)

- Soufflé identity in the store's key (`session::Build`: path, size, modification time), added by the host where it looks an outcome up, not in `evaluation::key`. Test: `session::tests::an_outcome_is_kept_for_the_souffle_that_gave_it`.
- Every kept outcome costs 256 bytes besides its rows. Test: `session::tests::outcomes_without_rows_count_against_the_budget`.
- The hand-over after a write keeps every input whose stat is unchanged without reading it (`tracked::Keep::Unchanged`). Test: `daemon::warm::a_write_reads_no_other_input_again_after_its_renames` (fails without: 3 reads, 4 hashes).
- A write's window: `Inputs::apply` opens `session::Deriving::window` before `write::apply` and closes it after the hand-over; the background derivation waits for it (`Deriving::begin`), a write waits for a derivation in progress; reads never wait. Test: `daemon::warm::no_derivation_runs_between_a_writes_first_rename_and_its_hand_over` (fails without: a derivation with 1 read and 1 parse in the window).
- `Inputs::rederive` publishes the derivation's counts while it still holds the session's inputs.
- Tests wait for the watcher through `MARA_TEST_DERIVED` (debug builds only; the release-build check of the pause points looks for its name too), never for a deadline; the post-write test asserts the watcher's derivation read and parsed nothing.
- `ReadBack::knowing(snapshot)` takes the parse of a snapshot document with the same contents. Test: `daemon::warm::a_write_that_gives_a_file_the_contents_of_another_input_parses_nothing` (fails without: 1 parse).
- The background skip covers the configuration and the schema (`Reads::keep` in `tracked::check`). Test: `daemon::warm::the_watcher_reads_no_unsettled_schema_again` (fails without: 2 reads).
- VER-WORK-COUNTERS wording: the request after a write (both runs), kept outcomes, programs whose key an edit leaves unchanged.

## Decisions made (review fixes)

- By the orchestrator for the maintainer: the session's store tells outcomes apart by the identity of the Soufflé executable (path, size, modification time), as a daemon belongs to a build. DES-DAEMON-SESSIONS.
- Each kept outcome is estimated at 256 bytes plus its rows (provisional). DES-DAEMON-SESSIONS.
- The hand-over after a write keeps every other input whose stat is unchanged without reading it. DES-DAEMON-SESSIONS.
- A write's window runs from before its first rename (before `write::apply`'s checks) to its hand-over; no background derivation runs in it. DES-DAEMON-SESSIONS.
- The background skip covers the configuration and the schema. DES-DAEMON-SESSIONS.
- The read-back reuses the parse of a document of the edit's snapshot with the same contents, found by comparing contents (no hash, no index). Direct mode benefits alike.
- `MARA_TEST_DERIVED`, a debug-only report of background derivations for tests. ADR-DEBUG-PAUSE-POINT.
- VER-WORK-COUNTERS: "`project validate` or `query gate` after a document edit" counts no run for a program run against empty facts whose outcome the session keeps.
