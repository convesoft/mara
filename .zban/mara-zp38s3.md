---
title: Explanations of the engineering template's invariant and gate rows
status: todo
type: task
parent: mara-17tdb3
blocked_by:
  - mara-jm5ecb
discovered_from: mara-tx0fcf
created_at: 2026-11-16T21:06:53Z
---

Goal: the VER-TEMPLATE-CASES row "A failing row of each template invariant and gate → Has an explanation" (docs/template/index.mara.md) passes.

Context: mara-tx0fcf shipped the `engineering` programs (`.mara/rules/knowledge.dl`, `readiness.dl`, `risk-matrix.dl`) and tests every other case of VER-TEMPLATE-CASES that needs no code links in `crates/mara/tests/template/`. `query report` is mara-jm5ecb, not yet integrated when mara-tx0fcf closed, so this case had nothing to call.

- [ ] A test in `crates/mara/tests/template/` that makes each of the 13 template invariants and each gate condition fail (the fixtures of `invariants.rs` and `readiness.rs` already do) and runs `query report` on one failing row of each: an explanation, never "cannot explain", whose leaves are document locations or supplied inputs
- [ ] If a template program uses a construct whose rows `query report` cannot explain (an aggregate branch, for example), rewrite the rule in the admitted language so it can (DES-TEMPLATE-QUERIES: the programs use only constructs Soufflé can explain)

Note for the `query report` scaling case (DES-PERFORMANCE-HARNESS): since mara-tx0fcf the corpus declares the template's programs, and its `query gate verified` case is `unavailable` (exit 1) with no failing row, because no code link reaches programs before mara-hk2arx.
