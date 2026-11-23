---
title: One copy of the timestamp test helper and of the dispatcher's version comparison
status: todo
type: task
priority: deferred
tags:
  - deferred
  - backlog
  - audit
discovered_from: mara-c8scn7
created_at: 2026-11-23T10:22:28Z
---

Goal: `visibly` and `compareVersions` each exist once.

Context: architecture audit after rc.1. `crates/mara/src/inputs.rs` holds a verbatim copy of `visibly` from `crates/mara/tests/support/stat.rs` (mara-efyg6p). `packages/mara-npm/scripts/verify-install.mjs` re-implements `compareVersions` of `bin/mara.cjs` and writes the glibc version just below the floor as the literal "2.34".

- [ ] The unit tests of `inputs.rs` include the helper from the tests' support file; the install check takes `compareVersions` and the floor from the dispatcher
