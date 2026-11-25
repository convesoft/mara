---
title: Unify duplicated host-test helpers across crates/mara/tests/hosts/
status: todo
type: task
priority: deferred
tags:
  - deferred
  - backlog
  - audit
created_at: 2026-11-25T18:30:35Z
---

From the post-`mara-xzfeyj` architecture audit at `4cbcf26` (Finding #11, wave: `seam`).

- `crates/mara/tests/hosts/node/support/hosts.mjs:69-86` repeats the positional-argument table defined in `packages/mara-browser/daemon.js:26-58` (`tools`).
- `lockstep.mjs:31-72` duplicates the `mara mcp` spawn, JSON-RPC initialize handshake, and stdout loop between `mcpDirect` and `toolList`.
- `library.mjs:27-33` duplicates `hosts.mjs:14-20` (`env`); `limits.test.mjs` and `timing.test.mjs` duplicate `LONG` and `until`/`waitFor`; `parity.test.mjs` and `work_counters.test.mjs` redefine `NO_WORK`/`NONE`.

- [ ] Derive `POSITIONAL` from `daemon.js`'s `tools`, extract `mcpCall` in `lockstep.mjs`, and share `env`, `NO_WORK`, `LONG`, and `waitFor`
