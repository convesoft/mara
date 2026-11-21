---
title: A session keeps the outcome of `query report`
status: todo
type: bug
parent: mara-0mxw21
discovered_from: mara-p27z9t
created_at: 2026-11-21T12:38:04Z
---

Goal: the row "The same read request again, inputs unchanged" of VER-WORK-COUNTERS (docs/verification.mara.md) holds for `query report`: 0 program runs and 0 subprocess spawns when the same report is sent again to a daemon on unchanged inputs.

Context: found by the 1× warm-row tests (mara-p27z9t, `crates/mara/tests/performance/warm.rs`, `the_same_read_request_again_does_no_work`). On the 1× corpus, `query report implementation-ready --id <retired requirement>` sent twice counts `program_runs: 1, subprocess_spawns: 1` the second time. `rules::consume_explained` (crates/mara/src/rules/consumer.rs) runs `rules::explain` in provenance mode on every request; the session's store (`session::Evaluated`, DES-DAEMON-SESSIONS) keeps only `Result<Rows, Fault>` by evaluation key, and the explanation also depends on the targets that `select` picks from the rows. Not fixed in mara-p27z9t: it needs a second kind of kept outcome, keyed by the evaluation key and the selected targets, across `rules.rs`, `rules/consumer.rs` and `session.rs`.

- [ ] Keep explained outcomes per session (key: evaluation key, Soufflé build, selected targets; within the store's budget), never a stop at a limit or a fault
- [ ] Remove the `report` exclusion in `warm::the_same_read_request_again_does_no_work`
- [ ] Record the decision in DES-DAEMON-SESSIONS
