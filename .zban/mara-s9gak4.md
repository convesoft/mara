---
title: Grammar-level endpoints
status: in-progress
type: feature
parent: mara-779tms
blocked_by:
  - mara-yhxq5x
created_at: 2026-11-13T22:10:06Z
---

Context: DES-CODE-MARKERS in docs/code.mara.md.

- [ ] Tree-sitter WASM grammars with `tags.scm`; endpoint is file and owning declaration
- [ ] ABI check on load; fallback to the basic level with a diagnostic

- [ ] VER-GRAMMAR-MARKERS passes, and the grammar-level cases of VER-CODE-FACTS
- [ ] VER-CODE-EDITS passes with the grammars installed (markers before declarations, marker-like lines in raw strings), and the grammar-level `get` row of VER-CODE-WORK-COUNTERS (1 code scan); mara-yhxq5x ran both at the basic level only
