---
name: iterate-linear
description: Explicit-only, parameterless Mara delivery orchestrator that persists until every in-scope Linear issue is closed. It repeatedly invokes `$find-next-issue`, assesses the returned issue with `$assess-issue`, dispatches READY work to one dedicated Codex worktree task using `$implement-issue`, verifies and merges MERGE READY pull requests, waits for their automatic Linear completion, synchronizes the default branch, and repeats until the scheduler reports COMPLETE. Use only when the user explicitly invokes `$iterate-linear` with no arguments; never invoke this skill implicitly.
---

# Iterate Linear

Serially drive the complete Mara Linear delivery graph from current state to verified
closure.

## Invocation and authority

Accept only:

```text
$iterate-linear
```

If arguments follow the skill name, stop and state that the skill is parameterless.
Never invoke implicitly.

Explicit invocation authorizes this orchestrator to:

- apply the bounded Linear mutations authorized by `$find-next-issue` and
  `$assess-issue`;
- apply READY issues' `codex-model:<model-id>` and
  `codex-reasoning:<effort>` labels when creating their worker tasks;
- create dedicated Codex worktree tasks for READY issues;
- merge worker pull requests only after a fresh merge-readiness recheck;
- wait for and verify the automatic completion of merged implementation,
  clarification, prerequisite, and split-child issues;
- fast-forward the local default branch after each merge;
- archive a worker task after its issue is merged and closed.

Do not manually close an ordinary issue. The only manual completion transition in
this workflow is the verified coordination-parent closure performed inside
`$find-next-issue` under that skill's contract.

It does not authorize force pushes, history rewrites, destructive worktree cleanup,
merging a stale or unverified PR, arbitrary issue creation, or work outside the Mara
Linear delivery scope.

## Create or resume the persistent goal first

After validating the parameterless invocation, make the first native tool action a
goal read. Reuse a matching unfinished goal for this delivery run, including one
the platform resumed after interruption or blockage. If none exists, create a goal
without a token budget using this objective:

> Close every in-scope Mara Linear issue: repeatedly select the globally next
> available issue, assess and remediate it, implement READY work in a dedicated
> Codex worktree task, merge each verified merge-ready pull request, wait for and
> verify its automatic Linear completion, synchronize the default branch, and
> continue until the complete Linear delivery graph is verified closed.

If a different unfinished goal exists, return BLOCKED instead of replacing it or
creating a duplicate. A completed prior Iterate Linear goal does not block a new
explicit run.

Keep the goal active during selection, assessment, remediation, implementation,
review, waiting, merge, synchronization, and issue closure. Complete it only after
`$find-next-issue` reports COMPLETE and an independent final Linear/GitHub check
confirms no in-scope issue remains open or lacks required merged evidence.

Use the native blocked-goal status only after the same blocker meets the platform’s
repeated-turn threshold. Never complete the goal because work is waiting.

## Compose explicit-only child skills

This skill is the only in-task orchestrator. After its goal gate and before the first
child invocation, read these project skill contracts completely:

- `.agents/skills/find-next-issue/SKILL.md`
- `.agents/skills/assess-issue/SKILL.md`
- `.agents/skills/implement-issue/SKILL.md`

All child skills remain explicit-only. Invoke them only with the exact `$skill-name`
syntax defined by their contracts; a semantic match is never sufficient.
`$find-next-issue` and `$assess-issue` return nonterminal structured phase results
to this active orchestrator. `$implement-issue` runs only in its dedicated worker
task and returns its terminal report through `wait_threads`.

Every worker creation or resumption message must consist solely of
`$implement-issue ISSUE_ID`; append no prose or additional input.

If a child contract is missing, unavailable, or differs from the expected input or
output schema, return BLOCKED. Do not approximate or inline a substitute workflow.

## Establish the control checkout

Before dispatching work:

1. Read root `AGENTS.md` and `docs/index.mara.md` completely.
2. Inspect the repository root, current branch, HEAD, status, remotes, default
   branch, and worktree list.
3. Require the control task to use the primary local checkout on the repository’s
   default branch with a clean tracked and untracked status. Do not run this
   orchestrator from a worker worktree or feature branch.
4. Fast-forward from the canonical remote with `git pull --ff-only`. Never reset,
   discard changes, or rewrite history to satisfy this gate.
5. Use the Codex app’s project listing and resolve exactly one saved project for the
   current Mara repository. If ambiguous, return BLOCKED before creating a task.
6. Resolve the GitHub repository, Linear workspace/project scope, and team statuses
   from authoritative tools. Do not hardcode status IDs or branch names.

If the checkout becomes dirty outside an expected fast-forward, stop and preserve
the changes.

## Keep one implementation worker at a time

Dispatch serially. Do not create a second implementation worker until the current
worker is merged and archived or has returned a blocker that the orchestration loop
has durably routed to separate remediation work. Serial operation prevents default-
branch and merge-readiness races.

Maintain a registry in task context containing each issue’s worker `threadId`,
`hostId`, selected model and reasoning effort, latest wait cursor, PR, branch, and
last result. Reuse an existing blocked worker for the same issue rather than
creating a conflicting worktree.

Before creating any worker, reconcile that registry against the Codex project task
list, Git worktree list, and open GitHub PRs/branches for the exact issue. Rehydrate
an existing worker's `threadId`, `hostId`, cursor, branch, PR, and actual model and
reasoning from the owning task's metadata when exactly one match exists. If that
metadata does not expose its creation settings, record model and reasoning as
`unknown`; never infer them from current issue labels. If multiple possible workers
exist, or a branch/PR exists without an unambiguous owning task, return BLOCKED and
create nothing. A missing in-memory registry entry is never sufficient proof that
no worker exists.

## Run the orchestration loop

Repeat the following phases without asking the user to restate the command.

### 1. Select globally

Invoke `$find-next-issue` with no arguments and follow its skill contract exactly.

- `NEXT`: continue with the returned issue ID.
- `WAITING`: do not busy-loop. Monitor active worker tasks or the named external
  completion signals, then invoke `$find-next-issue` again when state changes.
- `COMPLETE`: perform the final completion audit below.
- `BLOCKED`: report the graph/scope inconsistency. Keep the goal active unless the
  blocked-goal threshold is met.

Never choose an issue independently of `$find-next-issue`.

### 2. Assess exactly the selected issue

Invoke `$assess-issue ISSUE_ID` and follow its contract exactly.

- `READY`: dispatch or resume the issue’s implementation worker.
- `CLARIFY`: the assessor has created/reused and linked a clarification issue.
  Reinvoke `$find-next-issue`; do not directly choose the new issue.
- `SPLIT`: the assessor has created/reused children and converted the issue to a
  coordination parent. Reinvoke `$find-next-issue`; do not choose a child here.
- `BLOCKED` with `Blocker kind: EXISTING_ISSUE` or `CREATED_PREREQUISITE`: reinvoke
  `$find-next-issue`, which will evaluate the updated global graph.
- `BLOCKED` with `Blocker kind: COORDINATION_PARENT`: treat it as a contract
  mismatch between selector and assessor and return BLOCKED.
- `BLOCKED` with `Blocker kind: EXTERNAL` or `MUTATION_FAILURE`: wait or return the
  blocker. Do not repeatedly call the two skills against an unchanged state.

Thus every verified Linear mutation from a non-READY assessment returns control to
the global selector. The orchestrator never interprets created issue order as
permission to bypass `$find-next-issue`.

If assessment returns a coordination-parent boundary for an issue selected by the
global selector, treat that as a contract mismatch between the two skills and return
BLOCKED instead of looping.

### 3. Dispatch or resume a READY worker

If no worker registry entry exists for the issue:

1. Re-read the exact Linear issue and use its current label names as the
   authoritative worker-configuration snapshot.
2. Independently collect labels with the exact prefixes `codex-model:` and
   `codex-reasoning:`. Require at most one label for each prefix and a non-empty
   suffix. If either prefix is duplicated or has an empty suffix, return BLOCKED
   without creating a task.
3. Treat this skill's explicit invocation as authorization to apply a
   `codex-model:<model-id>` label as the issue's explicit worker-task `model`
   configuration, and treat a
   `codex-reasoning:<effort>` label as the explicit request for its `thinking`
   value. Preserve each suffix exactly; do not normalize, alias, or infer values.
   A missing prefix means omit only that corresponding override so the task uses
   the configured default.
4. Check the native worker-creation capability's current model and reasoning
   contract. If a requested value or model/reasoning combination is unsupported,
   return BLOCKED with the offending labels and create nothing. If task creation
   rejects the requested combination for the destination host, likewise return
   BLOCKED; never retry by silently dropping or changing either override.
5. Call the Codex app project-list capability and use the already resolved Mara
   project ID.
6. Create one project task with a new `worktree` environment. Omit `startingState`
   so it starts from the project default branch. Pass the resolved model as
   `model` and reasoning effort as `thinking` when their labels are present.
7. Use this entire prompt, substituting the issue ID:

   ```text
   $implement-issue ISSUE_ID
   ```

8. Record the resolved model and reasoning effort with the returned `threadId` and
   `hostId`. If worktree creation is queued and only a client ID is available, wait
   for that creation to resolve; never dispatch a duplicate task.

If a blocked worker already exists for the issue, resume it with
`send_message_to_thread` under the worker-message rule above. The worker re-reads
authoritative state to verify which blocker changed. The existing task retains the
model and reasoning selected when it was created; do not replace it or reinterpret
changed labels during resumption.

### 4. Wait for the worker result

Use `wait_threads` with the worker’s `threadId`, `hostId`, and latest cursor. Use
bounded waits and continue waiting on timeouts; commentary does not mean completion.

Accept only:

- `MERGE READY` with issue, PR URL, branch, exact head SHA, requirements, CI,
  automatic Codex review, other review threads, local validation, local independent
  review, residual risks, and recommended merge method;
- `BLOCKED` with issue, phase, concrete blocker, evidence, safe work already
  completed, required decision/authority/external change, available options, and a
  recommended option.

Treat malformed, incomplete, or mismatched results as BLOCKED. Never infer merge
readiness from commentary or an open PR alone.

### 5. Handle a blocked worker

Keep the worker task and worktree available. Reassess the same issue once using
`$assess-issue ISSUE_ID`, incorporating the worker’s evidence.

- If assessment returns `CLARIFY` or identifies or creates a blocking prerequisite,
  return to `$find-next-issue`. Preserve the blocked worker registry entry for later
  resume.
- If assessment returns `SPLIT`, first verify the worker has no unique commits,
  uncommitted changes, or open PR. Then archive it and remove its registry entry;
  if unique work exists, return BLOCKED rather than abandoning or reassigning it.
- If assessment still reports READY without resolving the worker blocker, return
  BLOCKED because assessment and implementation evidence conflict.
- If the blocker is external or requires user authority, report it and wait. Do not
  create another worker or silently broaden scope.

### 6. Recheck and merge a MERGE READY result

Immediately before merge, independently fetch current GitHub state and verify:

- the PR is open, non-draft, and targets the canonical default branch;
- current head SHA exactly equals the worker-reported SHA;
- the branch is current with the default branch and the PR is mergeable;
- every required check passes;
- automatic GitHub Codex review completed for that head through `eyes` followed by
  either `+1` or a terminal bot comment, with a no-findings comment equivalent to
  `+1`, and its delayed snapshot has no actionable findings;
- no actionable human review thread remains;
- the PR description cites the Linear issue and affected Mara IDs;
- the worker’s validation and independent-review evidence is complete;
- no residual risk contradicts merge readiness.

If state is stale or a condition regressed, do not merge. Resume the same worker with
the exact worker message above. It re-reads the PR and authoritative state to
identify the regression. Wait for a fresh MERGE READY result.

Use the repository-authorized merge method, preferring the worker’s recommendation
only when it matches GitHub and project policy. Merge through the GitHub capability
with `expected_head_sha` set to the verified head. Never merge without that guard.

Confirm the API reports a successful merge, re-fetch the PR, and record the merged
commit. A failed or ambiguous merge is BLOCKED until reconciled; never retry blindly.

### 7. Verify automatic issue closure and synchronize main

After confirmed merge:

1. Require the local control checkout to remain clean and on the default branch.
2. Run `git pull --ff-only` from the canonical remote and verify local HEAD contains
   the merged commit. Do not reset on failure.
3. Re-read the Linear issue until the merge automation moves it to a completed
   state, using bounded waits between checks.
4. Do not manually update the state of an implementation, clarification,
   prerequisite, or split-child issue. Manual closure is reserved for verified
   coordination parents and occurs only inside `$find-next-issue`.
5. Confirm the PR/merge evidence remains linked or otherwise discoverable from the
   issue. Do not add closing-keyword semantics to PR text.
6. Archive the completed worker task using its recorded `threadId` and `hostId`.
7. Remove its active registry entry, record the completed issue and PR, then return
   to `$find-next-issue`.

If the PR merged but the automatic Linear completion signal does not arrive within
the bounded wait, report BLOCKED with the merged commit and the missing automation
signal. Do not manually close the issue or dispatch another worker. If local
synchronization fails, likewise report BLOCKED with the exact recovery action and
preserve all state until control is reconciled.

## Prevent loops and duplicate work

Record a fingerprint after each selector, assessor, worker, merge, and Linear
mutation result. A fingerprint includes relevant issue states, dependency edges,
worker IDs/results and selected model/reasoning, PR head, and local default-branch
HEAD.

Do not repeat a phase against an unchanged fingerprint unless waiting on an
explicitly named external signal. Never create duplicate remediation issues,
duplicate worktree tasks, duplicate PRs, or duplicate merge attempts.

## Final completion audit

When `$find-next-issue` reports COMPLETE:

1. Re-list the complete in-scope Linear issue set without open-state filters.
2. Verify every implementation, clarification, prerequisite, split child, and
   coordination parent is in a completed state permitted by its contract.
3. Verify every implementation-bearing issue has confirmed merged GitHub evidence.
4. Verify no active worker remains and no open Mara implementation PR remains.
5. Fast-forward and verify the local default branch one final time.
6. Invoke `$find-next-issue` once more against the unchanged final state and require
   a second COMPLETE result.
7. Complete the persistent goal only after this audit succeeds.

## Return terminal reports

Keep ordinary loop progress in commentary. End the turn only for genuine blockage
or final completion.

For blockage:

```text
BLOCKED — Iterate Linear
Phase: <selection, assessment, dispatch, worker, merge, Linear closure, or synchronization>
Issue: <ID or None>
Worker task: <thread ID or None>
Concrete blocker: <facts>
Evidence: <Linear, GitHub, worker, or Git facts>
Safe work completed: <summary>
Required change or authority: <exact requirement>
```

For completion:

```text
COMPLETE — all in-scope Mara Linear issues are closed
Issues completed this run: <IDs and PRs>
Coordination parents closed: <IDs>
Final default branch: <name and HEAD>
Open implementation PRs: None
Active worker tasks: None
Final scheduler result: COMPLETE confirmed twice
```
