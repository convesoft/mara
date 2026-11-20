---
title: Sessions, watching and concurrency
status: done
type: feature
parent: mara-0mxw21
blocked_by:
  - mara-17gykj
created_at: 2026-11-13T22:10:06Z
closed_at: 2026-11-20T13:43:41Z
---

Context: docs/daemon.mara.md (REQ-NEVER-STALE, DES-DAEMON-SESSIONS); REQ-READS-DURING-VALIDATION in docs/performance.mara.md.

- [x] Decide new-file detection, the idle period and session eviction (DES-INPUTS-AND-KEYS, DES-IDLE-SESSIONS)
- [x] Idle sessions are dropped, and the daemon exits with its last session (DES-IDLE-SESSIONS)
- [x] One session per canonical project root
- [x] Watching with debounce and full rescan on overflow; stat check on every request
- [x] Lock-free reads, per-project write queue, cancellable worker threads
- [x] Test: VER-NEVER-STALE, its idle-drop case with `MARA_TEST_IDLE_SECONDS` included (ADR-TEST-IDLE-PERIOD), and the release-build case of DES-PERFORMANCE-HARNESS for that variable; the other cases of VER-DAEMON-WRITES (docs/verification-daemon.mara.md)

## Note from mara-17gykj

The daemon blocks SIGINT, SIGTERM and SIGHUP in every thread and takes them on one thread with `sigwait` (`crates/mara/src/cli/daemon/server.rs`). A child process inherits that mask, also through `std::process::Command`. So when the daemon runs rule evaluations, the function that starts Soufflé must reset the child's signal mask (in its `pre_exec`). Shutdown must also end a running evaluation through the evaluation's own cancellation: the daemon's blocked signals never reach the `signal-hook` handlers of mara-bx82k0 (`crates/mara/src/rules.rs`, `interrupt`).

## Note from mara-jkabqy

- Where a session plugs in: `client::answer` (`crates/mara/src/cli/client.rs`) builds a fresh `WorkCounts` and `Context` for each `operation` request and runs the operation's function on the files as they are. A session replaces that: it selects the project from the request's `cwd` and `project`, runs the request against its snapshot and returns the session's counts since its previous response (DES-PERFORMANCE-HARNESS). The request reaches it from `Daemon::answer` (`cli/daemon/server.rs`), which also updates the idle clock (`touch`).
- The daemon now runs in `/` (`detach`). A session opened at start (DES-DAEMON-LIFECYCLE: the project that `--project` names or that holds the start's working directory) must be resolved before `detach`. A client passes `--project` made absolute, and runs `daemon start` in its own working directory.
- Soufflé children now get an empty signal mask (`limit` in `crates/mara/src/rules.rs`), so the mask part of the note from mara-17gykj is done; cancelling evaluations at shutdown is still yours.
- Session assertions that `crates/mara/tests/daemon/` leaves to this issue, because until sessions every daemon response has the counts of a cold request: `mara daemon status` lists the project's session after the first on-demand read (`client::the_first_read_runs_in_process_and_starts_a_daemon_that_serves_the_next`); the count-based proofs of VER-DAEMON-START that a call was served in-process or by the daemon, and its rule that the project's files last changed at least 2 s before the first call. The tests prove routing meanwhile by stopping the daemon with SIGSTOP (`Test::served_by`, `Test::in_process`). They compare daemon-served stats lines with those of direct mode, and the MCP four-call case expects 4 × the cold counts; both change once warm requests report less.
- `stop` and SIGTERM still wait for every running request; ending reads early while writes finish (DES-DAEMON-LIFECYCLE, VER-DAEMON-WRITES stop case) is yours.

## Moved from the review of mara-jkabqy

- [x] A client never waits without bound on a daemon that stopped answering. Today `crates/mara/src/cli/client.rs` removes the read timeout once the request is sent, so a stopped or hung daemon blocks `mara item list` and an MCP call for ever, and connecting has no timeout either (`Runtime::connect`; a socket with a full backlog). A long operation is not cut off while the daemon is alive and working; silence past a bound ends the wait with a diagnostic; after the request was sent in full the client reports `daemon-lost` and never runs it again; before that it falls back to direct mode. Note: in the review an incomplete send fell back only after 20.5 s, so the 10 s socket write timeout (`SEND_TIMEOUT`) is not a total deadline.

## Summary of Changes

- `crates/mara/src/session.rs` (new): a daemon's sessions, one per canonical project root (`Sessions`), each holding the project's `Inputs` and the work done since its previous response, which that response reports (`take_work`). A request enters its session (`Sessions::enter`), so it is not dropped while served; `drop_idle` drops sessions idle for the idle period. `derive_in_background` re-derives at most once at a time per session, again when a change came meanwhile.
- `crates/mara/src/inputs.rs`: `Inputs::shared` gives a request a view of the session's inputs. `project` and `generation` run on the session's inputs under their lock, then the view keeps what that check found (`state` is now an `Arc`, copied only while a view still holds it), so `basis` and `admits` answer for the contents the edit was computed from. Every request still checks every input on disk.
- `crates/mara/src/watch.rs` (new): one inotify instance per daemon (Linux, through `libc`) watches the directories the sessions' last checks listed, `.mara` and the schema's directory; a burst is debounced (50 ms quiet, at most 500 ms), then the session re-derives in the background and its watches follow its directories; an overflow or an error of the instance re-derives every session. Elsewhere no watching (mara-txbf8x).
- `crates/mara/src/cli.rs`: `Context` carries the request's session; `Context::inputs` replaces `Inputs::new` in every operation (`read`, `validate`, `relink`, `fix`, `author` through `read::open`).
- `crates/mara/src/cli/client.rs`: `answer` selects the project from `cwd` and `project`, enters its session, runs the operation there and reports the session's counts. Client deadlines: the connection (`Runtime::connect`, non-blocking, 2 s while the daemon's queue is full) and the send (10 s in all) fall back in-process; after dispatch, heartbeat frames keep the client waiting, and 15 s without a frame ends it with `daemon-lost`, never running the request again. The `daemon-lost` message reads "gave no answer to".
- `crates/mara/src/cli/daemon/server.rs`: `status` lists the sessions; `daemon start` opens the session of its project (resolved before the fork) and derives it in the background; each operation request runs on a worker thread while the connection's thread sends a heartbeat every second; idle sessions are dropped and the daemon exits with its last; `stop` and SIGTERM/SIGINT/SIGHUP cancel rule evaluations, let writes finish and exit, ending reads.
- `crates/mara/src/rules.rs`: `rules::cancel` ends every running and later evaluation of the process (child killed, private directory removed) and counts the running ones, which a stopping daemon waits for.
- `crates/mara/src/daemon.rs`: `frame`, `CONNECT_TIMEOUT`, `HEARTBEAT`, the non-blocking connection.
- Tests: `tests/daemon/fresh.rs` (VER-NEVER-STALE), `tests/daemon/deadlines.rs` (client deadlines), new cases in `tests/daemon/writes.rs` (VER-DAEMON-WRITES and the paused-move read) and `tests/daemon/caller.rs` (stop cancels an evaluation); `tests/daemon/client.rs` and `mcp.rs` prove routing by counts where VER-DAEMON-START does (status lists the session; a cold first daemon response for the opening; a repeated read reports nothing; the MCP four calls sum to 4 × cold), and test projects settle 2 s before their first call; `tests/performance/idle_period.rs` opens a session and finds it listed 3 s later in a release build. Mutations checked, each failing its tests: a session that skips the stat check (9 tests), exit without waiting for writes, no heartbeats, no evaluation cancel, no idle drop, one session for every root, a blocking connect, a per-write send timeout.
- Docs: DES-DAEMON-SESSIONS, DES-DAEMON-LIFECYCLE and DES-IDLE-SESSIONS hold the decisions below.
- Build: no new crate; the release binary grows from 10,367,568 to 10,601,048 bytes.
- Checks at the head, each exit 0: cargo fmt --check; clippy with warnings denied; cargo test --workspace --locked (no daemon of a test left, nothing created under `/run/user/1000/mara`, which did not exist before or after); project validate in direct mode (valid, 0 errors, 0 warnings); the release performance test; the wasm32 build; zban check.

Cases and their tests:

- VER-NEVER-STALE: documents edited in place, saved by rename, added, added in a new directory, removed, renamed: `fresh::every_change_of_a_document_is_seen_by_the_next_request`; schema, include patterns, `.gitignore`: `fresh::a_changed_schema_configuration_or_gitignore_is_seen_by_the_next_request`; documents' directory replaced: `fresh::a_replaced_documents_directory_is_seen_by_the_next_request`; program file edited (`project validate`, `query execute`), plus a supplied input file and a code file: `fresh::an_edited_program_input_or_code_file_is_seen_by_the_next_request`; V1/V2/V3: `fresh::a_validation_answers_for_the_files_it_read_and_the_next_for_the_files_after`; idle drop with `MARA_TEST_IDLE_SECONDS=4` and an MCP server: `fresh::idle_sessions_are_dropped_and_the_daemon_exits_with_the_last`; two copies: `fresh::two_copies_of_a_project_are_answered_each_from_its_own_files`; read during a paused move: `writes::a_read_while_a_move_waits_answers_from_the_files_as_they_are`.
- VER-DAEMON-WRITES: ten creates: `writes::ten_creates_sent_at_once_all_land`; another program saves, then an update: `writes::a_write_after_another_program_saved_keeps_what_it_saved`; target changed while waiting: `writes::a_write_whose_target_changed_while_it_waited_fails_and_changes_nothing`; `stop` and SIGTERM during a move: `writes::a_running_write_finishes_when_the_daemon_is_told_to_stop`, `writes::a_running_write_finishes_on_sigterm`; the SIGKILL cases (from mara-jkabqy): `writes::a_create_whose_daemon_is_killed_before_step_2_changes_nothing`, `writes::a_move_whose_daemon_is_killed_at_its_second_file_is_left_interrupted_and_reported`.

## Decisions made

- Watching: inotify through `libc`, one instance per daemon (inotify instances are limited per user), no new crate; a directory that cannot be watched is only checked by requests. Other systems do not watch yet (mara-txbf8x). DES-DAEMON-SESSIONS.
- Watched: the directories the last check listed, `.mara` and the schema's directory. Debounce 50 ms after the last change, at most 500 ms after the first. Overflow or error: every session re-derives. DES-DAEMON-SESSIONS.
- A background derivation does what a document read does (no code scan, no evaluation), one per session at a time; its work goes to the next response. A daemon started for a project derives its session in the background at once, so its first response reports the opening as a cold request. DES-DAEMON-SESSIONS, DES-DAEMON-LIFECYCLE.
- A request holds the session's inputs only while it checks and derives them, then keeps its own view; reads never wait for a write or an evaluation. The per-project write queue is the project's write lock, as between processes. DES-DAEMON-SESSIONS.
- A request that selects no project runs in no session and reports its own counts. DES-DAEMON-SESSIONS.
- Idle: the period runs from the start or end of a session's last request; a session serving a request is not dropped; the daemon exits once its last session is dropped and it serves no request. DES-IDLE-SESSIONS.
- Client deadlines: non-blocking connection retried for 2 s while the queue is full, then in-process; 10 s in all to send, then in-process; after dispatch a heartbeat frame (`working` true) every second, and `daemon-lost` after 15 s without a frame. The protocol keeps `version` 1: only one build talks to its own daemon. DES-DAEMON-LIFECYCLE.
- Stop: rule evaluations end through `rules::cancel` (child killed, directory removed) and the daemon waits for them and for its writes; other reads end with the process, and their clients get `daemon-lost`. DES-DAEMON-LIFECYCLE.

## Filed

- mara-8hw038: warm counts after a watched change and after a write (the 2 s rule after a background derivation, read-back parses kept, evaluations kept); for mara-p27z9t's rows.
- mara-txbf8x (backlog): watch inputs on macOS.
- mara-vday9w (backlog): concurrent in-process evaluations share one interrupt flag (`mara mcp --no-daemon`).

## Review fixes

- Requests told apart by identity, not time: each `Inputs` value or view has a request number from a process-wide counter that never repeats, and the configuration and schema are reused only within the request of that number (`Inputs::project_for`, `generation_for`); before, two requests with the same `now` (a clock that stands still) were answered from the configuration the first loaded. A request is a view, or the calls on a value kept by a host up to and including its next `generation`, which takes a new number. No other place took equal times for one request. Test: `inputs::tests::requests_with_the_same_time_each_check_the_configuration` (one session, a fixed `now`: changing `include` must change the next answer; then a clock stepped back an hour, and a value kept between requests with the same `now`); it fails when views share one request number.
- `stop` answered before the daemon stops: the `stop` request no longer sets `stopping`; the connection's thread sets it after it has sent the answer, so the process cannot exit first. A client whose connection closes after it asked for the stop reports `stopped: true` with the process ID it read from the lock before asking. Tests: `the_daemon_answers_stop_before_it_exits` (50 starts and raw `stop` requests through the socket, each answered before end of file; failed 3 of 3 runs without the fix) and `stop_reports_a_daemon_that_ends_before_it_answers` (`stop` to a stopped daemon, then SIGKILL; failed without the fix).
- Watches follow replaced directories: a session's watch records the directory's device and inode; each sync replaces a watch whose path now names another directory, or that the kernel ended; `IN_IGNORED` forgets the watch, and `IN_DELETE_SELF` or `IN_MOVE_SELF` removes it, then re-derives the session, whose sync watches what the path names now. Test: `watches::watches_follow_a_directory_moved_away_and_copied_back_or_created_again` (reads the daemon's inotify `fdinfo`; fails when a remembered path is trusted).
- The session that `daemon start` opens is registered with the watcher before its first derivation (`Watcher::derive` registers), so its watches exist without a request. Test: `watches::daemon_start_watches_its_project_before_any_request` (the project root, `.mara` and `docs` watched; fails without the registration).
- Docs: DES-DAEMON-SESSIONS (identity of requests, watches on inodes and their replacement, the started session watched from its first derivation), DES-DAEMON-LIFECYCLE (`stop` answered before the daemon stops; the client's report).
- Filed to the backlog: mara-wrtqnd (a request run without a worker thread sends no heartbeats).

- Rebased onto the integration branch with mara-tx0fcf, mara-ptq6xc and mara-jm5ecb. The provenance run of `query report` (`rules/explain.rs`) also counts as a running evaluation (`rules::cancel::Running`), so a stopping daemon waits until its sandboxed Soufflé is killed and its directory removed; the cancellation check sits in the shared `spawn_and_wait` loop. Tests: `caller::stop_ends_a_running_evaluation_and_leaves_no_souffle` (query execute) and `caller::stop_ends_a_running_report_and_leaves_no_souffle` (query report; failed 3 of 3 runs without the count), both with the sandboxed child.

- A read that ends while the daemon stops is not answered (its evaluation may have been cancelled), so its client reports `daemon-lost` and never a fault made by the shutdown; writes are still answered. Found by the repeated runs: `caller::stop_ends_a_running_report_and_leaves_no_souffle` failed 2 of 10 runs under load with the answer "program-fault: … the daemon stops".

- Test cleanup: in repeated loaded runs a test daemon was found running after the run ended, launched by a client's on-demand `daemon start` after its test had removed its project (no session, so it would have lasted its 30-minute idle period). `Daemons::kill` (`tests/support/daemons.rs`) now returns only after no daemon of the test has run for 500 ms, and a test kills its daemons before its directory is removed and again after. After that, 10 loaded runs of the daemon target left no process.

- `client::code_operations_through_the_daemon_match_direct_mode` (from mara-s9gak4) compares every request after the session's first with direct mode but for its counts: a warm session reports less work for the same output.

## Rebase onto mara-hk2arx (`923d9ae`)

- `Inputs::scan_listed` (mara-hk2arx: the code files a program over code links reads, after a `CodeReads::List` generation) works on a session's view: it runs on the session's inputs under their lock, as `project` and `generation` do, and the view then keeps the result. The code files a program reads are therefore inputs of the session like the documents, checked on disk on every request: read again only when their stat changed, scanned again only when contents, language or grammar changed. Its own value takes `state` out of its `Arc`, copying it only while a view still holds it. `item validate`'s conflict resolved to `context.inputs(root)` with mara-hk2arx's `CodeReads::List`.
- `client::a_query_over_code_links_gives_the_rows_of_direct_mode_after_a_marker_changes` (mara-hk2arx) compares the session's warm requests with direct mode on everything but the counts.
- New test `fresh::every_change_of_code_is_seen_by_the_next_request_over_code_links`: a query, a report and an item validation over code links equal direct mode after a marker changed, a code file added and removed, `[code]` changed and restored, and a code file edited twice within one second, at once and 2 s later; then, on settled files, a repeated request reports 0 file reads, hashes, parses, snapshot builds and code scans. It fails when the session's checks keep a stale file, and when `scan_listed` scans without the session's tracking (1 read, 1 hash, 1 code scan on a warm request).
- Grammars loaded and dropped are process-wide (mara-s9gak4): neither the idle drop nor shutdown waits on a grammar thread; shutdown waits only for writes and rule evaluations.
- Docs: DES-DAEMON-SESSIONS (code files a request reads are session inputs).

## Review of the rebase over mara-hk2arx (`d5b7a30`)

- Code scans hold no lock of the session. A view lists the code files under the session's inputs lock, then reads, loads grammars and parses without it (`Inputs::scan_listed`, `scan`). Under the lock again it gives back only results whose file still has the stat it was read with (`give_back`, `Tracked::still_as_read`), so a scan that a change overtook is not kept and the next check reads that file again; the view keeps its own results, which an edit's basis comes from. `generation` with `CodeReads::Scan` (`project validate`, edits that read code) is now the listing plus that scan, in direct mode as in a session, so both give the same results by construction; the work counts are unchanged.
- Tests:
  - `client::a_read_is_answered_while_a_program_scans_code` (`item validate` with an invariant over code links) and `client::a_read_is_answered_while_project_validate_scans_code`: with the grammar stalled on a file for 10 s (`MARA_TEST_GRAMMAR_STALL`, `MARA_TEST_GRAMMAR_BUDGET_MS`), an `item list` sent once the grammar's thread exists returns while the scanning request still runs and within half the stall. With the previous scan the read waited 21 s and both fail.
  - `inputs::tests::a_scan_that_a_change_overtook_is_not_kept`: a scan whose file changes before it gives back leaves the session's record as it was, and the next request reads the new contents; it fails when every result is given back.
  - `fresh::every_change_of_code_is_seen_by_the_next_request_over_code_links` gains the two sequences of this merge: a program without code links, a code file changed, then a program over code links; and code read, changed while only programs without code links and a document read are requested, then the program over code links again.
- Stat limit (decided by the orchestrator, no code): DES-INPUTS-AND-KEYS states that a write through a writable shared memory mapping not yet flushed or unmapped can leave the compared stat unchanged until the kernel updates the timestamps. Backlog: mara-v0qcvb.
- Docs: DES-DAEMON-SESSIONS (scans outside the lock, results given back only while current).
