---
title: Code links in programs and the template's code cases
status: todo
type: feature
parent: mara-779tms
blocked_by:
  - mara-yhxq5x
  - mara-tx0fcf
created_at: 2026-11-16T17:22:19Z
---

Goal: rule programs can use code links, and the engineering template's gates that need them say more than "unavailable".

Split by the orchestrator on 2026-11-16 from mara-yhxq5x, which builds the markers themselves. This part needs the program evaluation and query operations of alpha.3 and the template's programs (mara-tx0fcf).

Context: DES-CODE-READERS and DES-CODE-MARKERS in docs/code.mara.md; the code input relation in DES-FACT-BRIDGE (docs/rules-facts.mara.md); DES-TEMPLATE-GATES in docs/template-readiness.mara.md; VER-CODE-FACTS and VER-CODE-WORK-COUNTERS in docs/verification-code-use.mara.md; VER-TEMPLATE-CASES in docs/verification.mara.md.

- [ ] Code links as an input relation for programs, and `source_complete("code")` (DES-CODE-READERS): VER-CODE-FACTS passes at the basic level
- [ ] The code-dependent cases of VER-TEMPLATE-CASES pass: `implemented`, `verified`, `release-ready`
- [ ] The rows of VER-CODE-WORK-COUNTERS for `query execute`, `query view`, `query gate` and `query report` of programs that use code links, and of programs that do not, pass
