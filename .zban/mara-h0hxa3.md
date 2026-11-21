---
title: mara code add
status: in-progress
type: feature
parent: mara-779tms
blocked_by:
  - mara-s9gak4
created_at: 2026-11-13T22:10:06Z
---

Context: docs/code.mara.md (REQ-ONLY-CODE-ADD-DOWNLOADS, DES-GRAMMAR-ASSETS).

- [ ] Built-in catalog and fetch from a given repository
- [ ] Checksum verification, per-user cache, pin in the project configuration
- [ ] VER-CODE-ADD passes, and its row of VER-CODE-WORK-COUNTERS: no other operation connects to another machine (the daemon's loopback listener is not such a connection)

- [ ] Set ADR-GRAMMAR-TRUST-ON-FIRST-USE (docs/code.mara.md) to Accepted and replace "Chosen provisionally by the orchestrator … for the maintainer to confirm" by the maintainer's confirmation of 2026-11-16: asked to choose between trust on first use, a required checksum and catalog only, the maintainer chose trust on first use
