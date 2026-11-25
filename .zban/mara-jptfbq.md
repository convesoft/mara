---
title: Browser-local workspace
status: in-progress
type: feature
parent: mara-xzfeyj
created_at: 2026-11-13T22:10:06Z
---

Context: REQ-BROWSER-LOCAL-WORKSPACE and DES-BROWSER-WORKSPACE in docs/hosts.mara.md; REQ-UNAVAILABLE-NOT-PARTIAL in docs/engine.mara.md.

- [ ] Engine as WASM in a worker over supplied file contents
- [ ] In-memory edits exported as a change set with base hashes
- [ ] Capabilities the browser lacks are reported as unavailable
- [ ] The Node and browser test setup of VER-HOST-COMPARISON (docs/verification.mara.md): `node --test` modules started by a Rust integration test, Playwright's Chromium
- [ ] Tests: every case of VER-BROWSER-WORKSPACE (docs/verification-browser.mara.md) passes but the program part of the caller-write row (mara-xsm8w6); the workspace parts of the first two rows of VER-UNAVAILABLE-NOT-PARTIAL; the browser workspace rows of VER-WORK-COUNTERS (docs/verification.mara.md) marked mara-jptfbq, through the workspace's debug stats option (DES-PERFORMANCE-HARNESS)
