---
title: The licence notices are sorted the same way on every machine
status: todo
type: task
priority: deferred
tags:
  - deferred
  - backlog
  - audit
related:
  - mara-6qcfn5
discovered_from: mara-c8scn7
created_at: 2026-11-23T10:22:28Z
---

Goal: `THIRD-PARTY-LICENSES` has the same order whatever the build machine's locale.

Context: architecture audit after rc.1. `packages/mara-npm/scripts/third-party-licenses.mjs` sorts crates with `localeCompare`, so the inventory and the order of the licence texts depend on the locale. Moot if the maintainer chooses cargo-about (mara-6qcfn5).

- [ ] A code-point comparison; packaging fails when an entry of the table of covering crates is used by no crate
