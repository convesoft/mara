---
title: Node library and shared typed client
status: todo
type: feature
parent: mara-xzfeyj
blocked_by:
  - mara-jptfbq
  - mara-fkkcad
  - mara-xsm8w6
created_at: 2026-11-13T22:10:06Z
---

Context: REQ-NODE-LIBRARY in docs/hosts.mara.md; DES-PACKAGES in docs/distribution.mara.md.

- [x] Decide how Node embeds the engine: the native addon (ADR-NODE-NATIVE-ADDON)
- [ ] One typed asynchronous ESM client over the embedded, daemon, HTTP and worker backends
- [ ] The same test suite passes against each backend
