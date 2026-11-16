---
title: Basic-level comment markers
status: todo
type: feature
parent: mara-779tms
created_at: 2026-11-13T22:10:06Z
---

Context: docs/code.mara.md (REQ-CODE-MARKER-LINKS, DES-CODE-MARKERS); code keys in DES-INPUTS-AND-KEYS; the code input relation in DES-FACT-BRIDGE.

- [x] Decide the open questions on docs/code.mara.md (DES-CODE-MARKERS, DES-CODE-READERS)
- [ ] `project validate` reports marker problems; `item delete` is refused while a marker names the item (DES-CODE-READERS, REQ-DELETE-ITEM): VER-CODE-MARKERS and the code row of VER-DELETE-ITEM pass
- [ ] Per-file scan keyed by content; code layer; edges in `relation list`: VER-CODE-MARKERS passes at the basic level, through the CLI, `mara mcp` and the daemon
- [ ] `item rename` rewrites the markers that name the old ID, before the `ID` row (REQ-RENAME-ID, DES-WRITE-PROCEDURE): VER-CODE-EDITS and the code rows of VER-RENAME-ID and VER-INTERRUPTED-EDITS pass
- [ ] Code links as an input relation for programs, and `source_complete("code")` (DES-CODE-READERS): VER-CODE-FACTS passes at the basic level
- [ ] The code-dependent cases of VER-TEMPLATE-CASES pass: `implemented`, `verified`, `release-ready`
- [ ] The configuration syntax of code languages, in DES-PROJECT-CONFIGURATION and in the generated corpus configuration; VER-CODE-WORK-COUNTERS and the code rows of VER-WORK-COUNTERS pass
