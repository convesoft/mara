---
name: iterate-linear
description: Explicit-only, parameterless Mara delivery orchestrator that persists until every in-scope Linear issue is closed. It repeatedly invokes `$find-next-issue`, assesses the returned issue with `$assess-issue`, provisions a dedicated Git worktree and dispatches one V1 implementation subagent using `$implement-issue`, verifies and merges MERGE READY pull requests, waits for their automatic Linear completion, synchronizes the default branch, and repeats until the scheduler reports COMPLETE. Use only when the user explicitly invokes `$iterate-linear` with no arguments; never invoke this skill implicitly.
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
  `codex-reasoning:<effort>` labels when spawning their worker subagents;
- create dedicated Git worktrees and implementation subagents for READY issues;
- merge worker pull requests only after a fresh merge-readiness recheck;
- reconcile verified acceptance checkboxes after a confirmed merge while preserving
  every other part of the issue description;
- wait for and verify the automatic completion of merged implementation,
  clarification, prerequisite, and split-child issues;
- fast-forward the local default branch after each merge;
- close a terminal worker subagent whenever this workflow mandates closure for
  concurrency release, recovery, blockage, malformed handoff, error, or post-merge
  coordination; and
- remove an orchestrator-created worktree only after proving it is clean and its
  issue branch has no unmerged unique work.

Do not manually close an ordinary issue. The only manual completion transition in
this workflow is the verified coordination-parent closure performed inside
`$find-next-issue` under that skill's contract.

It does not authorize force pushes, history rewrites, cleanup of user-created or
unverified worktrees, merging a stale or unverified PR, arbitrary issue creation,
or work outside the Mara Linear delivery scope.

## Create or resume the persistent goal first

After validating the parameterless invocation, make the first native tool action a
goal read. Reuse a matching unfinished goal for this delivery run, including one
the platform resumed after interruption or blockage. If none exists, create a goal
without a token budget using this objective:

> Close every in-scope Mara Linear issue: repeatedly select the globally next
> available issue, assess and remediate it, implement READY work through a
> dedicated subagent in an orchestrator-owned Git worktree, merge each verified
> merge-ready pull request, wait for and
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
to this active orchestrator. `$implement-issue` runs only in its assigned worker
worktree and returns its terminal report through `wait_agent`.

Every worker creation or resumption message must use exactly this four-line
orchestrator envelope, substituting the issue ID and normalized absolute worktree
path and appending nothing:

```text
$implement-issue ISSUE_ID
Worktree: ABSOLUTE_WORKTREE_PATH
Ownership: Implement only ISSUE_ID in this worktree.
Coordination: Other agents may be active elsewhere; preserve their work and do not modify the control checkout.
```

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
5. Resolve the GitHub repository, Linear workspace/project scope, and team statuses
   from authoritative tools. Do not hardcode status IDs or branch names.

If the checkout becomes dirty outside an expected fast-forward, stop and preserve
the changes.

## Keep one implementation subagent running at a time

Dispatch serially. Do not run a second implementation subagent until the current
subagent has returned a terminal result and is either closed for routed remediation
or its issue is merged and cleaned up. Serial operation prevents default-branch and
merge-readiness races. Closed blocked subagents and their worktrees may remain in
the registry while a selected prerequisite is implemented, but only one
implementation subagent may be running.

Maintain a registry in task context containing each issue's `agent_id`, subagent
state, exact original model and reasoning effort, normalized absolute worktree path,
worktree provenance, PR, branch, exact head, and last result. Treat the worktree,
branch, PR, and Git head as durable worker identity; treat the subagent as
replaceable execution state.

Before provisioning or resuming any worker, reconcile that registry against the
Git worktree list and open GitHub PRs/branches for the exact issue. Require exactly
one coherent durable worker or none. If multiple possible worktrees, branches, or
PRs exist, or their ownership cannot be proved, return BLOCKED and create nothing.
A missing `agent_id` never proves that no worker artifacts exist.

If no native subagent-list capability is available, a forgotten agent ID cannot be
recovered. Preserve every returned `agent_id` in task context. When an ID is
present, inspect it only through `wait_agent`, `send_input`, `resume_agent`, or
`close_agent`. When no usable ID survives but exactly one coherent, unambiguous
issue worktree and branch/PR chain does, spawn a replacement subagent against that
same worktree and require it to re-read all authoritative state. The worktree may
contain issue-owned uncommitted recovery work only when the prior writer is proven
closed, the exact issue branch is attached, and root inspection finds no unrelated
or ambiguous changes. Never create a second worktree merely because an agent is
unavailable.

Every worktree created by this workflow must have a unique normalized absolute path
outside the primary checkout and an issue-identifying final path component. Use an
authoritatively configured worktree root when available; otherwise use a dedicated
non-temporary sibling directory of the primary checkout. Record that exact path
before spawning. Create it detached at the verified canonical default-branch commit
with `git worktree add --detach`; the worker attaches the deterministic issue branch
under `$implement-issue`. Never point a worker at the control checkout, a temporary
directory, a user-created worktree, or another issue's worktree.

V1 subagents inherit the root task's sandbox and approval policy; the worktree path
in the prompt does not grant filesystem access. Before `git worktree add`, require
the active policy to permit the worker to write both the selected external worktree
path and the repository's shared Git metadata without a fresh approval. This may be
an unrestricted policy or explicit writable roots covering both locations. If the
policy does not provide that access, return BLOCKED before creating the worktree and
require the root task's permission mode or writable roots to be corrected. Never
place the worktree inside the primary checkout merely to bypass the sandbox.

## Run the orchestration loop

Repeat the following phases without asking the user to restate the command.

Before selecting new work, recover any registry issue whose exact PR head is
confirmed merged but whose phase 7 work did not fully finish. Rehydrate its recorded
acceptance matrix and merged commit, verify that the current criteria still match,
then resume phase 7 from its first incomplete step; completed steps are idempotent.
Do not reassess the issue, resume its worker, or attempt the merge again. If the
recorded evidence cannot be recovered or no longer matches, return BLOCKED with the
merged commit.

### 1. Select globally

Invoke `$find-next-issue` with no arguments and follow its skill contract exactly.

- `NEXT`: continue with the returned issue ID.
- `WAITING`: do not busy-loop. Monitor the running worker subagent or the named
  external completion signals, then invoke `$find-next-issue` again when state
  changes.
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

This workflow explicitly targets the V1 multi-agent API. Before any worktree
mutation, inspect the live V1 tool contracts and require all of these exact shapes:

- `spawn_agent` accepts `agent_type`, `fork_context`, `message`, optional `model`,
  and optional `reasoning_effort`, and returns `agent_id`;
- `wait_agent` accepts `targets: [agent_id]` and `timeout_ms`, and returns `status`
  plus `timed_out`;
- `send_input` targets an existing agent with `target` and `message`;
- `resume_agent` accepts the closed agent's `id`; and
- `close_agent` accepts the agent `target`.

If V1 is unavailable or any required field, result, or capability differs, return
BLOCKED before creating a worktree. Do not adapt to, fall back to, or approximate
V2 or another multi-agent protocol.

If no durable worker registry entry exists for the issue:

1. Re-read the exact Linear issue and use its current label names as the
   authoritative worker-configuration snapshot.
2. Independently collect labels with the exact prefixes `codex-model:` and
   `codex-reasoning:`. Require at most one label for each prefix and a non-empty
   suffix. If either prefix is duplicated or has an empty suffix, return BLOCKED
   without creating a subagent or worktree.
3. Treat this skill's explicit invocation as authorization to apply a
   `codex-model:<model-id>` label as the issue's explicit subagent `model`
   configuration, and a `codex-reasoning:<effort>` label as its explicit
   `reasoning_effort`. Preserve each suffix exactly; do not normalize, alias, or
   infer values. A missing prefix means omit only that override so the subagent
   uses its configured inheritance/default behavior.
4. Check `spawn_agent`'s current model and reasoning contract. If a requested value
   or combination is unsupported, return BLOCKED with the offending labels and
   create nothing. If spawning rejects the requested combination, likewise return
   BLOCKED; never retry by silently dropping or changing either override.
5. Reconcile Git worktrees, local/remote issue branches, and open PRs again. If no
   worker artifacts exist, choose and record a unique issue-identifying absolute
   path outside the primary checkout, verify the inherited V1 policy grants the
   required write access described above, and create one detached worktree at the
   verified canonical default-branch commit. Re-read the worktree list and require
   the new worktree to be registered, clean, detached at that exact commit, and
   distinct from every other issue worktree. If any issue worktree, branch, or PR
   already exists, leave this initial-dispatch path without spawning. Rehydrate and
   resume a surviving agent when its prior `agent_id`, provenance, and ownership are
   recovered and coherent; unknown model/reasoning metadata does not block
   same-agent resumption because `send_input` supplies no overrides and preserves
   its settings. Otherwise use the guarded replacement path only after proving the
   prior writer is closed, recovering the exact original model/reasoning settings,
   and satisfying all replacement preconditions; if they cannot pass, return
   BLOCKED. Never adopt existing artifacts using current issue labels and never
   create another worktree.
6. Call `spawn_agent` with `agent_type: "worker"`, `fork_context: false`, the exact
   orchestrator envelope above, and only the model/reasoning overrides supplied by
   the validated labels. The envelope assigns this issue and worktree as the
   worker's exclusive write scope and warns it that other agents may be active.
7. Record the returned `agent_id`, normalized worktree path, exact requested values
   or the exact inherited model/reasoning snapshot, and current durable Git/PR state
   before waiting. If an inherited value cannot be observed exactly, record it as
   `unknown`; never infer it from later issue labels. If the spawn result is
   ambiguous or missing an ID, reconcile state and return BLOCKED; never dispatch a
   duplicate.

If the V1 spawn call explicitly rejects before creating an agent after this workflow
created a new worktree, do not retry with altered settings. Re-read that worktree
and remove it only when it is still clean, detached at the recorded base commit,
and has no issue branch, process, or PR. Verify removal and return BLOCKED with the
original rejection. If any condition differs or spawn success is ambiguous,
preserve the worktree and registry and return BLOCKED.

If a registry entry has a `running` or `pending_init` subagent, wait for it instead
of sending duplicate input. If it has a closed subagent ID from an ordinary
completed blocker, recoverable interruption, or merge-ready result closed after a
failed or ambiguous merge, call `resume_agent` first. If it has an open completed or
interrupted ID, reuse that agent. Resume work with `send_input` and the exact
orchestrator envelope above. The existing subagent keeps its creation settings; do
not reinterpret changed labels during resumption.

Never resume an agent whose terminal cause was `errored`. After capturing its error
and closing it, retain the old ID only as provenance and spawn a replacement worker
against the same coherent worktree/branch/PR state under the replacement rules
below.

If no usable subagent ID survives but one coherent durable worker exists, require
both exact original model and reasoning values to be known, then spawn one
replacement `worker` subagent with those exact settings, the same worktree, and the
exact envelope. Record the replacement ID. If either setting is `unknown`, the
prior agent could still be running, or ownership is ambiguous, return BLOCKED
instead of changing worker configuration or risking concurrent writers.

Maintain a separate durable recovery fingerprint for replacement limits. Include
the issue, worktree path, branch, HEAD, hash of the complete worktree diff, PR URL/
head/state, and acceptance-criteria version; exclude agent IDs and agent statuses.
Allow at most one errored-agent replacement and one malformed-handoff correction
per unchanged durable recovery fingerprint. A new agent ID never resets either
allowance.

### 4. Wait for the worker result

Call `wait_agent` with `targets: [agent_id]` and a bounded `timeout_ms` of at most
60,000. If it returns `timed_out: true` with an empty status map, continue waiting
while providing concise root-task progress updates. Treat `pending_init` or
`running` as nonterminal. Observe V1 interruption through either the `wait_agent`
status map or the model-visible V1 subagent notification. On the first
`interrupted` observation for a fingerprint, call `send_input` once with the exact
orchestrator envelope and continue waiting. If the send fails or the same unchanged
fingerprint is interrupted again, call `close_agent`, preserve its ID and worktree
registry entry, and return BLOCKED. On the first `errored` result for a durable
fingerprint, capture the error, call `close_agent`, verify the prior writer is no
longer running, and reconcile the worktree, branch, PR, HEAD, and any uncommitted
diff. If that durable state is coherent and every dirty change is issue-owned,
both exact original model/reasoning settings are known, and no replacement has been
attempted for that durable recovery fingerprint, spawn one replacement worker with
those exact settings and the exact envelope, record its new ID and the consumed
allowance, and return to waiting. If either setting is unknown, replacement
spawning fails, the replacement errors against the same unchanged durable
fingerprint, or ownership is ambiguous, close any created terminal agent, preserve
the durable state, and return BLOCKED.
Treat `shutdown` as already closed and `not_found` as unavailable; preserve their
durable worker state and return BLOCKED until reconciled. Never interpret any of
these statuses as a completed worker report.

Accept only a completed status whose final text is exactly one of:

- `MERGE READY` with issue, PR URL, branch, exact head SHA, requirements, one
  acceptance-evidence row per criterion, CI, automatic Codex review, other review
  threads, local validation, local independent review, residual risks, and
  recommended merge method;
- `BLOCKED` with issue, phase, concrete blocker, evidence, safe work already
  completed, required decision/authority/external change, available options, and a
  recommended option.

If a completed status has missing, malformed, incomplete, or mismatched final text,
capture it. On the first such result for a durable recovery fingerprint, record the
consumed correction allowance, call `send_input` once with the exact orchestrator
envelope, and return to waiting so the same agent can reconstruct and return a
valid terminal report. If sending fails or another malformed result arrives for the
same unchanged durable fingerprint, call `close_agent`, preserve its ID and
worktree registry entry, and return BLOCKED in phase `worker`. Never infer merge
readiness from commentary, Git artifacts, or an open PR alone.

### 5. Handle a blocked worker

Keep the worker worktree available. If the worker reports `Phase: Invocation`, do
not assess the Linear issue. Compare the exact sent message with the required
four-line envelope. If the
root serialized it incorrectly, send the corrected envelope once to the same
pre-goal subagent and return to phase 4. If the sent message was already exact, or
if correction is rejected or repeats the blocker, capture the terminal result,
call `close_agent`, preserve its ID and worktree registry entry for diagnosis or
`resume_agent`, and return BLOCKED as a child-contract mismatch. Never replace the
issue or worktree to repair an invocation envelope.

For every other worker blocker, reassess the same issue once using
`$assess-issue ISSUE_ID`, incorporating the worker's evidence.

- If assessment returns `CLARIFY` or identifies or creates a blocking prerequisite,
  capture the terminal result, call `close_agent` to release its concurrency slot,
  preserve its ID and worktree registry entry for `resume_agent`, and return to
  `$find-next-issue`.
- If assessment returns `SPLIT`, first verify the worker has no unique commits,
  uncommitted changes, or open PR. Then close it, remove only its verified
  orchestrator-created worktree, re-read the worktree list, and remove its registry
  entry. If unique work exists or cleanup cannot be proved safe, close the terminal
  subagent while preserving its ID and worktree registry entry, then return BLOCKED
  rather than abandoning or reassigning it.
- If the worker reported changed acceptance criteria and assessment returns
  `READY`, resume the same worker and require fresh evidence.
- If assessment still reports READY without resolving any other worker blocker,
  close the terminal subagent while preserving its ID and worktree registry entry,
  then return BLOCKED because assessment and implementation evidence conflict.
- If assessment returns `BLOCKED` with `COORDINATION_PARENT`, close the terminal
  subagent while preserving its ID and worktree, then return BLOCKED as a selector/
  assessor contract mismatch.
- If assessment returns `BLOCKED` with `EXTERNAL` or `MUTATION_FAILURE`, close the
  terminal subagent while preserving its ID and worktree, report the evidence, and
  wait or return BLOCKED. Do not create another worker or silently broaden scope.
- If assessment returns any other malformed or unrecognized result, close the
  terminal subagent while preserving its ID and worktree, then return BLOCKED as a
  child-contract mismatch.

### 6. Recheck and merge a MERGE READY result

Immediately before merge, re-read the Linear acceptance criteria and current GitHub
state. Require exactly one worker evidence row for every criterion, with matching ID,
order, and text, excluding the checkbox marker itself.

If the criteria and worker rows differ, do not merge. Resume the same subagent with
the exact orchestrator envelope and require a fresh `MERGE READY` report.

When the criteria are unchanged, verify:

- the PR is open, non-draft, and targets the canonical default branch;
- current head SHA exactly equals the worker-reported SHA;
- the branch is current with the default branch and the PR is mergeable;
- every required check passes;
- automatic GitHub Codex review completed for that head through `eyes` followed by
  either `+1` or a terminal bot comment, with a no-findings comment equivalent to
  `+1`, and its delayed snapshot has no actionable findings;
- no actionable human review thread remains;
- the PR description cites the Linear issue and affected Mara IDs;
- the PR description contains exactly one acceptance-evidence row for every current
  criterion;
- every row is `PASS` with the required evidence, except an explicitly
  merge-dependent row may be `PENDING` with the exact post-merge evidence it needs;
- the worker’s validation and independent-review evidence is complete;
- the final independent reviewer evaluated the complete acceptance matrix for the
  current head and reported no actionable acceptance finding;
- no residual risk contradicts merge readiness.

Do not check Linear acceptance boxes before merge. Reconcile them only after GitHub
confirms that exact head was merged.

If state is stale or a condition regressed, do not merge. Resume the same subagent
with the exact orchestrator envelope. It re-reads the PR and authoritative state to
identify the regression. Wait for a fresh MERGE READY result.

Use the repository-authorized merge method, preferring the worker’s recommendation
only when it matches GitHub and project policy. Merge through the GitHub capability
with `expected_head_sha` set to the verified head. Never merge without that guard.

Confirm the API reports a successful merge, re-fetch the PR, and record the merged
commit. If merge confirmation fails or is ambiguous, close the terminal worker
subagent while preserving its ID and worktree registry entry, then return BLOCKED
until the merge state is reconciled; never retry blindly.

### 7. Verify automatic issue closure and synchronize main

After confirmed merge:

1. Require the worker result to be terminal. If its agent is not already closed,
   call `close_agent` with the recorded `agent_id`, verify it is no longer running,
   and preserve the ID and worktree registry entry for post-merge recovery.
2. Require the local control checkout to remain clean and on the default branch.
3. Run `git pull --ff-only` from the canonical remote and verify local HEAD contains
   the merged commit. Do not reset on failure.
4. Re-read the acceptance criteria and verify their text and order still match the
   evidence matrix, ignoring checkbox markers.
5. Verify the confirmed merge supplies the named evidence for every merge-dependent
   `PENDING` row and change those rows to `PASS`. If any row remains non-`PASS`,
   report BLOCKED with the merged commit.
6. Idempotently change each verified criterion's unchecked marker to checked while
   preserving all other issue-description content, then re-read the issue to verify
   the update. If criteria changed or the update failed, report BLOCKED with the
   merged commit.
7. Re-read the Linear issue until the merge automation moves it to a completed
   state, using bounded waits between checks.
8. Do not manually update the state of an implementation, clarification,
   prerequisite, or split-child issue. Manual closure is reserved for verified
   coordination parents and occurs only inside `$find-next-issue`.
9. Confirm the PR/merge evidence remains linked or otherwise discoverable from the
   issue. Do not add closing-keyword semantics to PR text.
10. Re-read the worker worktree's path, branch, current HEAD, and status. Require the
    path/provenance and branch to match the registry, require the status to be clean,
    and require current HEAD to equal both the worker-reported head and the exact PR
    head accepted by the successful guarded merge. Re-fetch the merged PR and
    require it to confirm that exact head and the recorded merged commit; require
    the synchronized default branch to contain the merged commit. This equality and
    merge evidence is required even for squash or rebase merges and proves that the
    worktree did not advance after review. Only then remove that exact
    orchestrator-created worktree with `git worktree remove` and re-read the
    worktree list to verify removal. Never delete the local issue branch here.
11. Remove its active registry entry, record the completed issue, acceptance
    reconciliation, PR, closed agent ID, and removed worktree, then return to
    `$find-next-issue`.

If checklist reconciliation or automatic Linear completion does not succeed within
the bounded wait, report BLOCKED with the merged commit. Do not manually close the
issue or dispatch another worker. If local synchronization fails, likewise report
BLOCKED and preserve state until control is reconciled.

## Prevent loops and duplicate work

Record a fingerprint after each selector, assessor, worker, merge, and Linear
mutation result. A fingerprint includes relevant issue states, acceptance evidence,
checklist reconciliation, dependency edges, agent IDs/statuses, worktree paths and
HEADs, selected model/reasoning, PR head, and local default-branch HEAD.

Do not repeat a phase against an unchanged fingerprint unless waiting on an
explicitly named external signal. Never create duplicate remediation issues,
duplicate worker worktrees or subagents, duplicate PRs, or duplicate merge attempts.

## Final completion audit

When `$find-next-issue` reports COMPLETE:

1. Re-list the complete in-scope Linear issue set without open-state filters.
2. Verify every implementation, clarification, prerequisite, split child, and
   coordination parent is in a completed state permitted by its contract.
3. Verify every implementation-bearing issue has confirmed merged GitHub evidence.
4. For every implementation-bearing issue completed during this run, verify its
   recorded acceptance matrix is fully `PASS` and its Linear checklist was
   reconciled after merge. Do not retroactively infer checklist evidence for issues
   completed before this run.
5. Verify every coordination parent closed during this run had its evidence-backed
   aggregate acceptance checklist reconciled by `$find-next-issue`.
6. Verify no running or open worker subagent remains, no orchestrator-created worker
   worktree remains, and no open Mara implementation PR remains.
7. Fast-forward and verify the local default branch one final time.
8. Invoke `$find-next-issue` once more against the unchanged final state and require
   a second COMPLETE result.
9. Complete the persistent goal only after this audit succeeds.

## Return terminal reports

Keep ordinary loop progress in commentary. End the turn only for genuine blockage
or final completion.

For blockage:

```text
BLOCKED — Iterate Linear
Phase: <selection, assessment, dispatch, worker, merge, Linear closure, or synchronization>
Issue: <ID or None>
Worker subagent: <agent ID or None>
Worker worktree: <absolute path or None>
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
Acceptance checklists reconciled: <implementation and coordination issue IDs>
Final default branch: <name and HEAD>
Open implementation PRs: None
Active worker subagents: None
Managed worker worktrees: None
Final scheduler result: COMPLETE confirmed twice
```
