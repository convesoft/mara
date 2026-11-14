---
title: Run the code-link case of VER-ONE-HOP
status: todo
type: task
parent: mara-779tms
related:
  - mara-bmc9gc
discovered_from: mara-km8qqj
created_at: 2026-11-14T13:26:53Z
---

Goal: `relation list` returns code links, as REQ-DIRECT-NEIGHBOURS (docs/retrieval.mara.md) requires.

Context: VER-ONE-HOP (docs/verification-retrieval.mara.md) expects the code link of item A in `relation list`. Code links arrive in this slice, so mara-bmc9gc passes VER-ONE-HOP without that case.

- [ ] VER-ONE-HOP passes with its code-link case
