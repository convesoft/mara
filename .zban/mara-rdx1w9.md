---
title: Avoid repeated fact-file cloning, hashing, and structured-cloning in browser rule evaluation passes
status: todo
type: task
priority: deferred
tags:
  - deferred
  - backlog
  - audit
created_at: 2026-11-25T18:30:35Z
---

From the post-`mara-xzfeyj` architecture audit at `4cbcf26` (Finding #5, wave: `behavior`).

- `crates/mara/src/rules/browser.rs:147-166, 216-220, 258-264, 296-302` calls `files(run)` (cloning every `Cow::Borrowed` fact file) and concatenates all bytes into one `Vec<u8>` in `identity()` on every pass and provenance round, even on cache hits.
- `packages/mara-browser/worker.js:130-137` posts sliced `Uint8Array` buffers to `souffle-worker.js` without a `transfer` list, structured-cloning every fact file again.

- [ ] Stream the browser cache key (or reuse `evaluation::key` + provenance flag) and materialize `files(run)` only on a cache miss
- [ ] Transfer the sliced `ArrayBuffer`s in `worker.js:130-137`
