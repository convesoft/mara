---
title: A session keeps the outcome of `query report`
status: done
type: bug
parent: mara-0mxw21
discovered_from: mara-p27z9t
created_at: 2026-11-21T12:38:04Z
closed_at: 2026-11-21T15:29:14Z
---

Goal: the row "The same read request again, inputs unchanged" of VER-WORK-COUNTERS (docs/verification.mara.md) holds for `query report`: 0 program runs and 0 subprocess spawns when the same report is sent again to a daemon on unchanged inputs.

Context: found by the 1× warm-row tests (mara-p27z9t, `crates/mara/tests/performance/warm.rs`, `the_same_read_request_again_does_no_work`). On the 1× corpus, `query report implementation-ready --id <retired requirement>` sent twice counts `program_runs: 1, subprocess_spawns: 1` the second time. `rules::consume_explained` (crates/mara/src/rules/consumer.rs) runs `rules::explain` in provenance mode on every request; the session's store (`session::Evaluated`, DES-DAEMON-SESSIONS) keeps only `Result<Rows, Fault>` by evaluation key, and the explanation also depends on the targets that `select` picks from the rows. Not fixed in mara-p27z9t: it needs a second kind of kept outcome, keyed by the evaluation key and the selected targets, across `rules.rs`, `rules/consumer.rs` and `session.rs`.

- [x] Keep explained outcomes per session (key: evaluation key, Soufflé build, selected targets; within the store's budget), never a stop at a limit or a fault
- [x] Remove the `report` exclusion in `warm::the_same_read_request_again_does_no_work`
- [x] Record the decision in DES-DAEMON-SESSIONS

## Summary of Changes

- `session::Evaluated` keeps a second kind of outcome, that of a run in provenance mode (`Evaluated::explain`, read by `rules::consume_explained` in a session), in the same store: one order, one 64 MiB budget, 256 bytes per entry plus rows, rows explained and explanations, the oldest dropped first, runs without holding the store. Keyed by the evaluation key and the Soufflé build (`Build`), and matched by the rows explained: the request's `select` runs on the kept run's rows and the kept run whose targets equal them answers it.
- Only a settled run is kept (`Explained::settled`, from the new `Explainer::settled` in the engine: every row has the explanation or reason its exchange gave), or a compile error; never a run that left a row unexplained at the time limit, an abort, a stray or invalid reply or the 16 MiB bound, nor a limit, runtime or host fault.
- Direct mode unchanged (one run per `query report`).
- DES-DAEMON-SESSIONS records which explained outcomes are kept and what their key covers.
- Tests: `warm::the_same_read_request_again_does_no_work` now asserts both `query report` cases (all eight counters, 0 runs, 0 spawns); `warm::a_report_after_an_edit_to_its_proof_runs_again` (moved lines: 1 run and new lines; restored: 0 runs, the first output); `warm::a_report_of_other_rows_or_inputs_runs_its_program` (another item: 1 run; both again: 0 runs, same output; `--limit 1`: 0 runs; gate report without `--complete` or for another candidate: 1 run); `session::tests::{an_explained_outcome_is_found_for_the_rows_that_the_request_explains, explained_outcomes_share_the_budget_with_evaluations}`; settled assertions in the engine's `a_reply_that_is_no_proof_or_comes_unasked_ends_the_explanations`. `daemon::parity` passes with the store in use: the MCP run of the `passed` report was answered from the store (shown by temporarily altering store hits: parity then failed at that report's "MCP through the daemon", so the later reports were not observed); the refused report of the `listed` query ends before any evaluation. The warm tests, not parity, assert the hit.
- Checks: fmt, clippy (warnings denied), `cargo test --workspace --locked` (803 passed, 13 ignored), `project validate` (0 errors, the pre-existing warning in docs/distribution.mara.md), release performance test (28 passed, 2 ignored), wasm32 build, `zban check`: all exit 0.

## Decisions made

- An explained outcome is kept when it is settled (every chosen row's explanation, or its reason, came from the exchange) or is a compile error; an outcome with a row left unexplained by the time limit, an abort, a stray or invalid reply or the 16 MiB bound is not. A stray reply after every row was explained leaves the outcome settled. DES-DAEMON-SESSIONS.
- Kept by evaluation key and Soufflé build, matched by the rows explained, which the request chooses from the kept run's rows: the targets are compared, not the inputs of `select` (entry, item), so nothing `select` depends on can be left out of the key. Rejected: hashing the entry and item into a report key (a field added to an entry later could be missed). DES-DAEMON-SESSIONS.
- What else an explanation depends on is in the evaluation key (facts with their paths and lines through the snapshot key, supplied inputs, program, limits) or fixed by the daemon's build (`MAX_NODES`, `MAX_PROOF_BYTES`); page options choose no part of the outcome and are not in the key. DES-DAEMON-SESSIONS.
- Two entries of one program share the evaluation key; when they choose the same rows they share the kept run, and each judges its outcome and roles from the rows itself. DES-DAEMON-SESSIONS.
- Explained outcomes are kept apart from evaluations: no other consumer reads a provenance run's rows.

## Review fixes (review of `e5ea869`)

- A run is settled, and kept, only when every chosen row was settled by the exchange and the run then ended as a successful run: no limit (the host's kill at the deadline included), a successful exit, nothing on its standard error after its first reply (`rules::explain::settled`). The response is unchanged: proofs completed before a stop still show. Test: `rules::explain::tests::a_run_is_settled_only_when_every_row_was_explained_and_it_ended_successfully` (a filled run ended by the deadline, a filled run with an unsuccessful exit, one with an abort's message: not settled); `session::tests::an_explained_outcome_is_found_for_the_rows_that_the_request_explains` shows an unsettled run is not kept.
- `warm::a_report_after_an_edit_to_its_proof_runs_again` asserts the lines the proof's fact leaves cite in the edited document: each 2 more after the edit, the first ones after the restore (the item's summary is not counted).
- DES-DAEMON-SESSIONS: the store compares the Soufflé executable's path, size and modification time (a replacement with all three the same is not told apart); a run stopped at a limit or failed after its last reply is not kept; a stray reply after every chosen row was explained leaves the run kept.
- The parity sentence of the summary above corrected.
- Checks at `36623b3`, without `MARA_NO_DAEMON` in the environment: fmt, clippy (warnings denied), `cargo test --workspace --locked` (804 passed, 13 ignored), `project validate` (0 errors, the pre-existing warning in docs/distribution.mara.md), release performance test under `berth --exclusive` (28 passed, 2 ignored), wasm32 build, `zban check`: all exit 0.

## Decisions made (review fixes)

- By the orchestrator for the maintainer: a run in provenance mode stopped at a limit, or that exited unsuccessfully or printed on its standard error after its first reply, is not kept, even when every chosen row has its proof. DES-DAEMON-SESSIONS.
