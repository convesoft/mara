---
title: No line places an item before a document's first item when that item starts at line 1
status: todo
type: bug
priority: deferred
tags:
  - deferred
  - backlog
discovered_from: mara-8vqk4a
created_at: 2026-11-16T11:09:50Z
---

Found while writing the MCP parity tests (mara-8vqk4a); the behaviour is the same through the CLI.

`item create --line <n>` and `item move --line <n>` take a line outside every item (DES-AUTHORING-REQUESTS, DES-MOVE-AND-RENAME). A line that holds an item's anchor counts as inside that item. A document that `item create` makes for a new path starts with the first item's anchor at line 1. So no line places a new or moved item before that first item: `--line 1` is refused with `invalid-position` ("line 1 of docs/a.mara.md is inside NEED-ONE, at lines 1 to 10"). A document that begins with a heading or narrative is not affected: a line there is outside every item.

Who meets it: an author or agent who wants to put an item first in a document that holds only items. Workaround: write a heading or a sentence at the top first.

- [ ] Decide whether the line of a region's first block (anchor, or `---` before it) places an item before that region, and write it into DES-AUTHORING-REQUESTS and DES-MOVE-AND-RENAME
- [ ] If so, accept it in `item create` and `item move`, with cases in VER-CREATE-ITEM and VER-MOVE-ITEM
