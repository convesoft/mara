---
title: Move keeps links that use a link reference definition outside the moved item
status: todo
type: task
parent: mara-6k50w0
blocked_by:
  - mara-esxqzv
related:
  - mara-esxqzv
discovered_from: mara-6ex559
created_at: 2026-11-15T12:17:06Z
---

Goal: `item move` never silently drops a relation or mention of the moved item.

Context: the writer's read-back guard (DES-ITEM-WRITER, ADR-READ-BACK-EVERY-EDIT, `mara_engine::write::ReadBack`) checks that an inserted item reads as its own text: each of its links that a link reference definition decides uses a definition inside the item. A row or mention of the moved item can use a definition elsewhere in the source document, such as `| Refines | [REQ-C][r] |` with `[r]: #<MID>` in narrative. In a destination that does not define `[r]`, that text reads as plain text: the guard sees no link and accepts it, and the relation is lost. VER-MOVE-ITEM requires the move to keep content and rewrite link paths.

- [ ] Compare the moved item's links in the destination with its links in the source parse, the rewritten paths aside, and refuse the move with a diagnostic naming the link and the definition, or carry the definition with the item
- [ ] A test of `item move` for an item whose relation and mention use a definition outside it
