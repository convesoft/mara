---
title: Browser-local workspace
status: in-progress
type: feature
parent: mara-xzfeyj
created_at: 2026-11-13T22:10:06Z
---

Context: REQ-BROWSER-LOCAL-WORKSPACE and DES-BROWSER-WORKSPACE in docs/hosts/index.mara.md; REQ-UNAVAILABLE-NOT-PARTIAL in docs/engine.mara.md.

- [x] Engine as WASM in a worker over supplied file contents
- [x] In-memory edits exported as a change set with base hashes
- [x] Capabilities the browser lacks are reported as unavailable
- [x] The Node and browser test setup of VER-HOST-COMPARISON (docs/verification.mara.md): `node --test` modules started by a Rust integration test, Playwright's Chromium
- [x] Tests: every case of VER-BROWSER-WORKSPACE (docs/hosts/verification.mara.md) passes but the program part of the caller-write row (mara-xsm8w6); the workspace parts of the first two rows of VER-UNAVAILABLE-NOT-PARTIAL; the browser workspace rows of VER-WORK-COUNTERS (docs/verification.mara.md) marked mara-jptfbq, through the workspace's debug stats option (DES-PERFORMANCE-HARNESS)
