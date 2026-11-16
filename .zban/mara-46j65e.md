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
