---
title: Program bytes are shared and hashed once, and a relation's JSON is built when it is needed
status: todo
type: task
parent: mara-qzp1gd
created_at: 2026-11-20T20:22:51Z
---

Goal: a request neither copies nor hashes a rule program's contents again, and a relation does not hold its rows twice.

Context: Found by the maintainer's code audit of 2026-11-20 (at `a5f8ec0`, then `a5f8ec0..c6a9706`), filed at the maintainer's request. Locations were checked at `c6a9706`. Items: DES-INPUTS-AND-KEYS (docs/engine.mara.md), DES-FACT-BRIDGE (docs/rules/index.mara.md), DES-PERFORMANCE-HARNESS (docs/performance.mara.md).

- [ ] `crates/mara/src/inputs.rs`: `State.programs` holds `Arc<Vec<u8>>`, and `Generation.programs` clones the bytes into a `BTreeMap<String, Vec<u8>>` for every request. Share the `Arc`
- [ ] `crates/mara/src/cli/validate.rs:74-75`, `input_keys`: every program file's contents are hashed again with `Hash::of_bytes`, although the inputs already track each program file's hash. Use the tracked hash (the work-count rows for hashes change accordingly; update DES-PERFORMANCE-HARNESS and the tests)
- [ ] `crates/mara-engine/src/facts.rs:76`: `Relation` stores its `rows` and, eagerly, the same rows as `json: String`, also for a relation that never goes to Soufflé. Build the JSON on demand, with the engine still free of I/O and building for `wasm32`

Done when the three are done and the local checks pass.
