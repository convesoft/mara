---
title: "Explanations: query report"
status: todo
type: feature
parent: mara-17tdb3
blocked_by:
  - mara-bx82k0
created_at: 2026-11-16T16:11:57Z
---

Goal: `query report` explains one row of an invariant or gate outcome, down to the documents and supplied inputs it rests on, or says that it cannot.

Split by the orchestrator on 2026-11-16 from mara-bx82k0, which builds the execution this issue calls. The query operations (`query execute`, `view`, `gate`, `list`) are a sibling issue; this one needs their command group and the gate outcome only for the gate rows, so it can start on invariants as soon as mara-bx82k0 is integrated.

Context: REQ-EXPLAIN-OUTCOMES and DES-RULE-EXECUTION in docs/rules-evaluation.mara.md; DES-RULE-CONSUMERS in docs/rules.mara.md; VER-EXPLANATIONS in docs/verification-rules.mara.md.

- [ ] Explanations from provenance mode (`-t explain`, driven on stdin with `format json`) with document locations: `subproof` references expanded, the returned proof checked against the requested row, constraint leaves left out
- [ ] A leaf from a supplied input names that input and holds the row's values as supplied (REQ-EXPLAIN-OUTCOMES as amended; decided by the maintainer on 2026-11-16)
- [ ] "Cannot explain" for values the `explain` command cannot write (quotes, backslashes, control characters, negative numbers), invalid replies, a branch that ends in an aggregate, aborts, and the time limit; the outcome stands either way
- [ ] `query report` in the CLI and as an MCP tool, for rows of invariants and of gates
- [ ] Tests, natively: the cases of VER-EXPLANATIONS, and the `query report` cases of VER-PROGRAM-FAULTS and VER-EVALUATION-TIME-LIMIT (docs/verification-programs.mara.md). The checks that no admitted program can reach (a returned proof that is not for the requested row, a reply that is not JSON, an aborted provenance run) are verified by inspection in this issue's review (decided by the orchestrator on 2026-11-16; no fault-injection hook is added). Browser rows wait for mara-xsm8w6
- [ ] The `query report` row of VER-CLI-MCP-PARITY, its cold row of VER-WORK-COUNTERS (1 program run, in provenance mode) and its VER-SCALING case pass
- [ ] The agent guide names `query report` within its size limit

What mara-bx82k0 exposes: `mara::rules::evaluate` writes the private copies and wrapper and runs Soufflé through `spawn_and_wait`, the one place Mara starts a process (mara-ptq6xc will wrap it). Provenance mode needs that function to take extra arguments (`-t explain`) and a standard input instead of `Stdio::null()`; the limits, the private directory and `evaluation::interpret` apply unchanged.
