---
title: "Evaluate programs: execution, invariants in validation, limits and faults"
status: in-progress
type: feature
parent: mara-17tdb3
blocked_by:
  - mara-n3nx6d
  - mara-vt2ktq
  - mara-y4asgm
  - mara-wrbjte
created_at: 2026-11-13T22:10:06Z
---

Goal: `project validate` evaluates a project's invariants with Soufflé, safely: only admitted programs run, in a private directory, within limits, and every failure is reported.

Split by the orchestrator on 2026-11-16 into three issues, so that each has one outcome and one reviewable change: this one (execution and invariants), the query operations (see "Blocks"), and explanations. The other two build on what this one exposes.

Context: docs/rules.mara.md (REQ-PROJECT-AUTHORED-LOGIC, REQ-PROGRAMS-ONLY-COMPUTE, DES-RULE-CONSUMERS, DES-REQUEST-INPUTS); docs/rules-facts.mara.md (DES-PROGRAM-PROFILE, DES-FACT-BRIDGE, RISK-SOUFFLE-REACHES-OUTSIDE); docs/rules-evaluation.mara.md (REQ-PROGRAM-FAULTS-REPORTED, REQ-EVALUATION-TIME-LIMIT, DES-RULE-EXECUTION, EVD-SOUFFLE-PROGRAM-REACH). The rows of EVD-SOUFFLE-PROGRAM-REACH are reproductions from independent reviews; each one becomes a test, here for invariants and in the query-operations issue for queries and gates.

Built before this issue: the parser that admits programs (`mara_engine::program`, mara-wrbjte), the fact bridge (`Snapshot::facts`, `MARA_DL`, `facts::check_inputs`, `rules::write_facts`, `rules::supplied_input`, mara-y4asgm), and the pinned Soufflé binary (`scripts/build-souffle.sh`, mara-vt2ktq).

- [ ] The configuration entries that declare programs and their consumers (DES-RULE-CONSUMERS) are loaded and checked for every kind of consumer: invariants, queries, views and gates. This issue evaluates invariants; the other kinds are evaluated by the query-operations issue
- [ ] Only programs that the parser admits run, on the parsed copies in a private directory (DES-PROGRAM-PROFILE)
- [ ] Generated wrapper with JSON `.input` and `.output` directives: every fact relation and supplied input, and every relation that a configuration entry names in the program; Soufflé runs in the private directory with `-l ""` (DES-RULE-EXECUTION)
- [ ] Child-process evaluation keyed by the layers read, the program with its includes, and the supplied inputs; every consumer of a program reads the same run. Expose it so that the query operations and explanations can call it
- [ ] Invariants in validation: `project validate` and `item validate` report their rows as diagnostics of the declared severity
- [ ] Time limit (REQ-EVALUATION-TIME-LIMIT)
- [ ] Limits on the output size and the memory of an evaluation: exceeding one ends the evaluation and is reported as a fault, never a passing outcome (REQ-PROGRAM-FAULTS-REPORTED). The worker chooses the numbers and the mechanism, records them in DES-RULE-EXECUTION, and removes the limits part of the open question on docs/rules-facts.mara.md
- [ ] Compile errors, refused programs, timeouts, runtime faults and anything Soufflé prints on stderr while evaluating are reported; never a passing outcome
- [ ] Soufflé names an included file in a diagnostic only by its base name (`in file b.dl` for `sub/b.dl`, observed 2026-11-16): map each diagnostic to its project path also when two included files share a base name, record how in DES-RULE-EXECUTION, and test it with VER-PROGRAM-FAULTS's included-program case
- [ ] A standing test compiles every probe fixture whose `parser` is `admitted` and every program in `crates/mara-engine/tests/programs/trial/` with the pinned `souffle`: a run against empty facts, with a wrapper that reads and writes the fixture's boundary relations as JSON, exits 0 with no compile diagnostic (VER-ADMITTED-LANGUAGE)
- [ ] Tests, natively: the invariant cases of VER-PROGRAM-FAULTS, VER-EVALUATION-TIME-LIMIT and VER-PROGRAM-REACH-CASES (docs/verification-programs.mara.md) and of VER-PROGRAM-CONSUMERS (docs/verification-rules.mara.md) pass; their query, gate and `query report` cases belong to the two issues this one blocks, and their browser rows wait for mara-xsm8w6
- [ ] The alpha.3 cold rows of VER-WORK-COUNTERS for validation with programs pass, and VER-SCALING covers it (DES-PERFORMANCE-HARNESS)

Decided by the orchestrator on 2026-11-16 (from mara-t2qrcb, under the maintainer's rule of that day; listed in mara-46j65e), and confirmed for the sandbox by the maintainer: an evaluation gets limits on output size and memory besides time. A finite program wrote 8 MB in 15 ms in a review probe. Rests on REQ-PROGRAMS-ONLY-COMPUTE, RISK-SOUFFLE-REACHES-OUTSIDE and REQ-EVALUATION-TIME-LIMIT. The sandbox half of that question is mara-ptq6xc.
