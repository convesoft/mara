---
name: find-next-issue
description: Explicit-only, parameterless Linear delivery scheduler for Mara. It loads the complete Linear issue set, reconciles coordination parents and dependency state, closes verified-complete coordination parents, and returns one globally next available delivery issue without assessing Mara readiness or creating issues. Use only when a user explicitly invokes `$find-next-issue` with no arguments, or when an explicitly invoked `$iterate-linear` workflow issues that exact nested invocation; never invoke this skill by semantic matching.
---

# Find Next Issue

Load the complete Linear delivery graph and return one globally next available Mara
issue. Do not assess implementation readiness.

## Invocation contract

Accept only this exact parameterless invocation:

```text
$find-next-issue
```

If any identifier or other argument follows the skill name, stop and state that the
skill takes no parameters. Never infer a starting issue or narrow scope from
conversation context.

Never run implicitly. Explicit invocation authorizes only verified coordination-
parent closure; all other access is read-only.

When called by `$iterate-linear`, treat the exact nested invocation as explicit.
Return the structured result to that active orchestrator as a nonterminal phase
result; do not end the outer task. Standalone invocation returns the same report as
the entire final response.

## Boundary

Do not read Mara contracts to decide READY/CLARIFY/SPLIT, create or edit delivery
issues, create blocker relations, edit repository files, mutate GitHub, or start
implementation. `$assess-issue` owns readiness and verdict remediation after this
skill returns a candidate.

The only permitted mutation is moving an explicit coordination parent to the team’s
resolved completed status after every child, blocker, aggregate criterion, and
required merge-evidence check passes.

## Load every Linear issue

1. Verify the current repository is Mara and read root `AGENTS.md` and
   `docs/index.mara.md` completely without changing repository state.
2. Page through the Linear issue list without an issue, parent, state, assignee,
   cycle, priority, or archival filter until every workspace issue, including
   archived issues, is loaded. If the API separates active and archived queries,
   exhaust both result sets.
3. Include completed issues because they establish dependency satisfaction.
4. Resolve the Mara delivery scope from current repository links, Linear project
   metadata, issue descriptions, and attachments. Load all workspace issues before
   applying this eligibility boundary.
5. Exclude an issue from candidacy only when evidence clearly places it outside the
   Mara repository’s delivery scope. If repository scope is ambiguous, return
   BLOCKED rather than selecting across unrelated products.
6. Fetch complete details and relations for every in-scope non-completed issue,
   every coordination parent, and every issue referenced by their dependency edges.
7. Read linked GitHub evidence needed to distinguish merged delivery from a Linear
   status label alone.
8. Track all IDs and edges; return BLOCKED on cycles, dangling relations,
   contradictory directions, or incomplete pagination.

Do not search only recent, assigned, high-priority, or parented issues. Selection
must be based on the complete in-scope graph.

## Classify delivery state

Treat an issue as:

- `completed` only when its Linear status is completed and repository-required
  implementation and verification evidence is merged;
- `available` when its Linear state category is backlog or unstarted, it is not
  canceled or a coordination parent, and every blocker is completed;
- `active` when it is started or In Review;
- `blocked` when a predecessor or external completion signal remains unresolved;
- `coordination` only when its description explicitly delegates implementation to
  children.

Do not count cancellation as completion unless the coordination contract explicitly
permits that outcome.

Do not silently omit archived in-scope issues. Treat an archived issue as completed
only when its completed state and required merged evidence verify normally. If an
in-scope issue was archived while still incomplete, return BLOCKED with the required
restore, completion, or contract-authorized cancellation signal; never select it as
active implementation work and never report COMPLETE around it.

## Reconcile coordination parents

For every explicit coordination parent:

1. Load all direct children and the declared dependency order.
2. Leave the parent open while any child is open, active, blocked, lacks merged
   evidence, or is impermissibly canceled.
3. When all children are completed, verify every parent blocker, aggregate
   completion criterion, and required merge artifact.
4. Immediately before writing, re-read the parent, every direct child, every
   blocker, all dependency relations, aggregate completion signals, and required
   merge evidence. If anything changed, abandon the write and recompute the graph.
5. Resolve the team’s unique completed state, move the parent to it, and re-read the
   result. Skip the write if already completed.
6. Record every verified parent transition for the final result.

If completion contract or evidence is ambiguous, return BLOCKED. Do not select the
parent itself as implementation work.

After any parent transition, re-fetch affected dependency relations and recompute
the complete graph before candidate selection.

Parent closure is reconciliation performed inside the same invocation, not a
standalone result. After closing one or more parents, continue through candidate
selection and return exactly one normal `NEXT`, `WAITING`, `COMPLETE`, or `BLOCKED`
report. Include the closed parent IDs in `Coordination updates`; never return a
closed parent as the selected issue.

## Build the candidate set

Consider every in-scope issue after reconciliation.

- Exclude completed, canceled, active, and coordination issues.
- Exclude issues with any unresolved issue or external blocker.
- Exclude children whose explicit earlier sibling or prerequisite remains
  incomplete.
- Include every remaining available issue, even if it is independent of other
  available work.

If no candidate exists:

- Return WAITING when active or blocked work remains, listing all active issues and
  the unresolved blockers preventing availability.
- Return COMPLETE when every in-scope delivery issue and coordination parent is
  verified completed and no work remains.
- Return BLOCKED when the graph, scope, state, or evidence is inconsistent.

## Choose exactly one candidate

Rank available candidates using only durable Linear delivery state, in this order:

1. Explicit dependency and coordination-parent order.
2. Linear priority, with Urgent before High, Medium, Low, and no priority.
3. Ascending project milestone order, then earliest target date, when defined and
   comparable.
4. Ascending cycle order, then earliest due date, when defined and comparable.
5. Oldest creation time first.
6. Lowest numeric issue identifier as the final deterministic tie-breaker.

For every optional ranking field, defined values sort before missing values. When
defined values belong to otherwise incomparable projects, milestones, teams, or
cycles, sort first by canonical scope ID and then by the stated ascending order or
date. Use stable IDs as the final comparison within that criterion and never skip a
criterion only for one candidate pair.

Do not use title wording, estimated code difficulty, assumed business value, model
preference, or repository proximity as hidden ranking criteria. Explain the winning
comparison in the result.

Return the candidate without assessing whether its Mara contract is READY. Another
workflow decides that.

## Return one result

Produce exactly one report. For standalone invocation, make it the entire final
response. For nested invocation by `$iterate-linear`, hand it back as the
orchestrator's nonterminal selection phase result and continue the outer workflow.

For a candidate:

```text
NEXT — ISSUE_ID — <title> — <URL>
Reason: <durable ranking facts that selected this issue>
Dependencies: <completed prerequisite IDs, or None>
Coordination parent: <ID, or None>
Coordination updates: <parents moved to completed, already completed, or None>
Candidate count: <number of available in-scope issues>
```

For delivery that cannot advance yet:

```text
WAITING — no issue is currently available
Active issues: <all IDs, statuses, and URLs; or None>
Blocking issues: <all unresolved blocker IDs, statuses, and URLs; or None>
External blockers: <signals and responsible authority; or None>
Completion signal: <observable state required before retry>
Coordination updates: <parents moved to completed, or None>
```

For a fully completed delivery graph:

```text
COMPLETE — no Mara delivery issue remains
Completed issue count: <count>
Coordination updates: <parents moved to completed, already completed, or None>
```

For inconsistent scope or graph state:

```text
BLOCKED — cannot select a deterministic next issue
Evidence: <scope ambiguity, cycle, dangling edge, evidence gap, or failed parent transition>
Required correction: <observable correction>
Coordination updates: <verified mutations, or None>
```
