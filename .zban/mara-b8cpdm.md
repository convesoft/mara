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
