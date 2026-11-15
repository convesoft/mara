---
title: Move and rename across files
status: in-progress
type: feature
parent: mara-6k50w0
blocked_by:
  - mara-6ex559
created_at: 2026-11-13T22:10:06Z
---

Context: REQ-MOVE-ITEM and REQ-RENAME-ID in docs/authoring.mara.md; REQ-INTERRUPTED-EDIT-LOSES-NOTHING, DES-WRITE-PROCEDURE and ADR-NO-MUTATION-JOURNAL.

- [x] Decide how the duplicate left by an interrupted move is resolved: rerunning the move (DES-WRITE-PROCEDURE)
- [ ] `item move` and `item rename` in the documented write order
- [ ] VER-MOVE-ITEM and VER-RENAME-ID (docs/verification-authoring.mara.md) and VER-INTERRUPTED-EDITS pass, with its four ways of stopping and its rename cases: code markers and mention text before the `ID` row, inbound relation link text after it (DES-WRITE-PROCEDURE, ADR-MENTIONS-BEFORE-ID-ROW, decided by the maintainer); for these operations, so do the other cases of docs/verification-writes.mara.md and the alpha.2 rows of VER-MID-RESOLUTION, VER-GITHUB-RENDERING and VER-NESTED-PROJECTS. The code-marker cases wait for mara-yhxq5x (alpha.5)
- [ ] Work-counter and scaling tests for `item move` and `item rename`, the hub cases included (VER-WORK-COUNTERS, VER-SCALING, DES-PERFORMANCE-HARNESS)
