---
title: "The Node addon build: nx-rust's plain build or napi-build"
status: todo
type: task
tags:
  - maintainer
  - question
related:
  - mara-9a1zjp
discovered_from: mara-vfkgbs
created_at: 2026-11-25T15:24:51Z
---

Goal: confirm whether `crates/mara-node` should keep nx-rust's plain `build` target (`artifacts = []`, `ADR-NAPI-RS-ADDON`) or switch to nx-rust's `napi-build` target.

Context: `mara-vfkgbs` built the Node addon (`crates/mara-node`) with napi-rs using nx-rust's plain `build` target (`artifacts = []`) instead of nx-rust's `napi-build` target. `napi-build` would pull in `@napi-rs/cli` and generate a JS loader and `.d.ts` declarations that `@convesoft/mara` does not use (it has its own dispatcher lookup and hand-written `operations.d.ts`).

Options:
- [ ] Keep nx-rust's plain `build` with `artifacts = []` (recommended; provisional until answered). Switching later changes only `crates/mara-node/Cargo.toml` and one target dependency in `mara:test`.
- [ ] Switch to nx-rust's `napi-build` target, matching nx-rust's own N-API workflow.
