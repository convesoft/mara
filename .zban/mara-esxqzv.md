---
title: Move and rename across files
status: todo
type: feature
parent: mara-6k50w0
blocked_by:
  - mara-6ex559
created_at: 2026-11-13T22:10:06Z
---

Context: REQ-MOVE-ITEM and REQ-RENAME-ID in docs/authoring.mara.md; REQ-INTERRUPTED-EDIT-LOSES-NOTHING, DES-WRITE-PROCEDURE and ADR-NO-MUTATION-JOURNAL.

- [x] Decide how the duplicate left by an interrupted move is resolved: rerunning the move (DES-WRITE-PROCEDURE)
- [ ] `item move` and `item rename` in the documented write order
- [ ] Test: interrupting between files never loses an item and leaves only reported states; rerunning an interrupted move of an item with relative links completes it
