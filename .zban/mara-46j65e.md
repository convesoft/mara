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
