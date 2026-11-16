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
