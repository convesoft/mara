---
title: Parse Mara documents
status: todo
type: feature
parent: mara-t1rffa
blocked_by:
  - mara-0ztabc
  - mara-csmk0c
created_at: 2026-11-13T22:10:06Z
---

Goal: one pass per file turns a `*.mara.md` document into items, metadata, links and diagnostics.

Context: docs/format.mara.md (DES-REGIONS, DES-ITEM-STRUCTURE, DES-METADATA-TABLE); docs/identity.mara.md (DES-MID-ANCHOR, DES-ITEM-IDS, DES-RELATION-LINKS); DES-SNAPSHOT in docs/engine.mara.md for the single `pulldown-cmark` pass. Verified end to end, through `project validate` (mara-000f9q), by VER-GITHUB-RENDERING and VER-SEEDED-PROBLEMS in docs/verification-documents.mara.md.

- [x] Decide whether YAML front matter is accepted: skipped at the start of a file (DES-REGIONS)
- [ ] Regions, items and incomplete items
- [ ] Metadata table with label matching
- [ ] Relation links and mentions
- [ ] Diagnostics for misplaced anchors, setext headings and malformed rows
- [ ] The documents in `docs/` parse without diagnostics
