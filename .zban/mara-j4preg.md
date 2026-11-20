---
title: Gates, queries and explanations do no quadratic work and serialize a proof once
status: todo
type: task
parent: mara-qzp1gd
created_at: 2026-11-20T20:22:51Z
---

Goal: the engine's evaluation and read paths named below are linear in their input, and a proof is written once.

Context: Found by the maintainer's code audit of 2026-11-20 (at `a5f8ec0`, then `a5f8ec0..c6a9706`), filed at the maintainer's request. Locations were checked at `c6a9706`. Items: DES-RULE-EXECUTION (docs/rules/evaluation.mara.md), DES-QUERY-RESULTS (docs/rules/index.mara.md), VER-SCALING (docs/verification.mara.md), DES-PERFORMANCE-HARNESS (docs/performance.mara.md).

- [ ] `crates/mara-engine/src/evaluation/gate.rs`, `judge`: remove the quadratic pass
- [ ] `crates/mara-engine/src/read/query.rs:107`: `mids.contains(&mid)` on a `Vec` for every row; use a set
- [ ] `crates/mara-engine/src/evaluation/explain.rs`, `Leaves::location`: it searches `relation.rows` for the row whose values it was just handed; pass the row's values and do not search
- [ ] The same file, `folded` leaves: index the `title` and `field` rows once in place of scanning both tables for every leaf
- [ ] The same file, `Node::write_json`: write into the output directly, without a `String` per child at every level, and do not serialize a proof twice (once for the size limit, once for the output)
- [ ] `crates/mara/src/rules/explain.rs`: the reply buffer is parsed again from its first byte whenever a chunk ends with a closing brace; keep the position

Done when the items are done, results and work counts are unchanged, and a scaling test covers `query gate` and `query report` on rows and proofs large enough to show quadratic growth.
