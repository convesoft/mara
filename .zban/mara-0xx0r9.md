---
title: The sandbox bounds what it reads from an ELF header; search and word indexing allocate less
status: todo
type: task
parent: mara-qzp1gd
created_at: 2026-11-20T20:22:51Z
---

Goal: finding the Soufflé binary's interpreter cannot ask for gigabytes of memory, and two small engine defects are gone.

Context: Found by the maintainer's code audit of 2026-11-20 (at `a5f8ec0`, then `a5f8ec0..c6a9706`), filed at the maintainer's request. Locations were checked at `c6a9706`. Items: DES-SOUFFLE-SANDBOX (docs/rules/evaluation.mara.md).

- [ ] `crates/mara/src/rules/sandbox.rs:302`, `interpreter`: `vec![0u8; size * count]` takes both factors from 16-bit fields of the ELF header with no upper bound (up to about 4.29 GiB for a damaged file), while the `PT_INTERP` length is already limited to 4096. Bound the product, with a checked multiplication, before allocating; a unit test gives it a header with the largest values
- [ ] `crates/mara-engine/src/read/search.rs:325-372`: `found: HashMap<Node, Vec<WordMatch>>` allocates a `Vec` for every candidate node, although what is computed from it (`exact`, `exact_high`, `high`, `exact_at`, `near_at`) is a fold that fits a small `Copy` struct. Keep the fold, not the list
- [ ] The same function, an empty query (`mara search "" --type ...`): every item and narrative unit of the corpus is put into that map before `matcher.matches` is applied. Stream the nodes that match
- [ ] `crates/mara-engine/src/read/search.rs:319`: a query of several words intersects its posting lists in alphabetical order (`texts.sort_unstable()`). Start with the rarest word, the shortest list
- [ ] `crates/mara-engine/src/document/region.rs`, `align`: it runs for every text event and every one-line code span and allocates two vectors also in the common case where the slice equals the rendered text and is no code. Give that case a path without allocation (one run on the stack, a vector only for several)
- [ ] `crates/mara-engine/src/document/region.rs:1095-1104`, `add_words`: `word.to_lowercase()` allocates for every occurrence of every word, and a new word is looked up twice (`get_mut`, then `entry`). Lowercase without allocating when the word already is lowercase, and look up once

Done when the allocation is bounded with its test, the engine items are done with search results, their order and every other result unchanged, and the local checks pass. The engine items are efficiency work under VER-SCALING (docs/verification.mara.md): work counts that count parses and reads stay as they are.
