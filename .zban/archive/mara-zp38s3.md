---
title: Explanations of the engineering template's invariant and gate rows
status: done
type: task
parent: mara-17tdb3
blocked_by:
  - mara-jm5ecb
discovered_from: mara-tx0fcf
created_at: 2026-11-16T21:06:53Z
closed_at: 2026-11-20T15:37:52Z
---

Goal: the VER-TEMPLATE-CASES row "A failing row of each template invariant and gate → Has an explanation" (docs/template/index.mara.md) passes.

Context: mara-tx0fcf shipped the `engineering` programs (`.mara/rules/knowledge.dl`, `readiness.dl`, `risk-matrix.dl`) and tests every other case of VER-TEMPLATE-CASES that needs no code links in `crates/mara/tests/template/`. `query report` is mara-jm5ecb, not yet integrated when mara-tx0fcf closed, so this case had nothing to call.

- [x] A test in `crates/mara/tests/template/` that makes each of the 13 template invariants and each gate condition fail (the fixtures of `invariants.rs` and `readiness.rs` already do) and runs `query report` on one failing row of each: an explanation, never "cannot explain", whose leaves are document locations or supplied inputs
- [x] If a template program uses a construct whose rows `query report` cannot explain (an aggregate branch, for example), rewrite the rule in the admitted language so it can (DES-TEMPLATE-QUERIES: the programs use only constructs Soufflé can explain)

Note for the `query report` scaling case (DES-PERFORMANCE-HARNESS): since mara-tx0fcf the corpus declares the template's programs, and its `query gate verified` case is `unavailable` (exit 1) with no failing row, because no code link reaches programs before mara-hk2arx.

## Summary of Changes

Closed by the orchestrator on 2026-11-20 as a tracker change, with no branch of its own: mara-jm5ecb (`f1ee4b4`) did the work when it rebased over mara-tx0fcf.

- `crates/mara/tests/template/invariants.rs` (`check`) runs `query report` on every failing row of the template's 13 invariants in its cases (24 rows) and requires outcome `failed` and a proof for each row.
- `crates/mara/tests/template/main.rs` (`Project::gate`) runs `query report` on every item a gate case judges failed or unavailable (51 items over `implementation-ready`, `verified` and `release-ready`) and requires the gate's outcome, the same number of rows, and a proof for each row.
- No template rule was rewritten. Three invariants (`links-to-accepted`, `supersedes-retired`, `risk-assessed`) had rows that came back unexplained because `relation_class` and `risk_dimension` are defined only by facts; mara-jm5ecb fixed the reader instead: such a leaf is shown as a fact at the program file and line that states it (DES-RULE-EXECUTION, DES-QUERY-RESULTS).
- Not asserted per template row: that every leaf is a document location or a supplied input. Leaf kinds are tested in mara-jm5ecb's own tests (`crates/mara/tests/programs/`), and since its fix a leaf may also be a fact of the program.
- Checks: the orchestrator's gate at `6979f4d` (the seven local checks, all exit 0), which runs these tests.

## Decisions made

- Orchestrator: this issue is met by mara-jm5ecb's tests and closed without a worker; the leaf-kind clause of its first checklist item is covered by mara-jm5ecb's tests of `query report`, not repeated for each template row.
