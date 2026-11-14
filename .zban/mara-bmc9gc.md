---
title: "Read operations: search, get, relation list, item list, schema inspection"
status: todo
type: feature
parent: mara-t1rffa
blocked_by:
  - mara-8d6cnw
created_at: 2026-11-13T22:10:06Z
---

Goal: bounded reads over the snapshot from the CLI.

Context: docs/retrieval.mara.md (all requirements, DES-NARRATIVE-UNITS, DES-PAGING); REQ-SCHEMA-INSPECTION in docs/project.mara.md.

- [x] Decide how `get` pages a large node: consecutive portions (DES-PAGING)
- [ ] `search` with filters, ranking and typo tolerance
- [ ] `get`, `relation list`, `item list`, `schema get` and `schema list`
- [ ] Paging with cursors that carry the snapshot key
- [ ] VER-SCALING covers each operation
- [ ] VER-SEARCH-CASES, VER-NODE-READS, VER-ITEM-LIST, VER-CURSOR-WALK, VER-PAGE-LIMITS and VER-SCHEMA-INSPECTION pass (docs/verification-retrieval.mara.md)
- [ ] VER-ONE-HOP passes except its code-link case, which waits for code links (mara-d005q9)
- [ ] VER-MID-RESOLUTION and the `get` case of VER-INVALID-ROWS pass, and VER-NESTED-PROJECTS and VER-UNCHANGED-SOURCES pass for the read operations (docs/verification-documents.mara.md)
