---
title: Review the decisions workers made in alpha.3
status: todo
type: task
tags:
  - maintainer
blocked_by:
  - mara-17tdb3
created_at: 2026-11-14T11:14:21Z
---

Goal: the maintainer reviews what workers decided on their own in the alpha.3 slice.

Context: AGENTS.md (Deciding during the rewrite). When the alpha.3 epic (mara-17tdb3) closes, the orchestrator appends the `## Decisions made` list of each of its issues below. This issue blocks the release workflow, not the next slice. No worker starts it.

- [ ] Every listed decision accepted, or overturned by a follow-up issue

## Decided by the orchestrator

One line per decision made on the maintainer's behalf under the rule of 2026-11-16: what was decided, what it rests on, how to undo it. "Provisional" marks a decision that is the maintainer's to confirm and stands until they answer.

- 2026-11-16, provisional, from mara-t2qrcb: the `souffle` child process runs in an operating-system sandbox on Linux (mara-ptq6xc), instead of a Decision that accepts RISK-SOUFFLE-REACHES-OUTSIDE. Rests on that risk (level HIGH), REQ-PROGRAMS-ONLY-COMPUTE and EVD-SOUFFLE-PROGRAM-REACH. Undo: cancel mara-ptq6xc and record a Decision that `Accepts` the risk.
- 2026-11-16, from mara-t2qrcb: an evaluation gets limits on output size and memory besides time (an item of mara-bx82k0). Rests on REQ-PROGRAMS-ONLY-COMPUTE, RISK-SOUFFLE-REACHES-OUTSIDE and REQ-EVALUATION-TIME-LIMIT. Undo: drop that item of mara-bx82k0.
- 2026-11-16, provisional, from mara-b60px3: the four views the first `engineering` template leaves out (outcome coverage and validation, engineering basis and rationale, knowledge validity, change impact) are deferred to after 0.4.0 (backlog mara-7h4z9f); mara-tx0fcf says so in DES-TEMPLATE-VIEWS. Rests on DES-TEMPLATE-VIEWS, which ships three views, and on a view being addable later without breaking a project. Undo: pull mara-7h4z9f, or one view of it, into a slice.

### Confirmed by the maintainer on 2026-11-16

The two provisional decisions above and one more were put to the maintainer as questions; each answer is the chosen option with its description, word for word. They are no longer provisional.

- Sandbox (mara-ptq6xc). Question: "Provisional decision 1 of 3, nothing waits on your answer. A rule program from an untrusted checkout could get past Mara's scans or hit a memory fault in Soufflé and then read or write files as the user (risk level HIGH in the docs). I decided to run the `souffle` child process in an operating-system sandbox on Linux, as its own issue before the release. Confirm?" Answer: "Sandbox on Linux (Recommended) — Keep my decision: a Landlock sandbox around the child process on Linux, built before 0.4.0; macOS follows with the macOS work. Costs one issue of release work, and closes the risk instead of accepting it." Not chosen: accepting the risk; a sandbox after 0.4.0.
- Four views (mara-7h4z9f). Question: "Provisional decision 2 of 3, nothing waits on your answer. The first `engineering` template ships three views (delivery readiness, requirement verification, risk treatment). You asked to come back to the other four designed views before the release: outcome coverage and validation, engineering basis and rationale, knowledge validity, change impact. I deferred all four to after 0.4.0. Confirm?" Answer: "Defer all four (Recommended) — Keep my decision. A view can be added in a later release without breaking any project, and 0.4.0 ships sooner. They are collected in one backlog issue." Not chosen: shipping some in 0.4.0; dropping them.
- Explanations (mara-wamxz3, which amends REQ-EXPLAIN-OUTCOMES). Question: "Provisional decision 3 of 3, nothing waits on your answer. REQ-EXPLAIN-OUTCOMES (Accepted) says an explanation's leaves are input facts with document path and line. The template's gates also consume records supplied from outside, such as test results, which have no document location. Under the current wording such an outcome could never be explained. I amended the requirement so that a leaf from a supplied input names that input. Confirm?" Answer: "Leaf names the input (Recommended) — Keep my decision: an explanation may end in a supplied record and names the input it came from. The outcomes users most need explained, the release gates, can then be explained." Not chosen: keeping the old wording.

### Decided by the orchestrator, continued

- 2026-11-16, mara-wamxz3: the explanation checks that no admitted program can reach (a proof that is not for the requested row, a reply that is not JSON, an aborted provenance run) are verified by inspection when mara-bx82k0 is reviewed. Rests on ADR-DEBUG-PAUSE-POINT, which keeps test-only hooks to the writer. Undo: a debug-only hook, which needs the maintainer's approval.
- 2026-11-16, mara-wamxz3: REQ-RUN-WITHOUT-TOOLCHAIN gets its Verification item in mara-haczya (0.4.0), not in alpha.3. Rests on the requirement being about the npm package, which mara-a4m588 delivers. Undo: add the item in alpha.3.
- 2026-11-16: mara-wamxz3 was started before the alpha.2 epic closed, since it writes documents only. Undo: not needed.

### Decided by the orchestrator, 2026-11-16, alpha.3 work

- mara-wrbjte: VER-ADMITTED-LANGUAGE establishes that an admitted program compiles (exits 0 with no compile diagnostic), not that a run prints nothing; four admitted fixtures print runtime warnings, which are faults and belong to VER-PROGRAM-FAULTS. Rests on REQ-PROGRAMS-ONLY-COMPUTE and REQ-PROGRAM-FAULTS-REPORTED. Undo: restore the empty-stderr criterion.
- mara-wrbjte: two findings the reviewer rated exotic were fixed anyway, because the parser's input comes from checkouts the user did not write: component expansion counts inherited bodies before expanding them, and every recursive step counts against one depth limit, so no program text exhausts memory or the stack. Rests on RISK-SOUFFLE-REACHES-OUTSIDE. Left to the backlog: quadratic resolution of very large forward-declared unions, and repeated inheritance colons that Soufflé accepts (mara-gz2qp5). Undo: pull the issue.
- mara-wrbjte: mara-stpjjc (compile the admitted fixtures with the pinned Soufflé) and mara-x0qc0q (Soufflé names an included file by its base name) were folded into mara-bx82k0. Undo: not needed.
- mara-y4asgm: left to the backlog: a line number above 2,147,483,647 is clamped in the exported facts (mara-n0q4k3). Undo: pull the issue.
- mara-msymag: a heading followed by an item table is an item wherever it stands, so a plain rendered example of an item on a conventions page is reported and `project fix` makes it an item; examples of items belong in a fenced code block. Rests on DES-REGIONS, DES-ITEM-STRUCTURE and REQ-REPORT-EVERY-PROBLEM: the other reading leaves real hand-written items silent. An item's table is recognized by a `Type` row plus an `ID` or `Status` row of any value; a custom HTML anchor before the heading is reported and split off, and fix adds no anchor next to it. Undo: a new issue that adds an explicit marker for examples.
- mara-bx82k0 was split into three issues with one outcome each: execution and invariants (mara-bx82k0), the query operations (mara-gfb6n6) and explanations (mara-jm5ecb). mara-tx0fcf now also waits for mara-gfb6n6. Undo: not needed.
- alpha.3 work was started beside the last alpha.2 issues, and mara-vt2ktq, mara-wrbjte and mara-y4asgm ran in parallel. Undo: not needed.

- 2026-11-16, mara-bx82k0 (review): an entry that names a relation its program does not declare, or one that cannot cross the boundary (`undeclared-relation`, `invalid-relation`), does not make `item list`, `project fix` and the other operations that evaluate nothing refuse the project. Validation reports it, and the operations that evaluate the entry fail on it, as on a compile error. Rests on DES-RULE-CONSUMERS (these diagnostics exist only against the admitted program) and on VER-WORK-COUNTERS (a request that evaluates nothing does no program work). Undo: have every operation admit the declared programs before it runs.
- 2026-11-16, mara-bx82k0 (review): two findings that need an unusual program (a compile error's caret after an `.include` on the same line; an invalid entry name hiding the table's other errors) are fixed only if small, otherwise they go to the backlog. An include cycle that hid a compile error is fixed, because validation said "valid". Undo: pull the backlog issues.

- 2026-11-16, mara-gfb6n6 (review): a gate request on a project whose configuration or schema has an error is refused as `invalid-project` with exit status 1 and no gate or item outcome; "unavailable" is only the outcome of a gate whose evaluation failed. Rests on DES-PROJECT-CONFIGURATION (every operation except validation refuses such a project) and REQ-PROGRAM-FAULTS-REPORTED (a refusal is no passing outcome); DES-QUERY-RESULTS says so. Undo: report the gate and every named item as unavailable there.

- 2026-11-16, mara-2dnzjc (review): crash records where the core pattern names a socket are not covered now; backlog issue mara-8dtsks holds the options (ignore SIGXFSZ and read the output limit from file sizes; a non-dumpable Soufflé through a build patch; a cgroup limit). The 1-byte soft core limit covers piped handlers and file patterns, which is this machine's configuration. Undo: pull mara-8dtsks.

- 2026-11-16: mara-ptq6xc (the Linux sandbox around Soufflé, a release-slice issue) was started on a free worker slot before the earlier slices closed; it needs only the evaluation of mara-bx82k0. Undo: not needed.
