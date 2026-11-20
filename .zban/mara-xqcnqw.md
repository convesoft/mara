---
title: Sessions, watching and concurrency
status: todo
type: feature
parent: mara-0mxw21
blocked_by:
  - mara-17gykj
created_at: 2026-11-13T22:10:06Z
---

Context: docs/daemon.mara.md (REQ-NEVER-STALE, DES-DAEMON-SESSIONS); REQ-READS-DURING-VALIDATION in docs/performance.mara.md.

- [x] Decide new-file detection, the idle period and session eviction (DES-INPUTS-AND-KEYS, DES-IDLE-SESSIONS)
- [ ] Idle sessions are dropped, and the daemon exits with its last session (DES-IDLE-SESSIONS)
- [ ] One session per canonical project root
- [ ] Watching with debounce and full rescan on overflow; stat check on every request
- [ ] Lock-free reads, per-project write queue, cancellable worker threads
- [ ] Test: VER-NEVER-STALE, its idle-drop case with `MARA_TEST_IDLE_SECONDS` included (ADR-TEST-IDLE-PERIOD), and the release-build case of DES-PERFORMANCE-HARNESS for that variable; the other cases of VER-DAEMON-WRITES (docs/verification-daemon.mara.md)

## Note from mara-17gykj

The daemon blocks SIGINT, SIGTERM and SIGHUP in every thread and takes them on one thread with `sigwait` (`crates/mara/src/cli/daemon/server.rs`). A child process inherits that mask, also through `std::process::Command`. So when the daemon runs rule evaluations, the function that starts Soufflé must reset the child's signal mask (in its `pre_exec`). Shutdown must also end a running evaluation through the evaluation's own cancellation: the daemon's blocked signals never reach the `signal-hook` handlers of mara-bx82k0 (`crates/mara/src/rules.rs`, `interrupt`).

## Note from mara-jkabqy

- Where a session plugs in: `client::answer` (`crates/mara/src/cli/client.rs`) builds a fresh `WorkCounts` and `Context` for each `operation` request and runs the operation's function on the files as they are. A session replaces that: it selects the project from the request's `cwd` and `project`, runs the request against its snapshot and returns the session's counts since its previous response (DES-PERFORMANCE-HARNESS). The request reaches it from `Daemon::answer` (`cli/daemon/server.rs`), which also updates the idle clock (`touch`).
- The daemon now runs in `/` (`detach`). A session opened at start (DES-DAEMON-LIFECYCLE: the project that `--project` names or that holds the start's working directory) must be resolved before `detach`. A client passes `--project` made absolute, and runs `daemon start` in its own working directory.
- Soufflé children now get an empty signal mask (`limit` in `crates/mara/src/rules.rs`), so the mask part of the note from mara-17gykj is done; cancelling evaluations at shutdown is still yours.
- Session assertions that `crates/mara/tests/daemon/` leaves to this issue, because until sessions every daemon response has the counts of a cold request: `mara daemon status` lists the project's session after the first on-demand read (`client::the_first_read_runs_in_process_and_starts_a_daemon_that_serves_the_next`); the count-based proofs of VER-DAEMON-START that a call was served in-process or by the daemon, and its rule that the project's files last changed at least 2 s before the first call. The tests prove routing meanwhile by stopping the daemon with SIGSTOP (`Test::served_by`, `Test::in_process`). They compare daemon-served stats lines with those of direct mode, and the MCP four-call case expects 4 × the cold counts; both change once warm requests report less.
- `stop` and SIGTERM still wait for every running request; ending reads early while writes finish (DES-DAEMON-LIFECYCLE, VER-DAEMON-WRITES stop case) is yours.

## Moved from the review of mara-jkabqy

- [ ] A client never waits without bound on a daemon that stopped answering. Today `crates/mara/src/cli/client.rs` removes the read timeout once the request is sent, so a stopped or hung daemon blocks `mara item list` and an MCP call for ever, and connecting has no timeout either (`Runtime::connect`; a socket with a full backlog). A long operation is not cut off while the daemon is alive and working; silence past a bound ends the wait with a diagnostic; after the request was sent in full the client reports `daemon-lost` and never runs it again; before that it falls back to direct mode. Note: in the review an incomplete send fell back only after 20.5 s, so the 10 s socket write timeout (`SEND_TIMEOUT`) is not a total deadline.
