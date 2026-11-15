---
title: Move keeps links that use a link reference definition outside the moved item
status: done
type: task
parent: mara-6k50w0
blocked_by:
  - mara-esxqzv
related:
  - mara-esxqzv
discovered_from: mara-6ex559
created_at: 2026-11-15T12:17:06Z
closed_at: 2026-11-15T15:31:10Z
---

Goal: `item move` never silently drops a relation or mention of the moved item.

Context: the writer's read-back guard (DES-ITEM-WRITER, ADR-READ-BACK-EVERY-EDIT, `mara_engine::write::ReadBack`) checks that an inserted item reads as its own text: each of its links that a link reference definition decides uses a definition inside the item. A row or mention of the moved item can use a definition elsewhere in the source document, such as `| Refines | [REQ-C][r] |` with `[r]: #<MID>` in narrative. In a destination that does not define `[r]`, that text reads as plain text: the guard sees no link and accepts it, and the relation is lost. VER-MOVE-ITEM requires the move to keep content and rewrite link paths.

- [x] Compare the moved item's links in the destination with its links in the source parse, the rewritten paths aside, and refuse the move with a diagnostic naming the link and the definition, or carry the definition with the item
- [x] A test of `item move` for an item whose relation and mention use a definition outside it

## Summary of Changes

Done in mara-esxqzv's branch. `mara_engine::relink::move_item` refuses a move with `unreadable-edit` when a link, image or footnote reference of the moved item uses a link reference or footnote definition outside the item, after its last block or at the end of the page; the diagnostic is at the reference and the message names it and the definition's line. Nothing is written. Recorded in DES-MOVE-AND-RENAME.

Tests: `relink::tests::refuses_to_move_an_item_whose_links_use_a_definition_outside_it` (engine, both places, to another document and within its own) and `moving::refuses_a_move_that_would_leave_a_definition_behind` (CLI, a relation row and a mention using a definition at the end of the page; files byte-identical).

## Decisions made

- The simplest option the issue allows: refuse, never carry. Carrying the definitions after an item's last block is backlog mara-h8bc9x.
- Every reference kind is checked, not only relation rows and mentions: an ordinary link or footnote would read as plain text in the destination just as silently.
