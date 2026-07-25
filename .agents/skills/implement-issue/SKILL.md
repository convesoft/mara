---
name: implement-issue
description: Explicit-only, persistent worker workflow for delivering one Mara Linear issue from accepted scope through implementation, validation, independent review, pull-request maintenance, and a verified MERGE READY handoff returned to the Codex task that spawned the worker. Use only when a user explicitly invokes `$implement-issue` followed by exactly one Linear issue identifier such as `CON-99`, or when an explicitly invoked `$iterate-linear` workflow places that exact invocation in a dedicated worker task prompt; never invoke this skill by semantic matching or infer an issue from conversation context.
---

# Implement Issue

Deliver exactly one Linear issue from initial analysis through a verified merge-ready
pull request and return that result to the spawning task.

## Invocation contract

Accept only an explicit invocation in this form:

```text
$implement-issue CON-99
```

Treat the text following `$implement-issue` as the complete input. Require exactly
one issue identifier matching `^[A-Z][A-Z0-9]*-[0-9]+$`. Do not infer, repair, or
select an issue from surrounding context. If the input is missing, malformed, or
contains multiple identifiers, stop and ask the user for exactly one valid issue
identifier.

Never run this workflow because a request merely resembles issue implementation.
The user or an explicitly authorized orchestrator-created worker prompt must name
`$implement-issue` and the issue ID exactly.

## Start or resume the persistent goal

After validating the identifier, determine the task's goal state before any
repository, Linear, or GitHub action:

- On the initial worker invocation, make the first native tool action a goal
  creation. Do not assign a token budget. Use the objective below.
- When the caller resumes this task after `BLOCKED`, make the first native tool
  action a goal read. Require the existing matching goal to be unfinished and
  reusable, including a goal the platform has resumed from blocked state, and reuse
  it; do not create a duplicate goal.
- When the caller resumes this task because a prior `MERGE READY` result became
  stale before merge, make the first native tool action a goal read. Require the
  prior matching goal to be complete, then create a new unbudgeted recovery goal
  scoped to restoring this same issue and PR to verified `MERGE READY`.

Initial goal objective, substituting the identifier:

> Deliver ISSUE_ID from start to finish: understand its accepted scope, implement
> it completely, validate it, pass independent local review, open and maintain its
> pull request, address CI and GitHub Codex review findings, and continue until the
> pull request is verified merge-ready and that result is returned to the spawning
> task.

Recovery goal objective, substituting the identifier:

> Restore ISSUE_ID to verified merge readiness after a caller-detected regression:
> preserve the same issue, worktree, branch, and pull request; identify and resolve
> the stale condition from authoritative state; repeat all invalidated validation
> and review; and return a fresh MERGE READY result to the spawning task.

If the observed goal state does not match one of these cases, return `BLOCKED`
rather than creating or completing the wrong goal. Do not perform repository,
Linear, or GitHub actions before this goal gate completes.

## Resolve execution context

Read [references/worker-workflow.md](references/worker-workflow.md) completely after
the goal gate completes and before proceeding. Apply it as the authoritative
execution workflow with these substitutions:

- `ISSUE_ID`: the validated input identifier.
- `ISSUE_TITLE`: the title returned by Linear for that exact identifier.
- `LINEAR_ISSUE_URL`: the canonical URL returned by Linear.
- `REPOSITORY_URL`: the current repository's canonical `origin` URL.
- `BASE_BRANCH`: the repository default branch reported by GitHub, verified against
  the corresponding remote-tracking ref. Do not guess from the current branch.
- `WORKER_BRANCH`: on initial execution, derive a neutral outcome-named branch in
  the form `codex/<lowercased-issue-id>-<outcome-slug>`, for example
  `codex/con-99-preserve-source-spans`, and persist that exact value in the worker
  task context, worktree, PR, and terminal report. On any resumption, resolve and
  reuse the persisted branch from those artifacts; never derive a new slug from
  changed issue wording. Return BLOCKED if the artifacts disagree. Derive the
  initial concise slug from the accepted delivery outcome, not from an
  implementation technique or speculative design.

Treat “control plane” in the workflow as the user-facing parent Codex task that
created this worker task. Reports, authorization requests, and blockers return to
that caller through the task protocol below.

The bundled workflow's `PERSISTENT GOAL` section restates the initial goal contract.
Treat that action as already satisfied by this wrapper's goal gate; never create a
second goal merely because the reference says the first action is goal creation.
On resumption, this wrapper's active-goal reuse and completed-goal recovery rules
override that restatement.

Resolve every value from its authoritative source. If the Linear issue does not
exist, the current repository has no unambiguous canonical remote/default branch,
or the deterministic worker branch conflicts with existing work, report a blocker
using the workflow's blocker format. Do not substitute a nearby issue, repository,
base branch, or suffixed worker branch.

## Execute without truncation

Follow the bundled workflow through implementation, validation, independent local
review, PR creation and maintenance, remote CI and review, and a verified
merge-ready handoff. Its waiting states are active work states, not completion. Do
not mark the persistent goal complete before all merge-ready conditions pass and the
result is ready to return to the caller.

## Return results to the spawning task

Assume this skill runs in a Codex worktree task created by another Codex task. The
spawning task owns this worker's `threadId` and `hostId` and observes it with the
built-in `wait_threads` capability. The worker does not need the caller's thread ID:
ending a turn with a final response is the supported return channel to the caller.

Use these task boundaries:

- Keep ordinary implementation, CI, review, and monitoring updates in concise
  commentary. `REMOTE REVIEW STARTED` and timeout snapshots are progress updates,
  not terminal results. Continue working or monitoring after them.
- When every merge-ready condition passes, complete the persistent goal and end the
  turn with the exact `MERGE READY` report from the bundled workflow as the entire
  final response. Do not add a preamble or text after it. Goal completion and the
  final report form one terminal worker handoff.
- When authority, user input, or an external change is required and safe in-scope
  work is exhausted, end the turn with the exact `BLOCKED` report as the entire
  final response. Keep the persistent goal active unless the workflow's genuine
  blocked-status threshold has been met.
The spawning task is expected to receive `MERGE READY` or `BLOCKED` through
`wait_threads`. After `MERGE READY`, the caller owns merge authorization, merge, and
post-merge coordination; do not merge or wait for authorization in this worker. A
caller may resume this same worker with `send_message_to_thread` either to resolve
a `BLOCKED` result or to repair a caller-verified regression that invalidated a
prior `MERGE READY` result. Preserve the existing active goal for the former; use
the bounded recovery-goal path for the latter.

Do not try to discover or guess the caller with `list_threads`. Do not create a new
task to report status, send the result to an unrelated task, archive this worker, or
call `handoff_thread`; handoff moves checkout/worktree state and is not a result
channel.

When this wrapper and the bundled workflow differ, this wrapper controls the
invocation contract, goal lifecycle, runtime substitutions, meaning of “control
plane,” and caller return protocol. The bundled workflow controls all other
delivery behavior.
