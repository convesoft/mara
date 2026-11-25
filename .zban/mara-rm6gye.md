---
title: Load mara.node with standard require() instead of manual process.dlopen
status: todo
type: task
priority: deferred
tags:
  - deferred
  - backlog
  - audit
created_at: 2026-11-25T18:30:35Z
---

From the post-`mara-xzfeyj` architecture audit at `4cbcf26` (Finding #8, wave: `behavior`).

`packages/mara-npm/lib/index.js:66-81` constructs a synthetic `{ exports: {} }` object and calls `process.dlopen` manually instead of `require(path.join(nativePackage(), "bin", "mara.node"))`, bypassing Node's standard `.node` loader and `require.cache`.

- [ ] Replace `process.dlopen` with `require(path.join(nativePackage(), "bin", "mara.node"))`
