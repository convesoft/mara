---
title: souffle-worker.js leaves WebAssembly.instantiate rejections unhandled, hanging until timeout
status: todo
type: bug
tags:
  - audit
parent: mara-qzp1gd
created_at: 2026-11-25T18:30:35Z
---

From the post-`mara-xzfeyj` architecture audit at `4cbcf26` (Finding #3, wave: `behavior`).

In `packages/mara-browser/souffle-worker.js:121-124`, `instantiateWasm` runs `WebAssembly.instantiate(compiled, imports).then((created) => done(created, compiled))` with no `.catch()`. If instantiation rejects (e.g. `RangeError` OOM allocating the 32 MiB initial memory in a constrained tab), `done()` is never called, `await factory(instance)` never settles, and the parent worker waits out `run.time_limit_ms` (default 10 s) before terminating it and misreporting `limit: "time"` instead of `limit: "memory"` or `fault`. Also hoist `TextEncoder`/`TextDecoder` at lines 69 and 174 to module constants.

- [ ] Handle `WebAssembly.instantiate` rejections immediately in `souffle-worker.js`, reporting `limit: "memory"` on OOM or `fault` on other instantiation errors
- [ ] Hoist `TextEncoder` and `TextDecoder` to module constants
- [ ] Regression test and local checks pass
