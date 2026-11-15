---
title: "VER-RENAME-ID: relation add given the old ID fails"
status: todo
type: task
parent: mara-6k50w0
blocked_by:
  - mara-w0xasr
  - mara-esxqzv
related:
  - mara-w0xasr
discovered_from: mara-esxqzv
created_at: 2026-11-15T15:30:08Z
---

Goal: the VER-RENAME-ID case "`get`, `item validate` and `relation add` given the old ID afterwards fail with `unknown-item`" is tested for all three operations.

Context: mara-esxqzv tests `get` and `item validate` (`crates/mara/tests/relink/renaming.rs`, `renames_an_id_wherever_it_is_a_handle`). `relation add` comes with mara-w0xasr, so its assertion could not be written there.

- [ ] Once both are integrated, add `relation add` with the old ID as source and as target to that test, expecting `unknown-item` and no file changed
