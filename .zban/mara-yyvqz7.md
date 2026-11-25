---
title: Use server.closeAllConnections() in loopback test HTTP servers instead of tracking sockets in a Set
status: todo
type: task
priority: deferred
tags:
  - deferred
  - backlog
  - audit
created_at: 2026-11-25T18:30:35Z
---

From the post-`mara-xzfeyj` architecture audit at `4cbcf26` (Finding #9, wave: `behavior`).

`packages/mara-npm/scripts/verify-hosts.mjs:91-106` and `crates/mara/tests/hosts/node/support/hosts.mjs:328-345` both track HTTP sockets in a `Set` to destroy them on close, where Node 24's `server.closeAllConnections()` replaces the boilerplate.

- [ ] Replace `sockets` `Set` tracking with `server.close(resolve); server.closeAllConnections();` in both files
