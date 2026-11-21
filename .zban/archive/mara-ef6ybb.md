---
title: "Daemon housekeeping: watcher lock, one parse per frame, lock file name, WorkCounts sum, caller umask and mode"
status: done
type: task
parent: mara-0mxw21
created_at: 2026-11-20T20:22:51Z
closed_at: 2026-11-21T13:28:46Z
---

Goal: seven small defects of the daemon's plumbing and of the caller's identity are gone.

Context: Found by the maintainer's code audit of 2026-11-20 (at `a5f8ec0`, then `a5f8ec0..c6a9706`), filed at the maintainer's request. Locations were checked at `c6a9706`. They are grouped because each is small and they touch neighbouring code; split one off if it grows. Items: DES-DAEMON-SESSIONS and DES-DAEMON-LIFECYCLE (docs/daemon/index.mara.md), REQ-READS-DURING-VALIDATION and DES-PERFORMANCE-HARNESS (docs/performance.mara.md).

- [x] `crates/mara/src/watch.rs`, `linux::Inner::sync`: the watcher's state lock is held while `symlink_metadata` and `inotify_add_watch` run for every directory, which blocks the thread that reads events and the `sync` and `forget` of other sessions. Make the system calls outside the lock, or keep the locked part short
- [x] `crates/mara/src/cli/daemon/server.rs:455`, `connection`: each frame is parsed with `serde_json::from_slice(&frame)` to look at `request` and at `client::writes`, thrown away, and parsed again in `daemon.answer(&frame)`. Parse once and pass the value
- [x] `crates/mara/src/daemon.rs`: `create_lock` names its temporary file by PID only, so two threads of one process can collide; `write_frame` copies the whole payload into a second `Vec` where it could write the 4-byte length and then the payload
- [x] `crates/mara/src/session.rs` and `crates/mara/src/cli/client.rs` each add the eight fields of `WorkCounts` by hand: one `AddAssign` (or an `add` method) on `WorkCounts` in `mara-engine` replaces both
- [x] `crates/mara/src/caller.rs:31-45`, `Caller::of_this_process`: it reads the umask by setting it (`libc::umask(0o077)`) and setting it back, which changes process-wide state twice and races with any other thread that creates a file meanwhile, for example when Mara runs inside a multi-threaded host such as the Node library of alpha.6. On Linux, read `Umask:` from `/proc/self/status`; say what other platforms do
- [x] `crates/mara/src/caller.rs:54-64`, `serve`: when it ends it sets the thread's `SERVED` to `None` where it should restore the caller that was there before, so a nested `serve` loses the outer caller
- [x] `crates/mara/src/caller.rs:114-127`, `write`: it asks `fs::symlink_metadata(path).is_ok()` and then opens with `create(true)`, and decides from the first answer whether to set the file's permissions; a file created or removed between the two gets the wrong decision. Decide from the open itself (`create_new`, then fall back to opening the existing file)

Done when the items are done, tests cover the ones with behaviour (the watcher lock, the lock file name), and the local checks pass with unchanged work counts.

## Summary of Changes

- `watch.rs`, `linux::Inner::sync` (reworked after review, as the orchestrator decided): `symlink_metadata` runs without the lock. Each change of a watch, `inotify_add_watch` with its record or `inotify_rm_watch` with the removal of its record, is one step under the state lock, which is released before the next, so the lock is held for one system call at a time. Before each step the sync checks that its session is still the one registered for the root (`Weak::ptr_eq`). A request's sync (`claim`) registers its session; a background sync (`sync_root`) whose session was forgotten or replaced stops without changing anything more. Because every descriptor is recorded in the same step that adds it, nothing is left to clean up and no removed descriptor is ever recorded. Debug-only pause point `MARA_TEST_PAUSE_WATCH` sits before the directory loop, where the lock is not held.
- `cli/daemon/server.rs`, `connection`: each frame is parsed once (`parse`), and the `Result` is passed to `Daemon::answer` and `answer_working`. The `served` count is untouched (still counted only for answered operation requests).
- `daemon.rs`: `create_lock` stages the lock under `<lock>.<pid>.<n>`, `n` from a process-wide counter (`staged_lock`). `write_frame` writes the 4-byte length, then the payload; `frame` became `frame_length`. After review: `send_frame` ends a connection for writing (`shutdown(Write)`) when a frame fails, and the server (refusals, answers, heartbeats) and `mara daemon` use it. The client's `send` writes both parts within its deadline and also shuts down for writing on failure.
- `mara-engine/src/work.rs`: `impl AddAssign<&WorkCounts> for WorkCounts`; `Session::add`, `client::add` (removed) and its callers in `client.rs` and `cli/mcp.rs` use `+=`.
- `caller.rs`: the umask is read from the `Umask:` line of `/proc/self/status` on Linux; after review the file is read as bytes (`umask_in`), so a task name that is not UTF-8 no longer falls back; set-and-restore stays where that is missing and on macOS, behind a comment that names the race. `serve` restores the caller it replaced. `write` calls `create_new` and opens the existing file on `AlreadyExists`, so the mode decision comes from the open.
- Tests: `watches::a_sync_paused_at_its_system_calls_keeps_the_watcher_working` (daemon test, debug builds: a read's sync is paused, an edit made meanwhile is reported by `MARA_TEST_DERIVED`; fails when the pause is moved under the lock, checked by mutation); `daemon::tests::threads_of_one_process_create_the_lock_at_once`; `caller::tests` for the umask read, nested `serve`, and `write` modes.
- Docs: DES-DAEMON-LIFECYCLE (lock staging name; how a client reads its umask per platform; after review, `caller::write` failing with not-found on a dangling link or a file removed between its opens), DES-DAEMON-SESSIONS (what the watcher's lock covers), ADR-DEBUG-PAUSE-POINT (the new pause point).

Checks, after merging the integration tip `d6bcf9d`: the seven local checks in AGENTS.md. Work counts unchanged: the performance tests (debug and release) and `daemon/warm.rs` pass with no expected number edited.

## Decisions made

- Umask on platforms without `/proc/self/status`: macOS offers no way to read the umask without setting it, so it keeps set-and-restore with the race named in a comment and in DES-DAEMON-LIFECYCLE. Linux without `Umask:` (before 4.7, no `/proc`) falls back to the same.
- `caller::write` on a dangling symbolic link, or a file removed between the two opens, fails with `NotFound` instead of creating the target: the open decides the mode, and its only callers write fresh names in a private directory.
- The watcher-lock test uses a new debug-only pause point, `MARA_TEST_PAUSE_WATCH` (recorded in ADR-DEBUG-PAUSE-POINT), because no existing event falls inside the sync.
- What watching still misses is recorded in DES-DAEMON-SESSIONS: a change in a directory before its watch is added, and a directory replaced between its lookup and its watch, which is watched again at the next sync. Watching only saves work (REQ-NEVER-STALE).

Filed: mara-es7j9r (deferred backlog): `forget` is keyed by root and can remove a newer session's watches. It predates this issue and concerns `forget`, not the lock.

## Review round (at `568cbe4`)

- Watcher: the two orders the review found are ruled out by doing the add-and-record (and remove-and-unrecord) as one step under the lock, with the ownership check in the same step. (1) An old background sync that resumes after its session was forgotten and the project reopened finds a different `Weak` registered, so it adds, removes and installs nothing. (2) A shared descriptor is recorded in the same locked step that adds it, so another session's forget sees it in the roots of that wd and keeps the kernel watch; a descriptor is removed only together with its last record, so no sync records a removed one. Tested by `watches::a_sync_paused_at_its_system_calls_keeps_the_watcher_working`, which still fails when the lock is held across the pause point (checked by mutation). The reopen order has no test of its own: the pause point stops every sync, so holding the old sync while the reopened session's sync runs would need a selective pause.
- Cut frame: `daemon::tests::a_frame_cut_after_its_length_ends_the_connection_for_writing`: after a frame times out after its length, the next send fails with `BrokenPipe` and the reader sees `UnexpectedEof`. Without the shutdown it fails with `WouldBlock`, checked by mutation.
- Umask: `caller::tests::the_umask_is_read_from_a_status_whose_name_is_no_utf8`.
- Still open: mara-es7j9r, the reverse order where the forget of an old session removes the watches of a newer session of the same root. It predates this issue.

## Re-check round (at `26dd9b2`)

- The two orders the re-check found remaining in the replacement of a watch (a leaked watch when an ancestor is replaced between a sync's remove and add steps while another sync of the root runs; a removal missed by a sync's `gone` list taken at its start) went to mara-es7j9r, as the orchestrator decided. DES-DAEMON-SESSIONS now says that a watch replaced during another sync of the same root can be left unreleased.
- `cli.rs`: the comment at `Caller::of_this_process` now says the umask is set for a moment only where `/proc/self/status` cannot give it, as on macOS.
