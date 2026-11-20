---
title: The sandbox bounds what it reads from an ELF header; two engine nits
status: todo
type: task
parent: mara-qzp1gd
created_at: 2026-11-20T20:22:51Z
---

Goal: finding the Soufflé binary's interpreter cannot ask for gigabytes of memory, and two small engine defects are gone.

Context: Found by the maintainer's code audit of 2026-11-20 (at `a5f8ec0`, then `a5f8ec0..c6a9706`), filed at the maintainer's request. Locations were checked at `c6a9706`. Items: DES-SOUFFLE-SANDBOX (docs/rules/evaluation.mara.md).

- [ ] `crates/mara/src/rules/sandbox.rs:302`, `interpreter`: `vec![0u8; size * count]` takes both factors from 16-bit fields of the ELF header with no upper bound (up to about 4.29 GiB for a damaged file), while the `PT_INTERP` length is already limited to 4096. Bound the product, with a checked multiplication, before allocating; a unit test gives it a header with the largest values
- [ ] `crates/mara-engine/src/read/search.rs` and `crates/mara-engine/src/document/region.rs`: the audit noted minor edge-case and efficiency defects without naming them here; whoever starts this issue gets the list from the audit, or drops this item if there is none

Done when the allocation is bounded with its test, the engine items are done or dropped with a note, and the local checks pass.
