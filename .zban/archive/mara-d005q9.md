---
title: Run the code-link case of VER-ONE-HOP
status: done
type: task
parent: mara-779tms
blocked_by:
  - mara-yhxq5x
related:
  - mara-bmc9gc
discovered_from: mara-km8qqj
created_at: 2026-11-14T13:26:53Z
closed_at: 2026-11-16T18:04:44Z
---

Goal: `relation list` returns code links, as REQ-DIRECT-NEIGHBOURS (docs/retrieval.mara.md) requires.

Context: VER-ONE-HOP (docs/verification-retrieval.mara.md) expects the code link of item A in `relation list`. Code links arrive in this slice, so mara-bmc9gc passes VER-ONE-HOP without that case.

- [x] VER-ONE-HOP passes with its code-link case

## Summary of Changes

Done by mara-yhxq5x: `crates/mara/tests/reads/relations.rs` gives item A a code link, which `relation list` lists after its incoming links, and lists the code location's one connection. `cargo test --workspace` passes.
