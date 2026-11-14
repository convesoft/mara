---
title: "Read operations: search, get, related, list, schema inspection"
status: todo
type: feature
parent: mara-t1rffa
blocked_by:
  - mara-8d6cnw
created_at: 2026-11-13T22:10:06Z
---

Goal: bounded reads over the snapshot from the CLI.

Context: docs/retrieval.mara.md (all requirements, DES-NARRATIVE-UNITS, DES-PAGING); REQ-SCHEMA-INSPECTION in docs/project.mara.md.

- [ ] Decide the open questions on docs/retrieval.mara.md: where relation occurrences are inspected, and how `get` pages a large node
- [ ] `search` with filters, ranking and typo tolerance
- [ ] `get`, `related`, `item list`, `schema get` and `schema list`
- [ ] Paging with cursors that carry the snapshot key
- [ ] Scaling test covers each operation
