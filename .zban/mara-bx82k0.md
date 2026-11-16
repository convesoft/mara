---
title: "Evaluate programs: invariants, query, view, gate, report"
status: todo
type: feature
parent: mara-17tdb3
blocked_by:
  - mara-n3nx6d
  - mara-vt2ktq
  - mara-y4asgm
  - mara-wrbjte
created_at: 2026-11-13T22:10:06Z
---

Context: docs/rules.mara.md (REQ-PROJECT-AUTHORED-LOGIC, REQ-EXTERNAL-RECORDS-AS-SUPPLIED, REQ-PROGRAMS-ONLY-COMPUTE, DES-RULE-CONSUMERS, DES-GATE-OUTCOME, DES-REQUEST-INPUTS); docs/rules-facts.mara.md (DES-PROGRAM-PROFILE, RISK-SOUFFLE-REACHES-OUTSIDE); docs/rules-evaluation.mara.md (REQ-EXPLAIN-OUTCOMES, REQ-PROGRAM-FAULTS-REPORTED, REQ-EVALUATION-TIME-LIMIT, DES-RULE-EXECUTION, EVD-SOUFFLE-PROGRAM-REACH). The rows of EVD-SOUFFLE-PROGRAM-REACH are reproductions from four independent reviews; each one becomes a test here.

- [ ] Only programs that the parser admits run (mara-wrbjte), on the parsed copies in a private directory (DES-PROGRAM-PROFILE)
- [ ] Generated wrapper with JSON `.input` and `.output` directives: every fact relation and supplied input, and every relation that a configuration entry names in the program; Soufflé runs in the private directory with `-l ""` (DES-RULE-EXECUTION)
- [ ] Child-process evaluation keyed by the layers read, the program with its includes, and the supplied inputs
- [ ] Invariants in validation; named queries, views and gates; `query execute`, `view`, `gate`, `report` and `list` in CLI and MCP
- [ ] Gate outcome per named item from subject, condition and unavailable relations (DES-GATE-OUTCOME)
- [ ] Time limit (REQ-EVALUATION-TIME-LIMIT)
- [ ] Compile errors, refused programs, timeouts, runtime faults and anything Soufflé prints on stderr while evaluating are reported; never a passing outcome
- [ ] Explanations from provenance mode with document locations: `subproof` references expanded, the returned proof checked against the requested row, and "cannot explain" for unwritable values, invalid replies, aborts and timeouts (REQ-EXPLAIN-OUTCOMES)
- [ ] Tests, reaching outside: a `libfunctors.so` beside a harmless program or on the library path is not loaded; `#line` at the start of a line, after other text and after a block comment is refused; `__FILE__` is refused; no diagnostic shows another file
- [ ] Tests, input and output: the `output`, `printsize` and `input` qualifiers and the I/O directives are refused; nothing but the generated files is written
- [ ] Tests, determinism: two consumers of one program get the same rows; the admitted functions and operators give the same results natively and in WASM on the boundary cases of the review probes
- [ ] Tests, faults: an out-of-range `substr` and `to_number` of text that is not a number make the outcome unavailable, natively and in WASM; `to_number("7junk")` is 7 and a negative `substr` length means the rest of the text, on both
- [ ] Tests, explanations: a row whose value holds a newline, a quote or a negative number, and a proof that ends in an aggregate, are reported as not explainable; constraint leaves are left out
- [ ] VER-WORK-COUNTERS rows for `souffle` runs pass

Decided by the orchestrator on 2026-11-16 (from mara-t2qrcb, under the maintainer's rule of that day; listed in mara-46j65e): an evaluation gets limits on output size and memory besides time. A finite program wrote 8 MB in 15 ms in a review probe. Rests on REQ-PROGRAMS-ONLY-COMPUTE, RISK-SOUFFLE-REACHES-OUTSIDE and REQ-EVALUATION-TIME-LIMIT. The sandbox half of that question is mara-ptq6xc.

- [ ] Limits on the output size and the memory of an evaluation: exceeding one ends the evaluation and is reported as a fault, never a passing outcome (REQ-PROGRAM-FAULTS-REPORTED). The worker chooses the numbers and the mechanism, records them in DES-RULE-EXECUTION, and removes the limits part of the open question on docs/rules-facts.mara.md
