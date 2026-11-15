---
title: "VER-RENAME-ID: relation add given the old ID fails"
status: done
type: task
parent: mara-6k50w0
blocked_by:
  - mara-w0xasr
  - mara-esxqzv
related:
  - mara-w0xasr
discovered_from: mara-esxqzv
created_at: 2026-11-15T15:30:08Z
closed_at: 2026-11-15T15:42:18Z
---

Goal: the VER-RENAME-ID case "`get`, `item validate` and `relation add` given the old ID afterwards fail with `unknown-item`" is tested for all three operations.

Context: mara-esxqzv tests `get` and `item validate` (`crates/mara/tests/relink/renaming.rs`, `renames_an_id_wherever_it_is_a_handle`). `relation add` comes with mara-w0xasr, so its assertion could not be written there.

- [x] Once both are integrated, add `relation add` with the old ID as source and as target to that test, expecting `unknown-item` and no file changed

## Summary of Changes

`renames_an_id_wherever_it_is_a_handle` (`crates/mara/tests/relink/renaming.rs`) now also runs `relation add` with the old ID as source and as target after the rename. Each fails with `unknown-item`, and every file keeps its name, bytes and mtime; `get` and `item validate` are checked against the same unchanged tree. Done in the branch of mara-w0xasr, rebased onto `9c8d2fc`.

Checks: the seven local checks of AGENTS.md, all exit 0 (see mara-w0xasr).
