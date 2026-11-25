---
title: npm packages
status: todo
type: feature
parent: mara-qzp1gd
blocked_by:
  - mara-k053zx
  - mara-4dr2p2
  - mara-865a2e
created_at: 2026-11-13T22:10:06Z
---

Context: docs/distribution.mara.md (REQ-RUN-WITHOUT-TOOLCHAIN, DES-PACKAGES, ADR-SCRIPT-FREE-NPM-PACKAGES).

- [ ] The Node addon in each native package (the dispatcher and the Linux native packages with the CLI and `souffle` are mara-svq540 in the rc.1 epic; macOS: mara-ag4ees)
- [ ] Browser package with the engine and Soufflé WASM modules
- [ ] The install verification of mara-svq540 also loads the Node addon and the browser package, without lifecycle scripts

The Linux part of the three items is mara-865a2e in the rc.2 slice (mara-xzfeyj); this issue checks them for 0.4.0 after it.
