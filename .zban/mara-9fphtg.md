---
title: Code links through the daemon
status: todo
type: feature
parent: mara-779tms
blocked_by:
  - mara-yhxq5x
  - mara-xqcnqw
created_at: 2026-11-16T17:22:24Z
---

Goal: a daemon serves code links warm: it watches the declared code files and re-scans only what changed.

Split by the orchestrator on 2026-11-16 from mara-yhxq5x, which builds the markers for direct mode and `mara mcp`. This part needs the daemon's sessions and watching (mara-xqcnqw, alpha.4).

Context: DES-CODE-READERS in docs/code.mara.md; DES-INPUTS-AND-KEYS in docs/engine.mara.md; VER-CODE-MARKERS (its daemon cases) in docs/verification-code.mara.md; the warm rows for code inputs in VER-CODE-WORK-COUNTERS (docs/verification-code-use.mara.md) and the "One code file edited" row of VER-WORK-COUNTERS (docs/verification.mara.md); VER-NEVER-STALE in docs/verification-daemon.mara.md.

- [ ] The daemon watches declared code files; a request after a code file changed re-scans that file only
- [ ] The daemon cases of VER-CODE-MARKERS and the warm code rows of VER-CODE-WORK-COUNTERS and VER-WORK-COUNTERS pass
- [ ] The daemon case of VER-HOST-COMPARISON covers the operations that read code
