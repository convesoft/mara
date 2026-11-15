---
title: Move rewrites the item's relative links to files that are no documents
status: todo
type: feature
priority: deferred
tags:
  - deferred
  - backlog
discovered_from: mara-esxqzv
created_at: 2026-11-15T15:30:08Z
---

Goal: a moved item's relative links to files that are no documents, and its images, still lead to the same file from the destination.

Context: DES-MOVE-AND-RENAME in docs/authoring.mara.md rewrites only the item's relation links and mentions (links whose fragment is a MID); its other relative links and images keep their bytes, so a move to another directory breaks them. The limit is stated in that item. `mara_engine::relink::moved_text` builds the moved text; `relink::links::inline_path` finds a destination only by its `#<MID>` fragment.

- [ ] Rewrite the path of every relative link and image destination in the moved item, inline or through a definition inside it, so it names the same file from the destination
- [ ] A test of `item move` to another directory with a link to a plain file and an image
- [ ] Remove the limit sentence from DES-MOVE-AND-RENAME
