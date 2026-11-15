---
title: Move carries the definitions after an item's last block with it
status: todo
type: feature
priority: deferred
tags:
  - deferred
  - backlog
related:
  - mara-jnvzxr
discovered_from: mara-esxqzv
created_at: 2026-11-15T15:30:08Z
---

Goal: an item whose links use link reference definitions written after its last block, inside its region, can be moved; today the move is refused.

Context: the parser's `Item::span` ends at the item's last block, so definitions after it lie outside the item. mara-jnvzxr chose the simplest safe option: `item move` refuses with `unreadable-edit` when a reference of the moved item uses a definition outside it (DES-MOVE-AND-RENAME). Carrying the trailing definitions needs the moved text and the removal to cover them, and the read-back guard's removal check (`read_back.rs`, `removals`) to accept definitions after the removed item.

- [ ] Move the definitions between the item's last block and the next separator together with the item, and remove them from the source
- [ ] Keep the refusal for definitions elsewhere, such as at the end of the page
- [ ] A test of `item move` for an item that ends with its definitions, to another document and within its own
