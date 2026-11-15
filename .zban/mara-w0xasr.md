---
title: "Single-file item operations: create, update, delete, relations"
status: todo
type: feature
parent: mara-6k50w0
blocked_by:
  - mara-6ex559
created_at: 2026-11-13T22:10:06Z
---

Context: docs/authoring.mara.md (REQ-CREATE-ITEM, REQ-UPDATE-ITEM, REQ-DELETE-ITEM, REQ-EDIT-RELATIONS).

- [x] Decide whether adding a relation accepts the inverse name: refused (REQ-EDIT-RELATIONS)
- [ ] `item create` with ID proposal, `item update`, `item delete`
- [ ] `relation add` and `relation remove`
- [ ] VER-CREATE-ITEM, VER-UPDATE-ITEM, VER-DELETE-ITEM and VER-EDIT-RELATIONS pass (docs/verification-authoring.mara.md); for these operations, so do the cases of docs/verification-writes.mara.md, the CLI cases of VER-CONCURRENT-WRITERS, and the alpha.2 rows of VER-GITHUB-RENDERING, VER-MID-RESOLUTION (`item update`) and VER-NESTED-PROJECTS (docs/verification-documents.mara.md)
- [ ] Work-counter and scaling tests for each operation: the alpha.2 cold rows of VER-WORK-COUNTERS and VER-SCALING (docs/verification.mara.md, DES-PERFORMANCE-HARNESS)
