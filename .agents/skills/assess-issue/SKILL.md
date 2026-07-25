---
name: assess-issue
description: Explicit-only workflow that assesses exactly one passed Mara Linear issue, automatically applies bounded Linear remediation for CLARIFY, SPLIT, and missing-prerequisite BLOCKED outcomes, and returns a structured READY, CLARIFY, SPLIT, or BLOCKED result. It never chooses a child, blocker, successor, or next issue. Use only when a user explicitly invokes `$assess-issue` followed by exactly one Linear issue identifier such as `CON-99`, or when an explicitly invoked `$iterate-linear` workflow issues that exact nested invocation; never invoke this skill by semantic matching or infer an issue from conversation context.
---

# Assess Issue

Assess exactly the issue named by the caller, apply the verdict’s authorized Linear
remediation, and return that issue’s result. Never select or assess another issue.

## Invocation contract

Accept only an explicit invocation in this form:

```text
$assess-issue CON-99
```

Treat the text following `$assess-issue` as the complete input. Require exactly one
identifier matching `^[A-Z][A-Z0-9]*-[0-9]+$`. Do not infer, repair, replace, or
redirect the identifier. If the input is invalid, stop and request one valid ID.

Never run implicitly. Explicit invocation authorizes only the bounded Linear
mutations below; it does not authorize repository edits, implementation, GitHub
mutations, coordination traversal, or next-issue selection.

When called by `$iterate-linear`, treat the exact nested invocation as explicit.
Return the structured result to that active orchestrator as a nonterminal phase
result; do not end the outer task. Standalone invocation returns the same report as
the entire final response.

## Mutation boundary

Use repository and GitHub access read-only. Do not edit files, create branches,
commits, pull requests, or goals, fetch or rewrite refs, run generators, or modify
GitHub state.

Permit only these Linear mutations after complete assessment and preflight:

- Create or reuse one clarification issue and make it block the passed issue.
- Create or reuse bounded split children, wire their dependencies, and convert the
  passed issue into a coordination parent.
- Create or reuse one missing-prerequisite issue and make it block the passed issue.

Do not close coordination parents, choose children, follow outgoing dependencies,
or modify unrelated Linear state. Those actions belong to `$find-next-issue`. Do not
hardcode team states, labels, workflows, or assignees.

## Gather authoritative evidence

1. Verify the current repository is Mara and identify its root without changing it.
2. Read root `AGENTS.md`, `docs/index.mara.md`, and applicable nested `AGENTS.md`.
3. Read the complete passed Linear issue: description, comments, attachments,
   status, team, project, milestone or cycle, parent, children, blockers, blocked
   issues, duplicates, and related issues.
4. Read linked issues only to understand the passed issue’s existing relations and
   completion signals. Never replace the assessment target with a linked issue.
5. Extract and read every Mara item linked or cited by the issue, comments,
   attachments, and relevant relations, including status and necessary document
   context.
6. Follow item references needed to assess acceptance, consistency, dependencies,
   decisions, and verification. Narrative and generated indexes are not authority.
7. Follow root `AGENTS.md` discovery delegation gates. Keep probes atomic and
   read-only; retain verdict and mutation decisions here.

Use the Mara corpus for durable semantics, Linear for delivery scope and dependency
state, and GitHub for implementation and merge evidence.

## Reject coordination parents as leaf targets

If the passed issue explicitly delegates implementation to children, do not select
a child and do not close the parent. Return BLOCKED stating that the passed issue is
a coordination parent, list its child IDs and statuses without ranking them, and
state that next-issue selection is outside this skill.

Having children without an explicit coordination role is a CLARIFY contract gap.

## Assess readiness

Check all of the following for the passed issue:

- It describes one bounded, coherent delivery outcome.
- Its complete relevant Mara contract is accepted, complete, and non-contradictory.
- It contains an explicit ordered acceptance checklist whose criteria are unchecked
  Markdown checkboxes that map to observable, testable behavior or evidence.
- Dependencies and blockers have resolved with required completion signals.
- The work fits one focused branch and pull request.
- No substantial product, architecture, schema, methodology, or verification
  decision must be invented.
- Splitting would not materially improve reviewability or sequencing.

Do not mark READY from detailed prose or Linear status alone. A worker must be able
to implement the passed issue without expanding scope or making an unapproved
decision.

## Make mutations idempotent

Complete assessment before writing. Immediately before a mutation, re-read the
passed issue and its relations; abandon the write if relevant state changed.

Before creating an issue, search the same team and project for matching assessment
provenance and verify its scope and relations. Include this section in each generated
issue:

```text
## Assessment provenance

- Origin issue: ISSUE_ID
- Kind: clarification | prerequisite | split-child
- Mara items: <IDs or None>
```

Reuse a compatible match. If provenance matches but scope or relations conflict,
return BLOCKED. After every write, re-read affected issues and verify fields and
relations from Linear state.

Resolve the team's unique state whose category is `backlog` before creating any
remediation issue. Create clarification, prerequisite, and split-child issues in
that dynamically resolved Backlog state rather than relying on the team's default.
If no unique Backlog state exists, return BLOCKED before writing. When reusing an
issue, preserve an existing active state; otherwise require it to be in a backlog or
unstarted category so `$find-next-issue` can schedule it.

## Prevent remediation recursion

When the passed issue has assessment provenance, preserve its declared remediation
kind and origin:

- A `clarification` issue is READY when it is a bounded task to decide or document
  the named gap, identifies the responsible authority, and has observable contract
  acceptance and merge evidence. The unresolved contract it exists to produce is its
  outcome, not a reason to create another clarification issue.
- If a clarification issue is itself unbounded, contradictory, or lacks a decision
  authority, return external `BLOCKED` against that same issue and origin. Never
  create a clarification issue to clarify a clarification issue.
- Assess `prerequisite` and `split-child` issues normally, but reuse existing
  provenance-linked remediation before creating anything new.

## Select and apply one outcome

Select exactly one outcome for the passed issue. Existing unresolved blockers take
precedence because they already represent the prerequisite. Otherwise use CLARIFY,
SPLIT, missing-prerequisite BLOCKED, external BLOCKED, then READY.

### Existing issue blocker: BLOCKED

When unresolved Linear issues block the passed issue, create nothing. Return every
blocking issue with status, URL, relation, and completion signal. Do not rank them,
choose one, or identify a next issue.

### Missing Mara contract: CLARIFY

Use CLARIFY when required Mara contract is missing, proposed, incomplete,
materially ambiguous, or contradictory.

Create or reuse one clarification issue in the same team and project containing:

- the exact gap or contradiction with Mara IDs and source documents;
- required decision or accepted contract text;
- bounded documentation scope and non-goals;
- an explicit ordered acceptance checklist of unchecked Markdown boxes, with
  required evidence for every criterion, including merged contract evidence when
  required;
- the signal for reassessing the passed issue;
- provenance with kind `clarification`.

Make it block the passed issue and verify both sides. Do not alter the passed issue’s
contract text. Return CLARIFY only after creation/reuse and relation verification;
otherwise return BLOCKED with the partial mutation state.

### Over-broad outcome: SPLIT

Use SPLIT only when accepted contract supports complete, independently reviewable
outcomes and decomposition materially improves reviewability or sequencing. Do not
split an issue that is started, In Review, linked to an open implementation PR, or
already has incompatible children; return BLOCKED.

Define every child’s neutral title, complete scope, non-goals, Mara IDs, explicit
ordered acceptance checklist of unchecked Markdown boxes, verification evidence,
and dependency DAG before writing. Create or reuse children in topological order
with:

- the passed issue as `parentId`;
- inherited team, project, milestone, and priority when present;
- only semantically valid labels or assignment fields;
- `blockedBy` relations to prerequisite children;
- provenance with kind `split-child`.

After all children verify, update the passed issue while preserving objective, Mara
links, non-goals, and history. Add an explicit coordination-parent role, child list,
dependency order, prohibition on parent-level implementation, and an ordered
aggregate acceptance checklist of unchecked Markdown boxes whose criteria can be
verified from child and merge evidence.

Return SPLIT only after every child, relation, and parent conversion verifies. If a
write fails, stop and return BLOCKED with created/reused IDs, failed operation, and
recovery action. Never choose which child should execute next.

### Missing implementation prerequisite: BLOCKED

When accepted Mara contract clearly requires a concrete prerequisite but no Linear
blocker represents it, create or reuse one bounded prerequisite issue in the same
team and project. Include the prerequisite outcome, reason it blocks, controlling
Mara IDs, scope, non-goals, an explicit ordered acceptance checklist of unchecked
Markdown boxes, verification criteria, completion signal, and provenance kind
`prerequisite`.

Make it block the passed issue and verify both sides. Return the prerequisite issue
without selecting it as next. If it requires an unapproved product decision, create
a clarification issue instead.

### External or non-issue blocker: BLOCKED

For authorization, external service, unavailable evidence, active work, routing
ambiguity, or another blocker that should not become an implementation issue,
create nothing. Return the blocker, evidence, responsible authority when known, and
observable completion signal.

### Ready implementation: READY

Return READY only when every readiness gate passes. Make no Linear mutation and
return only the passed issue’s readiness signal and evidence. Another workflow owns
next-issue selection and implementation dispatch.

## Return one structured result

Produce exactly one report with no preamble or trailing text. For standalone
invocation, make it the entire final response. For nested invocation by
`$iterate-linear`, hand it back as the orchestrator's nonterminal assessment phase
result and continue the outer workflow.

For READY:

```text
READY — ISSUE_ID: <brief evidence-based reason>
Linear mutations: None
```

For CLARIFY:

```text
CLARIFY — ISSUE_ID: <exact Mara contract gap>
Clarification issue: CLARIFICATION_ID — <title> — <URL>
Mutation: <Created or Reused>; verified to block ISSUE_ID
Dependency: CLARIFICATION_ID -> ISSUE_ID
Completion signal: <accepted contract and merged evidence required before reassessment>
```

For SPLIT:

```text
SPLIT — ISSUE_ID converted to a coordination parent: <brief reason>
Issues:
1. CHILD_ID — <title> — <URL> — <Created or Reused>
2. CHILD_ID — <title> — <URL> — <Created or Reused>
Execution order: <topological order, including independent groups>
```

For BLOCKED:

```text
BLOCKED — ISSUE_ID: <concrete blocker, coordination-parent boundary, or failed mutation>
Blocker kind: <EXISTING_ISSUE | CREATED_PREREQUISITE | COORDINATION_PARENT | EXTERNAL | MUTATION_FAILURE>
Blocking issues: <all IDs, titles, URLs, statuses, and relations; or None>
Prerequisite issue: <ID, title, URL, and Created/Reused; or None>
Children: <IDs and statuses when ISSUE_ID is a coordination parent; or None>
Partial mutations: <verified changes requiring recovery, or None>
Completion signal: <observable resolution condition>
```
