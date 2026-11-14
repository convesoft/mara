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

Context: docs/rules.mara.md (REQ-PROJECT-AUTHORED-LOGIC, REQ-EXTERNAL-RECORDS-AS-SUPPLIED, REQ-PROGRAMS-ONLY-COMPUTE, DES-RULE-CONSUMERS, DES-GATE-OUTCOME, DES-REQUEST-INPUTS); docs/rules-facts.mara.md (DES-PROGRAM-PROFILE, RISK-SOUFFLE-REACHES-OUTSIDE); docs/rules-evaluation.mara.md (REQ-EXPLAIN-OUTCOMES, REQ-PROGRAM-FAULTS-REPORTED, REQ-EVALUATION-TIME-LIMIT, DES-RULE-EXECUTION, EVD-SOUFFLE-PROGRAM-REACH). The rows of EVD-SOUFFLE-PROGRAM-REACH are reproductions from four independent reviews; each one becomes a test here.

- [ ] Scan on private copies, by Soufflé's own lexical rules, that admits only the listed directives, relation qualifiers, types and functions (DES-PROGRAM-PROFILE)
- [ ] Generated wrapper with JSON `.input` and `.output` directives: every fact relation and supplied input, and every relation that a configuration entry names in the program; Soufflé runs in the private directory with `-l ""` (DES-RULE-EXECUTION)
- [ ] Child-process evaluation keyed by the layers read, the program with its includes, and the supplied inputs
- [ ] Invariants in validation; named queries, views and gates; `query execute`, `view`, `gate`, `report` and `list` in CLI and MCP
- [ ] Gate outcome per named item from subject, condition and unavailable relations (DES-GATE-OUTCOME)
- [ ] Time limit (REQ-EVALUATION-TIME-LIMIT)
- [ ] Compile errors, refused programs, timeouts, runtime faults and anything Soufflé prints on stderr while evaluating are reported; never a passing outcome
- [ ] Explanations from provenance mode with document locations: `subproof` references expanded, the returned proof checked against the requested row, and "cannot explain" for unwritable values, invalid replies, aborts and timeouts (REQ-EXPLAIN-OUTCOMES)
- [ ] Tests, reaching outside: a `libfunctors.so` beside a harmless program or on the library path is not loaded; `#line` at the start of a line, after other text and after a block comment is refused; `__FILE__` is refused; no diagnostic shows another file
- [ ] Tests, input and output: the `output`, `printsize` and `input` qualifiers and the I/O directives are refused; nothing but the generated files is written
- [ ] Tests, determinism: `choice-domain`, `ord`, a bare `$` and `autoinc()` are refused; `float` and `unsigned` are refused; two consumers of one program get the same rows
- [ ] Tests, faults: a bad `match` pattern and an out-of-range `substr` make the outcome unavailable, natively and in WASM
- [ ] Tests, explanations: a row whose value holds a newline or a quote is reported as not explainable instead of explaining another row
- [ ] VER-WORK-COUNTERS rows for `souffle` runs pass
