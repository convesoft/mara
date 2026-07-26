---
name: implement-issue
description: Explicit-only, persistent worker workflow for delivering one Mara Linear issue from accepted scope through implementation, validation, independent review, pull-request maintenance, and a verified MERGE READY handoff returned to its caller. Use only when a user in a dedicated non-primary Git worktree explicitly invokes `$implement-issue` with exactly one Linear issue identifier such as `CON-99`, or when an explicitly invoked `$iterate-linear` workflow sends the defined worktree envelope to a dedicated implementation subagent; never invoke this skill by semantic matching or infer an issue from conversation context.
---

# Implement Issue

Deliver exactly one Linear issue from initial analysis through a verified merge-ready
pull request and return that result to the caller.

## Invocation contract

Accept a direct explicit invocation only when the caller's current checkout is
already a dedicated non-primary Git worktree:

```text
$implement-issue CON-99
```

or this exact four-line `$iterate-linear` subagent envelope:

```text
$implement-issue CON-99
Worktree: /absolute/path/to/worktree
Ownership: Implement only CON-99 in this worktree.
Coordination: Other agents may be active elsewhere; preserve their work and do not modify the control checkout.
```

Require exactly one issue identifier matching `^[A-Z][A-Z0-9]*-[0-9]+$` on the
first line. In the orchestrator form, require exactly four lines, an absolute
worktree path on the second line, the same identifier in the exact
ownership sentence, and the exact coordination sentence shown above. Append no
other input. In the direct form, designate the current checkout for later
resolution as `WORKTREE_ROOT`; in the orchestrator form, designate the supplied
path. Do not perform filesystem or repository resolution before the goal gate.

Do not infer, repair, or select an issue or worktree from surrounding context. If a
single-line direct input is missing or malformed, stop and request one valid direct
invocation in a dedicated non-primary worktree. If input contains `Worktree:` or
is multi-line or otherwise resembles a malformed orchestrator envelope, do not fall
back to the direct form. Return this complete pre-goal blocker, filling every field:

```text
BLOCKED
Issue: <parsed valid ID or Unknown>
Phase: Invocation
Concrete blocker: Malformed $iterate-linear worker envelope.
Evidence: <which required line or exact value is missing or invalid>
Safe work already completed: None; no goal or repository action was started.
Decision, authority, or external change required: Resend the exact corrected four-line envelope.
Available options: Correct the envelope; stop this worker.
Recommended option: Correct the envelope without changing the assigned issue or worktree.
```

If the same subagent later receives the corrected exact envelope and still has no
goal because this pre-goal blocker performed no native action, treat that corrected
message as the initial valid invocation and create the initial goal. Do not apply
the ordinary `BLOCKED` resumption rule until a goal has actually been created.

Never run this workflow because a request merely resembles issue implementation.
The user or an explicitly authorized orchestrator-created subagent prompt must name
`$implement-issue` and the issue ID exactly.

## Start or resume the persistent goal

After validating the identifier, determine the task's goal state before any
repository, Linear, or GitHub action:

- On the initial worker invocation, make the first native tool action a goal
  creation. Do not assign a token budget. Use the objective below.
- When the caller resumes this subagent after `BLOCKED`, make the first native tool
  action a goal read. Require the existing matching goal to be unfinished and
  reusable, including a goal the platform has resumed from blocked state, and reuse
  it; do not create a duplicate goal.
- When the caller resumes this subagent after a V1 `interrupted` status before any
  terminal worker report, make the first native tool action a goal read. Reuse the
  matching unfinished goal. If no goal exists because interruption occurred before
  the initial goal action, create the initial goal then. If the matching goal is
  complete but no terminal `MERGE READY` report was delivered, create a new
  unbudgeted recovery goal for the same issue, worktree, branch, and PR; re-read and
  revalidate all merge-ready evidence before returning a fresh report. If a
  different goal exists, return `BLOCKED`.
- When the caller resumes this same subagent because its completed turn returned a
  missing, malformed, incomplete, or mismatched terminal report, make the first
  native tool action a goal read. Reuse a matching unfinished goal; if the matching
  goal is complete, create a new unbudgeted recovery goal for the same issue,
  worktree, branch, and PR. Re-read authoritative state and return a fully valid
  fresh terminal report. If no matching goal exists, return `BLOCKED` rather than
  inventing prior progress.
- When the caller resumes this subagent because a prior `MERGE READY` result became
  stale before merge, make the first native tool action a goal read. Require the
  prior matching goal to be complete, then create a new unbudgeted recovery goal
  scoped to restoring this same issue and PR to verified `MERGE READY`.

Initial goal objective, substituting the identifier:

> Deliver ISSUE_ID from start to finish: understand its accepted scope, implement
> it completely, validate it, pass independent local review, open and maintain its
> pull request, address CI and GitHub Codex review findings, and continue until the
> pull request is verified merge-ready and that result is returned to the control
> plane.

Recovery goal objective, substituting the identifier:

> Restore ISSUE_ID to verified merge readiness after a caller-detected regression:
> preserve the same issue, worktree, branch, and pull request; identify and resolve
> the stale condition from authoritative state; repeat all invalidated validation
> and review; and return a fresh MERGE READY result to the control plane.

If the observed goal state does not match one of these cases, return `BLOCKED`
rather than creating or completing the wrong goal. Do not perform repository,
Linear, or GitHub actions before this goal gate completes.

A replacement subagent, including one spawned after a prior agent errored, may have
no local goal even though its assigned worktree, branch, or PR already exists. In
that case create the initial goal, then recover the durable issue state after the
goal gate. Never try to resume the errored agent's goal, and never treat absent
replacement-agent history as permission to replace the assigned worktree or create
a second branch or PR.

## Resolve execution context

Read [references/worker-workflow.md](references/worker-workflow.md) completely after
the goal gate completes and before proceeding. Apply it as the authoritative
execution workflow with these substitutions:

- `ISSUE_ID`: the validated input identifier.
- `ISSUE_TITLE`: the title returned by Linear for that exact identifier.
- `LINEAR_ISSUE_URL`: the canonical URL returned by Linear.
- `REPOSITORY_URL`: the assigned worktree repository's canonical `origin` URL.
- `WORKTREE_ROOT`: the validated execution worktree from the invocation contract.
- `BASE_BRANCH`: the repository default branch reported by GitHub, verified against
  the corresponding remote-tracking ref. Do not guess from the current branch.
- `WORKER_BRANCH`: on initial execution, derive a neutral outcome-named branch in
  the form `codex/<lowercased-issue-id>-<outcome-slug>`, for example
  `codex/con-99-preserve-source-spans`, and persist that exact value in the worker
  agent context, worktree, PR, and terminal report. On any resumption, resolve and
  reuse the persisted branch from those artifacts; never derive a new slug from
  changed issue wording. Return BLOCKED if the artifacts disagree. Derive the
  initial concise slug from the accepted delivery outcome, not from an
  implementation technique or speculative design.

In orchestrated form, treat “control plane” as the user-facing root Codex task that
spawned this worker subagent. In direct form, treat it as the invoking user-facing
Codex task and its user; the final report is delivered directly there. In both
forms, the control plane retains merge authorization and post-merge coordination.

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

After the goal gate, use `WORKTREE_ROOT` as the working directory for every
repository command and use absolute paths beneath it for every read or edit. The
subagent may start in the control checkout; that startup directory is context only
and is never an editing surface. Before touching repository content, prove that
`WORKTREE_ROOT` is a registered non-primary Git worktree for this repository. Stop
if any command, tool, or edit would target the control checkout.

## Execute without truncation

Follow the bundled workflow through implementation, validation, independent local
review, PR creation and maintenance, remote CI and review, and a verified
merge-ready handoff. Its waiting states are active work states, not completion. Do
not mark the persistent goal complete before all merge-ready conditions pass and the
result is ready to return to the caller.

## Return results to the caller

In orchestrated form, the implementation runs in a subagent spawned by the root
Codex task while repository work occurs only in `WORKTREE_ROOT`. The root owns this
worker's `agent_id` and observes it with `wait_agent`; ending the subagent turn with
a final response returns the result. In direct form, the current user-facing task
is both worker session and control-plane boundary; deliver the same terminal report
directly to the user without attempting `wait_agent`, `send_input`, or agent-ID
discovery.

Use these subagent boundaries:

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
In orchestrated form, the spawning task receives `MERGE READY` or `BLOCKED` through
`wait_agent`; it may resume this subagent with `send_input`. In direct form, the user
receives the report and may resume the same task with another exact direct
invocation. After `MERGE READY`, the control plane owns merge authorization, merge,
and post-merge coordination; do not merge or wait for authorization in this worker.
Preserve the existing active goal when resuming from `BLOCKED`; use the bounded
recovery-goal path when a caller-verified regression invalidated a prior
`MERGE READY` result. A replacement subagent must reconstruct state from the
assigned worktree, branch, PR, and authoritative services rather than assuming
prior agent context.

Do not try to discover or guess the caller or another subagent. Do not create a new
task or subagent to report status, send the result elsewhere, close yourself,
remove the worktree, or call `handoff_thread`. The root orchestrator owns subagent
closure and safe worktree cleanup.

When this wrapper and the bundled workflow differ, this wrapper controls the
invocation contract, goal lifecycle, worktree confinement, runtime substitutions,
meaning of “control plane,” and caller return protocol. The bundled workflow
controls all other delivery behavior.
