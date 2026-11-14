---
title: "Evaluate programs: invariants, query, view, gate, report"
status: todo
type: feature
parent: mara-17tdb3
blocked_by:
  - mara-n3nx6d
  - mara-vt2ktq
  - mara-y4asgm
created_at: 2026-11-13T22:10:06Z
---

Context: docs/rules.mara.md (REQ-PROJECT-AUTHORED-LOGIC, REQ-EXTERNAL-RECORDS-AS-SUPPLIED, DES-RULE-CONSUMERS, DES-GATE-OUTCOME); docs/rules-evaluation.mara.md (REQ-EXPLAIN-OUTCOMES, REQ-PROGRAM-FAULTS-REPORTED, DES-RULE-EXECUTION).

- [ ] Child-process evaluation keyed by the layers read, the program with its includes, and the supplied inputs
- [ ] Invariants in validation; named queries, views and gates; `query execute`, `view`, `gate`, `report` and `list` in CLI and MCP
- [ ] Gate outcome per named item from subject, condition and unavailable relations (DES-GATE-OUTCOME)
- [ ] Explanations from provenance mode with document locations, or reported as unavailable when Soufflé cannot explain the program (REQ-EXPLAIN-OUTCOMES)
- [ ] Allow-list scan of directives on private copies, by Soufflé's own lexical rules: no `.input`, `.output` or `.printsize`, no `#` outside strings and comments, no `__FILE__`, `__INCL__` or `autoinc()` (REQ-PROGRAMS-ONLY-COMPUTE, DES-PROGRAM-PROFILE)
- [ ] Generated wrapper with JSON `.input` and `.output` directives for exactly the relations a run needs; Soufflé runs in the private directory with `-l ""` (DES-RULE-EXECUTION)
- [ ] Tests: a `libfunctors.so` beside a harmless program, or on the library path, is not loaded; `#line` at the start of a line, after other text and after a block comment is refused, and no diagnostic shows another file; `__FILE__` is refused
- [ ] Time limit (REQ-EVALUATION-TIME-LIMIT)
- [ ] Compile errors, runtime faults, refused programs and timeouts reported; never a passing outcome
- [ ] VER-WORK-COUNTERS rows for `souffle` runs pass
