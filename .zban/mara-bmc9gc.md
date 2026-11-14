---
title: "Read operations: search, get, relation list, item list, schema inspection"
status: in-progress
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
- [ ] VER-SCALING covers each operation, including its cold parse of every document: the parser has no case of its own (DES-PERFORMANCE-HARNESS, mara-t6xn9z). Narrative units and search text come from the parser's one pass (DES-SNAPSHOT): `document::parse` returns items, rows and links only, so extend it rather than parse again
- [ ] The cold rows of VER-WORK-COUNTERS (docs/verification.mara.md) pass through the real CLI for each read operation: a cold request (two documents with the same contents included: 1 parse per distinct content), `.git` and undeclared files, an operation that reads no code (moved here from mara-8d6cnw by the maintainer, mara-63dn2d)
- [ ] VER-SEARCH-CASES, VER-NODE-READS, VER-ITEM-LIST, VER-CURSOR-WALK, VER-PAGE-LIMITS and VER-SCHEMA-INSPECTION pass (docs/verification-retrieval.mara.md)
- [ ] VER-ONE-HOP passes except its code-link case, which waits for code links (mara-d005q9)
- [ ] VER-MID-RESOLUTION and the `get` case of VER-INVALID-ROWS pass, and VER-NESTED-PROJECTS and VER-UNCHANGED-SOURCES pass for the read operations (docs/verification-documents.mara.md)
- [ ] The read operations select and load the project with `mara::project::select` and `load` (crates/mara/src/project.rs, mara-b52dg3), which only host tests have exercised. Real-CLI tests cover selection from inside a nested project and with `--project`, `no-project`, `not-a-project`, `legacy-project` and `symbolic-link` (codes in DES-PROJECT-CONFIGURATION); each read operation refuses a project whose configuration or schema has an error and reports those diagnostics (mara-zv9d69); `schema list` and `schema get` return every declaration of DES-SCHEMA-MODEL as loaded: per-source targets, the `code` source, `same_type`, `acyclic`, `required` and `repeatable`
