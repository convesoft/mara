---
title: Basic-level comment markers
status: in-progress
type: feature
parent: mara-779tms
created_at: 2026-11-13T22:10:06Z
---

Goal: `@mara` comment markers in code files link code to items at file-and-line level: Mara reads them, reports their problems, shows them as relations, and keeps them right when an ID is renamed.

Split by the orchestrator on 2026-11-16, so that this part can be built while alpha.3 and alpha.4 are still in work. Two issues take what depends on those slices: code links as input for programs with the template's code cases (see "Blocks"), and code links through the daemon.

Context: docs/code.mara.md (REQ-CODE-MARKER-LINKS, DES-CODE-MARKERS, DES-CODE-READERS); code keys in DES-INPUTS-AND-KEYS (docs/engine.mara.md); the Verification items in docs/verification-code.mara.md and docs/verification-code-use.mara.md (mara-ttvp9z).

- [x] Decide the open questions on docs/code.mara.md (DES-CODE-MARKERS, DES-CODE-READERS)
- [ ] The configuration syntax of code languages, in DES-PROJECT-CONFIGURATION and in the generated corpus configuration
- [ ] Per-file scan keyed by content; code layer; edges in `relation list`: VER-CODE-MARKERS passes at the basic level, through the CLI and `mara mcp`; the code-link case of VER-ONE-HOP passes (mara-d005q9 is closed with this issue if this covers it)
- [ ] `project validate` reports marker problems; `item delete` is refused while a marker names the item (DES-CODE-READERS, REQ-DELETE-ITEM): VER-CODE-MARKERS and the code row of VER-DELETE-ITEM pass
- [ ] `item rename` rewrites the markers that name the old ID, before the `ID` row (REQ-RENAME-ID, DES-WRITE-PROCEDURE): VER-CODE-EDITS at the basic level and the code rows of VER-RENAME-ID and VER-INTERRUPTED-EDITS pass
- [ ] The cold rows of VER-CODE-WORK-COUNTERS and the code rows of VER-WORK-COUNTERS that need neither programs nor the daemon pass, with their scaling cases
- [ ] The agent guide and the name comparison stay right for whatever this adds to the CLI
