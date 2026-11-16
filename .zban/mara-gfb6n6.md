---
title: "Query operations: execute, view, gate and list in CLI and MCP"
status: in-progress
type: feature
parent: mara-17tdb3
blocked_by:
  - mara-bx82k0
created_at: 2026-11-16T16:11:48Z
---

Goal: a user or agent runs a project's named queries, views and gates, with supplied inputs, from the CLI and through MCP.

Split by the orchestrator on 2026-11-16 from mara-bx82k0, which builds the execution this issue calls (admitted programs in a private directory, the generated wrapper, the keyed child-process evaluation, limits and fault reporting, invariants in validation, and the configuration entries of every consumer kind). Explanations (`query report`) are a third issue.

Context: docs/rules.mara.md (REQ-PROJECT-AUTHORED-LOGIC, REQ-EXTERNAL-RECORDS-AS-SUPPLIED, DES-RULE-CONSUMERS, DES-GATE-OUTCOME, DES-REQUEST-INPUTS); docs/rules-evaluation.mara.md (REQ-PROGRAM-FAULTS-REPORTED, DES-RULE-EXECUTION); DES-OPERATIONS and REQ-CLI-MCP-PARITY in docs/hosts.mara.md.

- [ ] Named queries, views and gates: `query execute`, `query view`, `query gate` and `query list` in the CLI and as MCP tools
- [ ] Gate outcome per named item from subject, condition and unavailable relations (DES-GATE-OUTCOME)
- [ ] Request inputs on the command line and through MCP: `--input` inline and from a file, `--complete`; a missing, unlisted or wrong input refuses the request before any run (DES-REQUEST-INPUTS). Use `facts::check_inputs`, `rules::supplied_input` and `rules::write_facts` from mara-y4asgm
- [ ] A fault of a program consumed by a query or a gate fails the request or makes the gate and the items it names unavailable; never a passing outcome (REQ-PROGRAM-FAULTS-REPORTED)
- [ ] Tests, natively: the query, view, gate and list cases of VER-PROGRAM-CONSUMERS; VER-FACT-ROWS through a configured query and `query execute`; VER-REQUEST-INPUTS through `--input`, `--complete` and a gate: the per-item outcomes (unavailable, failed), the MCP rows, refusals with 0 program runs in `--debug-stats`, the spawn count, and a later request without inputs refused with the project tree unchanged (docs/verification-rules.mara.md); the query and gate cases of VER-PROGRAM-FAULTS, VER-EVALUATION-TIME-LIMIT and VER-PROGRAM-REACH-CASES (docs/verification-programs.mara.md). Browser rows wait for mara-xsm8w6
- [ ] The `query` rows of VER-CLI-MCP-PARITY pass for these four operations
- [ ] The alpha.3 cold rows of VER-WORK-COUNTERS for `query execute`, `query view`, `query gate` and `query list` pass, and VER-SCALING covers them (DES-PERFORMANCE-HARNESS)
- [ ] The agent guide (`crates/mara/src/cli/guide.md`, DES-AGENT-GUIDE) names the new operations within its size limit, since its tests compare its names with the operations in both directions

What mara-bx82k0 exposes: `mara_engine::config::{Rules, Entry, EntryKind, Consumer}` (every entry kind loaded and checked); `mara_engine::evaluation::plan(rules, Roots, files)` gives each admitted program with its valid entries (`PlannedEntry::inputs` are the `InputDecl`s for `facts::check_inputs`) and the union of their output relations; `mara::rules::{souffle, Run, evaluate, Evaluations}` runs one evaluation keyed by `evaluation::key` and returns `Rows` or a `Fault` (`Fault::diagnostics` places it). `mara::rules::invariants` shows the validation path. The performance corpus already declares a stand-in program with one invariant, query (`verified-requirements`), view (`unverified`) and gate (`verified`, inputs `candidate` and `verification_result(v: Mid, c: symbol, result: symbol)`) in `crates/mara/tests/performance/corpus.rs`. Test helpers: `crates/mara/tests/programs/main.rs` (`Sandbox`).
