---
title: Say in catalog.rs that the shipped catalog holds rust and python
status: todo
type: task
priority: deferred
tags:
  - deferred
  - backlog
blocked_by:
  - mara-7kfxn1
discovered_from: mara-yk8drm
created_at: 2026-11-21T17:36:01Z
---

Goal: the module comment of `crates/mara/src/cli/code/catalog.rs` describes the shipped catalog as it is.

Context: mara-yk8drm filled `crates/mara/src/cli/code/catalog.toml` with rust 0.24.2 and python 0.25.0 (DES-GRAMMAR-ASSETS in docs/code/index.mara.md). Lines 5–6 of `catalog.rs` still say the shipped catalog is empty until its entries are decided, citing an open question that no longer exists. mara-7kfxn1 owned `catalog.rs` while mara-yk8drm ran, so the comment was left alone.

- [ ] Replace the sentence with one that names what the catalog holds, or link to DES-GRAMMAR-ASSETS instead of restating it
