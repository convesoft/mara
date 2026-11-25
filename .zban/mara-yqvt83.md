---
title: Use single-pass raw JSON framing in mara-node instead of double-encoding replies and debug_stats
status: todo
type: task
priority: deferred
tags:
  - deferred
  - backlog
  - audit
created_at: 2026-11-25T18:30:35Z
---

From the post-`mara-xzfeyj` architecture audit at `4cbcf26` (Finding #7, wave: `behavior`).

`crates/mara-node/src/lib.rs:90-98` formats `debug_stats::line(&work)`, parses it back to `serde_json::Value`, double-escapes `reply.text` inside `json!({ "text": ... })`, and `packages/mara-npm/lib/index.js:118-125` runs `JSON.parse` twice — whereas `crates/mara-wasm/src/lib.rs:318-329` already splices raw JSON so JS parses once.

- [ ] Emit single-pass raw JSON from `mara-node` and parse once in `packages/mara-npm/lib/index.js`
