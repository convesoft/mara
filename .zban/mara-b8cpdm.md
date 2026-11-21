---
title: Review the decisions workers made in alpha.4
status: todo
type: task
tags:
  - maintainer
blocked_by:
  - mara-0mxw21
created_at: 2026-11-14T11:14:21Z
---

Goal: the maintainer reviews what workers decided on their own in the alpha.4 slice.

Context: AGENTS.md (Deciding during the rewrite). When the alpha.4 epic (mara-0mxw21) closes, the orchestrator appends the `## Decisions made` list of each of its issues below. This issue blocks the release workflow, not the next slice. No worker starts it.

- [ ] Every listed decision accepted, or overturned by a follow-up issue

## Decided by the orchestrator

One line per decision made on the maintainer's behalf under the rule of 2026-11-16: what was decided, what it rests on, how to undo it.

- 2026-11-16, mara-bx63yb: a daemon belongs to a build (the version plus the executable's path, size and modification time), not just a version; ADR-DAEMON-PER-VERSION says so. Rests on REQ-SAME-RESULT-ON-EVERY-HOST and REQ-NEVER-STALE: a daemon started from one worktree must not answer for another worktree's binary of the same version. Undo: key on the version alone.
- 2026-11-16, mara-bx63yb: tests reach the end of the idle period through one debug-only variable, `MARA_TEST_IDLE_SECONDS`; a release build ignores it and does not contain it (ADR-TEST-IDLE-PERIOD). Rests on ADR-DEBUG-PAUSE-POINT, where the maintainer approved test-only code that exists in debug builds only. Rejected: a user-visible `--idle` option; inspection only. Undo: remove the variable and verify the idle drop by inspection.
- 2026-11-16, mara-bx63yb: the daemon gets scaling cases of its own (a settled warm request, a request after one document edit), with the daemon's CPU time measured. Rests on REQ-NEAR-LINEAR-SCALING. Undo: drop them and rely on the CLI-direct cases.
- 2026-11-16: mara-bx63yb was started before the alpha.3 epic closed, since it writes documents only. Undo: not needed.

- 2026-11-16: mara-17gykj (daemon process, IPC and lifecycle) was started before the alpha.3 epic closed, on a free worker slot: it needs nothing from the rule evaluation. The slices still close and are tagged in order. Undo: not needed.

- 2026-11-16: mara-jkabqy (client routing) was started before the alpha.3 epic closed, as mara-17gykj was; mara-xqcnqw follows it rather than running beside it, because both change the daemon's request handling. Undo: not needed.

- 2026-11-16, mara-jkabqy (review): a client's wait on a daemon that stopped answering gets its bound in mara-xqcnqw, which rebuilds request handling, not in mara-jkabqy; alpha.4 does not close without it. Rests on REQ-FIRST-CALL-IN-PROCESS and the next issue's scope (worker threads, cancellation). Undo: add a fixed read timeout to the client now.
- 2026-11-16, mara-jkabqy (review): the local checks of AGENTS.md and the workers' own `mara` calls run in direct mode (`--no-daemon` or `MARA_NO_DAEMON=1`), so that each worktree's build leaves no daemon in the developer's session. Undo: drop the flag from the check line.

- Orchestrator, 2026-11-20 (mara-xqcnqw, provisional): file watching exists on Linux only; on macOS every request still checks the disk (REQ-NEVER-STALE holds) and does the rebuild itself. Rests on REQ-NEVER-STALE and DES-DAEMON-SESSIONS. Undo: do backlog issue mara-txbf8x (an FSEvents or kqueue watcher) before the release.

- Orchestrator, 2026-11-20 (mara-xqcnqw, provisional; flagged to the maintainer because it touches REQ-NEVER-STALE): a file changed through a writable shared memory mapping that is not flushed or unmapped can keep every stat field the session compares, so the session serves the old contents until the kernel updates the timestamps. Accepted as the known limit of deciding 'unchanged' by stat (DES-INPUTS-AND-KEYS states it); backlog mara-v0qcvb. Undo: make mara-v0qcvb a normal issue.
- Orchestrator, 2026-11-20 (mara-xqcnqw): two simultaneous code-link requests with the same grammar parse one at a time, by mara-s9gak4's design (ADR-GRAMMAR-WATCHDOG-THREAD); the re-check marked it 'not working' against an over-asked point of the orchestrator's brief. Not blocking; backlog mara-vts1tz.
- Orchestrator, 2026-11-20: the rebase of mara-xqcnqw over mara-hk2arx got its own review and re-check at the maintainer's request; it does not count as a third round on the first review's fixes.

- Orchestrator, 2026-11-21 (mara-8hw038): the worker asked how a session counts a file that the watcher's derivation read less than 2 s after its mtime. Decided: VER-WORK-COUNTERS changes, the session does not. The row for one edited document counts the watcher's read too: 2 reads and 2 hashes of that document when the watcher's derivation ran before the request (always in the run sent 2 s later), 1 parse, 1 snapshot build, no other input. The row's "same counts" contradicted the row for an input last hashed less than 2 s after its mtime and the settle rule of DES-INPUTS-AND-KEYS. Rests on REQ-NEVER-STALE (not loosened) and REQ-WORK-FOLLOWS-CHANGE (still only the changed input is read). Rejected: trusting a watch that was in place before the check (inotify misses some edits that keep the stat); delaying the derivation to 2 s after the change (loses the warm start, against the 50 ms debounce). Undo: take one of the two rejected options and restore the row.

- Orchestrator, 2026-11-21 (mara-8hw038, review): the outcomes a session keeps of its evaluations are also distinguished by the Soufflé executable an evaluation would run (path, size, modification time), as a daemon belongs to a build; the engine's evaluation key stays without I/O. Rests on REQ-NEVER-STALE and DES-RULE-EXECUTION. Undo: drop the executable's identity from the store's key.
- Orchestrator, 2026-11-21 (mara-8hw038, review): pre-authorised for the worker: if the row 'a write that gives a file the contents of another input: no parse for that file' of VER-WORK-COUNTERS cannot be met by a small change (offering the session's parses to the read-back), the row changes to what the code does (1 parse per distinct content of the edit). REQ-WORK-FOLLOWS-CHANGE holds either way. The worker's closing summary says which it took. Undo: restore the row and build the lookup.

- Orchestrator, 2026-11-21 (mara-wz40n6, review; provisional, user-visible, for the maintainer to confirm): `mara daemon status` reports `served`, the number of operation requests the daemon has run and answered (`null` in JSON when no daemon runs; one sentence in the human form). The comparison test of VER-HOST-COMPARISON needs it to show for each request that the test's daemon answered it; nothing that existed could. Rests on REQ-SAME-RESULT-ON-EVERY-HOST, DES-DAEMON-LIFECYCLE, VER-DAEMON-START. Undo: remove the field and give the test another proof of routing (for example a debug-only one).

- Orchestrator, 2026-11-21 (mara-p27z9t): a file named by `--input <name>=@<file>` is read again by every request and the session keeps nothing of it; the rows 'The same read request again' and 'Another request, inputs unchanged' of VER-WORK-COUNTERS say 0 file reads but one of each file that `--input` names. Rests on REQ-NEVER-STALE and the cold rows (such a file is no input of the project). Rejected: the session tracks request-supplied files. Undo: track them and restore the rows.
- Orchestrator, 2026-11-21 (mara-p27z9t, review): the rows for a touched input count the watcher's read too, as the edited-document row does since mara-8hw038; an input last hashed less than 2 s after its mtime is read exactly once, by the request or by a derivation that checked 2 s or more after the change. The tests assert fixed numbers per case; the watcher's report only tells a test which case it is in.
- Orchestrator, 2026-11-21 (mara-p27z9t, review; provisional, for the maintainer, because it narrows REQ-WORK-FOLLOWS-CHANGE for one case): a scan keeps the contents only of code files with markers, so the first `get` of a code file without markers after a scan reads it once; the row 'Another request, inputs unchanged' carries the qualifier. Keeping every scanned file's contents would hold the whole code base in a session's memory. Undo: backlog issue mara-2ndg14.
- Orchestrator, 2026-11-21 (mara-p27z9t): `query report` sent again still runs its provenance evaluation; the row is right and the code is not, so it is issue mara-6za81b of this slice, not a changed row.

- Orchestrator, 2026-11-21 (mara-ef6ybb, review): the watcher changes each kernel watch and its record in one step under its state lock, one directory at a time, and checks under that lock that its session is still the one registered for the root; `symlink_metadata` runs outside the lock. Chosen over running all of a sync's watch calls outside the lock and recording them afterwards, which let an old sync take over a reopened session's watches and let a removed descriptor be recorded as live. What remains (a watch replaced while another sync of the same root runs and a directory is moved; `forget` keyed by root) is backlog issue mara-es7j9r; none of it can make a request answer from content older than the disk. Rests on REQ-NEVER-STALE and DES-DAEMON-SESSIONS. Also from this issue: on platforms where the umask cannot be read (macOS, Linux without `Umask:` in /proc/self/status) reading it still sets it for a moment, with the race named in DES-DAEMON-LIFECYCLE; a frame whose write fails ends the connection for writing.

## Decisions made by workers in alpha.4

Appended by the orchestrator on 2026-11-21, when the epic mara-0mxw21 closed: the `## Decisions made` sections of its issues, as their closing summaries hold them.

### mara-17gykj Daemon process, IPC and lifecycle

- The runtime directory's base must be absolute, a directory and no symbolic link, owned by the user with mode 0700, or every call runs in direct mode; `mara/` must be the same, or the `daemon` operations fail with `io-error` and change nothing there (stricter than the design, which named only a missing base).
- Lock `<key>.lock` and socket `<key>.sock`, `<key>` the first 16 hex digits of BLAKE3 over the build. The lock: regular file, mode 0600, opened with `O_NOFOLLOW`, held with `flock` for the daemon's life, holding its PID. The socket: mode 0600; a stale one is removed only by the lock holder and only if it is a socket.
- The socket path with its NUL must fit `sun_path`: 108 bytes on Linux, 104 on macOS.
- IPC: 4-byte big-endian length plus a JSON object (max 64 MiB, `version` 1); each request carries the client's build key, which the daemon checks. Both ends drop a peer of another effective user ID.
- `daemon start` forks the daemon (setsid, stdio to /dev/null, other inherited descriptors closed) and returns once it listens. While another process holds the lock it waits up to 10 s for a daemon of its build, then fails with `io-error` naming the lock and its holder.
- Without a runtime directory, `start` fails with `io-error`; `status` and `stop` exit 0 reporting none. `CI` and `MARA_NO_DAEMON` do not affect the `daemon` operations.
- Without a session the daemon exits after the idle period of DES-IDLE-SESSIONS from its start or last request; `status` and `stop` do not count as requests. SIGINT and SIGHUP stop it as SIGTERM does.
- `status --format json`: `running`, `pid`, `sessions`, `socket`, `lock`, `unavailable`.

### mara-jkabqy Client routing: on-demand start, first call in-process

*Decisions made*

- The direct-mode flag is the global flag `--no-daemon`. `CI` counts when set to a non-empty value, and `MARA_NO_DAEMON` only when it is exactly `1`.
- A request is dispatched once its frame is written in full. A refusal from the daemon (`stopping`, another build, a malformed request) means it ran nothing, so the client serves the request in-process. If the runtime directory or socket fails its checks, the client serves in-process and starts no daemon. A failure after dispatch, including an unreadable answer, is `daemon-lost` with exit status 1 and no work counted.
- The client spawns `daemon start` in its own working directory and in a process group of its own, with `--project` made absolute when given. It does not wait for it, and a thread reaps it so that the MCP server leaves no zombie. A failed start does not fail the request.
- The request carries the operation's words, arguments, options, the MCP inputs, `project`, `cwd` and `json`. The daemon resolves every path from `cwd` and runs in `/`.
- Until sessions exist, the daemon serves each request from the files as they are, and reports the counts of that request.

*Decisions made in the review round*

- Orchestrator, on the maintainer's behalf: the client's wait on a daemon that stopped answering (no read or connect deadline) moved to mara-xqcnqw as a checklist item, with the note that an incomplete send fell back only after 20.5 s.
- Orchestrator, on the maintainer's behalf: the AGENTS.md local check and worker calls run in direct mode (`--no-daemon`), so that no worktree's build leaves a daemon in the developer's session.
- Worker: the client's values are kept per serving thread (`caller::serve`) rather than through `unshare(CLONE_FS)` per thread, which is Linux-only. A created file or directory gets its mode less the client's umask at creation, so it never has more bits than direct mode would give it, and then exactly that mode.

### mara-xqcnqw Sessions, watching and concurrency

- Watching: inotify through `libc`, one instance per daemon (inotify instances are limited per user), no new crate; a directory that cannot be watched is only checked by requests. Other systems do not watch yet (mara-txbf8x). DES-DAEMON-SESSIONS.
- Watched: the directories the last check listed, `.mara` and the schema's directory. Debounce 50 ms after the last change, at most 500 ms after the first. Overflow or error: every session re-derives. DES-DAEMON-SESSIONS.
- A background derivation does what a document read does (no code scan, no evaluation), one per session at a time; its work goes to the next response. A daemon started for a project derives its session in the background at once, so its first response reports the opening as a cold request. DES-DAEMON-SESSIONS, DES-DAEMON-LIFECYCLE.
- A request holds the session's inputs only while it checks and derives them, then keeps its own view; reads never wait for a write or an evaluation. The per-project write queue is the project's write lock, as between processes. DES-DAEMON-SESSIONS.
- A request that selects no project runs in no session and reports its own counts. DES-DAEMON-SESSIONS.
- Idle: the period runs from the start or end of a session's last request; a session serving a request is not dropped; the daemon exits once its last session is dropped and it serves no request. DES-IDLE-SESSIONS.
- Client deadlines: non-blocking connection retried for 2 s while the queue is full, then in-process; 10 s in all to send, then in-process; after dispatch a heartbeat frame (`working` true) every second, and `daemon-lost` after 15 s without a frame. The protocol keeps `version` 1: only one build talks to its own daemon. DES-DAEMON-LIFECYCLE.
- Stop: rule evaluations end through `rules::cancel` (child killed, directory removed) and the daemon waits for them and for its writes; other reads end with the process, and their clients get `daemon-lost`. DES-DAEMON-LIFECYCLE.

### mara-zmp8yz Starting and cancelling Soufflé is safe in the daemon

- The test stops the daemon through a new debug-only pause point, `MARA_TEST_PAUSE_EVALUATION`, recorded in ADR-DEBUG-PAUSE-POINT. Its name begins with `MARA_TEST_PAUSE`, so the existing release-build check also covers it. Rejected: stopping at a random moment, which almost never reaches a window that is a few instructions wide.
- The daemon calls `cancel::all()` before it removes its socket, not after. This lets a test observe that the cancellation happened (DES-DAEMON-LIFECYCLE).
- The test watches the client's `TMPDIR` with inotify, so it also catches a directory that was created and then removed. Without the watch it would pass on the old code too, whose later check in `spawn_and_wait` removed the directory.
- `pre_exec` returns `ESRCH` for a parent that ended. No one reads this error, because Mara has already ended.

### mara-v9tfhw The CLI and the daemon read options the same way

- Rule for a repeated option that does not repeat: the CLI (direct mode or with a daemon) and the MCP server reject it with `invalid-argument` (exit 2), and the last value never wins. The daemon checks a request again by the same rule and refuses one that repeats such an option with `malformed-request`, without running it; no client of its build sends one (review fix). This was already the CLI's and the daemon's behaviour and what the DES-OPERATIONS code table says ("repeated"), and it keeps a typo from silently overriding a value. Recorded in DES-OPERATIONS.
- In MCP such a parameter takes one value, so an array is a type error (`invalid-argument`). Recorded in DES-OPERATIONS.
- `--` is not an end-of-options marker; it is an unknown option. This was the existing behaviour, now written into DES-OPERATIONS.

Filed: mara-dxc5wm (`--debug-stats` and `requested_json` also scan the raw arguments: a different flag, and its rule for rejected arguments needs a decision) and mara-qdmsnd (duplicate JSON keys in an MCP `arguments` object: the last one wins inside `rmcp`/`serde_json`), both backlog.

Review fixes: DES-OPERATIONS and VER-CLI-MCP-PARITY now name `malformed-request` for a request sent to the daemon itself. The `--id --no-daemon` test decides "served by the daemon" by work counts, as VER-DAEMON-START defines it: the same request repeated 2 s later reports no work, which direct mode never does. It still fails with the old raw scan. Merged `chore/clear-worktree-for-rewrite` at `594e9a0` and reran all seven local checks: all pass.

### mara-mg711b Diagnostic codes and paths are not leaked for the life of the process

- `Cow<'static, str>` over `Arc<str>` or `String`: built-in codes stay allocation-free, and only an invariant's name is copied, once per diagnostic. Recorded in DES-DIAGNOSTICS.
- `Refusal::file` in the 0.3.0 conversion falls back to `invalid-project` if a diagnostic ever carried an owned code; the conversion runs no invariant, so this cannot happen today, and `Failure` keeps its `&'static str` code.

### mara-8hw038 Warm counts after a watched change and after a write

*Decisions made*

- By the orchestrator for the maintainer: a file that the watcher's derivation read less than 2 s after its mtime or ctime is read and hashed again by the next request's check; a watch is never proof that a file is unchanged. VER-WORK-COUNTERS changed instead of the session. Rejected: trusting a watch in place before the check (loosens REQ-NEVER-STALE); deferring the derivation until files settle (loses the 50 ms warm start). DES-DAEMON-SESSIONS, VER-WORK-COUNTERS.
- A write served in a session counts 1 hash per file written and no read; its files count as checked when the request began, before the renames, so the next check reads them again. DES-DAEMON-SESSIONS.
- A background derivation skips re-reading an unchanged-stat file it could not settle. DES-DAEMON-SESSIONS.
- The evaluation store keeps rows and compile errors only (never limit, runtime or host faults), at most 64 MiB per session by estimate (provisional), oldest dropped first; evaluations run without holding the store. DES-DAEMON-SESSIONS.

*Decisions made (review fixes)*

- By the orchestrator for the maintainer: the session's store tells outcomes apart by the identity of the Soufflé executable (path, size, modification time), as a daemon belongs to a build. DES-DAEMON-SESSIONS.
- Each kept outcome is estimated at 256 bytes plus its rows (provisional). DES-DAEMON-SESSIONS.
- The hand-over after a write keeps every other input whose stat is unchanged without reading it. DES-DAEMON-SESSIONS.
- A write's window runs from before its first rename (before `write::apply`'s checks) to its hand-over; no background derivation runs in it. DES-DAEMON-SESSIONS.
- The background skip covers the configuration and the schema. DES-DAEMON-SESSIONS.
- The read-back reuses the parse of a document of the edit's snapshot with the same contents, found by comparing contents (no hash, no index). Direct mode benefits alike.
- `MARA_TEST_DERIVED`, a debug-only report of background derivations for tests. ADR-DEBUG-PAUSE-POINT.
- VER-WORK-COUNTERS: "`project validate` or `query gate` after a document edit" counts no run for a program run against empty facts whose outcome the session keeps.

### mara-wz40n6 Compare CLI-direct and daemon results

- The test takes the operations to cover from the `tools/list` of `mara mcp`, which is one tool per document operation of DES-OPERATIONS (VER-CLI-MCP-PARITY). It leaves out `project_init` and `project_convert`, which run in the client's process (DES-DAEMON-LIFECYCLE). Written into VER-HOST-COMPARISON.
- Service is shown per request by a count that `mara daemon status` reports, which the orchestrator approved as the smallest addition. Nothing that already existed could show it: the PID in the daemon's reply does not reach the client's output, and the work counts of a cold request or a failure that does no work are the same in both modes. The count is global, not per session, and counts operation requests that the daemon ran and answered, not `status` or `stop` or the requests it refuses. Written into DES-DAEMON-LIFECYCLE, VER-DAEMON-START and VER-HOST-COMPARISON.
- The daemon starts outside the project, so the earlier request opens the session, as VER-HOST-COMPARISON says.
- An MCP call whose arguments the client rejects is compared with `mara mcp --no-daemon`, not with the CLI: DES-MCP-SERVER names the parameter in that message. Written into VER-HOST-COMPARISON.
- Between runs, the files are restored by rewriting only those that differ and removing those that were added, at the same path, instead of deleting and rewriting the whole tree. This is how a real workflow changes files under a watching daemon.
- The MCP test harness gets the test process's temporary directory as `TMPDIR`, not one inside the test's directory: the project is the test's directory there, and a temporary directory inside it would change the files that the parity tests compare.

### mara-p27z9t Daemon performance tests

*Decisions made*

- By the orchestrator for the maintainer: a file that `--input` names is read by every request that names it; the session keeps nothing of it. The rows "The same read request again" and "Another request, inputs unchanged" of VER-WORK-COUNTERS count one file read for each such file. Rejected: tracking request-supplied files in the session. DES-DAEMON-SESSIONS, VER-WORK-COUNTERS.
- A session keeps the findings of the built-in checks with the snapshot they checked. DES-DAEMON-SESSIONS.
- `get` of a code location reuses the contents the session kept of the file, from an earlier `get` or from a scan of a file with markers. DES-DAEMON-SESSIONS.
- The derivation report carries the derivation's check time; a warm test repeats a run whose watcher came late, at most 3 runs. ADR-DEBUG-PAUSE-POINT.
- VER-WORK-COUNTERS wording: the example of kept evaluations is `query execute` of an invariant's program after `project validate` (the corpus compiles the views' program against empty facts); the program-edit row names a program that no other program includes; the re-check's wording points (retained outcomes, a byte-for-byte restore, a late watcher after a write, the cold write row's known contents).

*Decisions made (review fixes)*

- By the orchestrator for the maintainer, provisional: a scan keeps the contents only of code files with markers, so the first `get` of a code file without markers after a scan reads it once; the row "Another request, inputs unchanged" says so. DES-DAEMON-SESSIONS, VER-WORK-COUNTERS; mara-2ndg14 stays in the backlog.
- By the orchestrator for the maintainer: the touched-input row counts the watcher's read: once by the request, once more by a derivation that checked the input before it and less than 2 s after the touch, once by that derivation alone when it checked 2 s or more after. The edited-document row and the post-write row state the same order. VER-WORK-COUNTERS.
- The row for an input last hashed less than 2 s after its change: a derivation after that check leaves the input unread (DES-DAEMON-SESSIONS), so the input is read once, by the request or by a derivation that checked it 2 s or more after the change, never twice. VER-WORK-COUNTERS.
- A run in which a derivation that did work reported while the request ran is repeated, at most three times; a late watcher is a case asserted, not repeated. ADR-DEBUG-PAUSE-POINT.
- The release scaling runs after an edit accept 1 or 2 reads, equal to the hashes: no derivation report exists in a release build to tell which case a run is in.

### mara-ef6ybb Daemon housekeeping: watcher lock, one parse per frame, lock file name, WorkCounts sum, caller umask and mode

- Umask on platforms without `/proc/self/status`: macOS offers no way to read the umask without setting it, so it keeps set-and-restore with the race named in a comment and in DES-DAEMON-LIFECYCLE. Linux without `Umask:` (before 4.7, no `/proc`) falls back to the same.
- `caller::write` on a dangling symbolic link, or a file removed between the two opens, fails with `NotFound` instead of creating the target: the open decides the mode, and its only callers write fresh names in a private directory.
- The watcher-lock test uses a new debug-only pause point, `MARA_TEST_PAUSE_WATCH` (recorded in ADR-DEBUG-PAUSE-POINT), because no existing event falls inside the sync.
- What watching still misses is recorded in DES-DAEMON-SESSIONS: a change in a directory before its watch is added, and a directory replaced between its lookup and its watch, which is watched again at the next sync. Watching only saves work (REQ-NEVER-STALE).

Filed: mara-es7j9r (deferred backlog): `forget` is keyed by root and can remove a newer session's watches. It predates this issue and concerns `forget`, not the lock.

### mara-6za81b A session keeps the outcome of `query report`

*Decisions made*

- An explained outcome is kept when it is settled (every chosen row's explanation, or its reason, came from the exchange) or is a compile error; an outcome with a row left unexplained by the time limit, an abort, a stray or invalid reply or the 16 MiB bound is not. A stray reply after every row was explained leaves the outcome settled. DES-DAEMON-SESSIONS.
- Kept by evaluation key and Soufflé build, matched by the rows explained, which the request chooses from the kept run's rows: the targets are compared, not the inputs of `select` (entry, item), so nothing `select` depends on can be left out of the key. Rejected: hashing the entry and item into a report key (a field added to an entry later could be missed). DES-DAEMON-SESSIONS.
- What else an explanation depends on is in the evaluation key (facts with their paths and lines through the snapshot key, supplied inputs, program, limits) or fixed by the daemon's build (`MAX_NODES`, `MAX_PROOF_BYTES`); page options choose no part of the outcome and are not in the key. DES-DAEMON-SESSIONS.
- Two entries of one program share the evaluation key; when they choose the same rows they share the kept run, and each judges its outcome and roles from the rows itself. DES-DAEMON-SESSIONS.
- Explained outcomes are kept apart from evaluations: no other consumer reads a provenance run's rows.

*Decisions made (review fixes)*

- By the orchestrator for the maintainer: a run in provenance mode stopped at a limit, or that exited unsuccessfully or printed on its standard error after its first reply, is not kept, even when every chosen row has its proof. DES-DAEMON-SESSIONS.
