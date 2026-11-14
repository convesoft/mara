---
title: Basic-level comment markers
status: todo
type: feature
parent: mara-779tms
created_at: 2026-11-13T22:10:06Z
---

Context: docs/code.mara.md (REQ-CODE-MARKER-LINKS, DES-CODE-MARKERS); code keys in DES-INPUTS-AND-KEYS; the code input relation in DES-FACT-BRIDGE.

- [x] Decide the open questions on docs/code.mara.md (DES-CODE-MARKERS, DES-CODE-READERS)
- [ ] `project validate` reports marker problems; `item delete` is refused while a marker names the item (DES-CODE-READERS, REQ-DELETE-ITEM)
- [ ] Per-file scan keyed by content; code layer; edges in `relation list`
- [ ] `item rename` rewrites the markers that name the old ID, before the `ID` row (REQ-RENAME-ID, DES-WRITE-PROCEDURE)
- [ ] Code links as an input relation for programs
- [ ] The code-dependent cases of VER-TEMPLATE-CASES pass: `implemented`, `verified`, `release-ready`
- [ ] VER-WORK-COUNTERS rows for code scans pass
