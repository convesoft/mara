---
title: Node library and shared typed client
status: in-progress
type: feature
parent: mara-xzfeyj
blocked_by:
  - mara-jptfbq
  - mara-fkkcad
  - mara-xsm8w6
created_at: 2026-11-13T22:10:06Z
---

Context: REQ-NODE-LIBRARY in docs/hosts.mara.md; DES-PACKAGES in docs/distribution.mara.md; the Node client's build (DES-DAEMON-LIFECYCLE) and debug stats (DES-PERFORMANCE-HARNESS).

- [x] Decide how Node embeds the engine: the native addon (ADR-NODE-NATIVE-ADDON)
- [ ] One typed asynchronous ESM client over the embedded, daemon, HTTP and worker backends
- [ ] The same test suite passes against each backend
- [ ] Tests: the first three rows of the Node and browser table of VER-HOST-COMPARISON (docs/verification.mara.md) pass; every case of VER-NODE-LIBRARY (docs/verification-browser.mara.md); the Node rows of VER-DAEMON-START and VER-DAEMON-WRITES (docs/verification-daemon.mara.md); the embedded Node row of VER-CONCURRENT-WRITERS (docs/verification-writes.mara.md); the Chromium case of VER-BROWSER-ACCESS; the through-the-daemon part of the first row of VER-UNAVAILABLE-NOT-PARTIAL
