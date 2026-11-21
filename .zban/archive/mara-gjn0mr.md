---
title: Say in catalog.rs that the shipped catalog holds rust and python
status: done
type: task
priority: deferred
tags:
  - deferred
  - backlog
blocked_by:
  - mara-7kfxn1
discovered_from: mara-yk8drm
created_at: 2026-11-21T17:36:01Z
closed_at: 2026-11-21T17:54:11Z
---

Goal: the module comment of `crates/mara/src/cli/code/catalog.rs` describes the shipped catalog as it is.

Context: mara-yk8drm filled `crates/mara/src/cli/code/catalog.toml` with rust 0.24.2 and python 0.25.0 (DES-GRAMMAR-ASSETS in docs/code/index.mara.md). Lines 5–6 of `catalog.rs` still say the shipped catalog is empty until its entries are decided, citing an open question that no longer exists. mara-7kfxn1 owned `catalog.rs` while mara-yk8drm ran, so the comment was left alone.

- [x] Replace the sentence with one that names what the catalog holds, or link to DES-GRAMMAR-ASSETS instead of restating it

## Summary of Changes

Done in the branch of mara-7kfxn1, which owned `catalog.rs`. The module comment of `crates/mara/src/cli/code/catalog.rs` now says the shipped catalog is `catalog.toml`, whose languages DES-GRAMMAR-ASSETS lists, and no longer restates them or cites the removed open question. Checks: the local checks of AGENTS.md, run for mara-7kfxn1 on the merged head.
