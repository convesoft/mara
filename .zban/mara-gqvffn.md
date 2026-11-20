---
title: Loading a grammar does not hold up other languages, and a load that times out is not tried again
status: todo
type: bug
parent: mara-779tms
created_at: 2026-11-20T20:22:51Z
---

Goal: one grammar that loads slowly or never finishes loading costs only the requests that need that grammar, and costs them once.

Context: Found by the maintainer's code audit of 2026-11-20 (at `a5f8ec0`, then `a5f8ec0..c6a9706`), filed at the maintainer's request. Locations were checked at `c6a9706`. Items: DES-GRAMMAR-ASSETS and ADR-GRAMMAR-WATCHDOG-THREAD (docs/code/index.mara.md), REQ-READS-DURING-VALIDATION (docs/performance.mara.md). Related backlog: mara-vts1tz (parses of one grammar run one at a time).

- `crates/mara/src/grammar.rs`, `get`: the process-wide registry (`REGISTRY`, line 136) is locked across `load`, which reads `grammar.wasm` and `tags.scm`, hashes both, checks the module's exports, starts the grammar's thread, compiles the module and the query, and waits up to `LOAD_LIMIT` (10 s). Meanwhile every other request that calls `grammar::get`, also for another language that is already loaded, and every `abandon` or `abandoned`, waits.
- A parse that overruns its budget is recorded in the registry's dropped set (mara-s9gak4). A load that times out (`loaded.recv_timeout(LOAD_LIMIT)`, line 316) returns an error without recording it, and a failed load is not reusable, so every later request tries again: another thread left behind and another 10 s with the registry locked, each time.

Done when both hold, with tests.

- [ ] A load in progress does not block `get` for another language or checksum; two requests for the same grammar still load it once
- [ ] A load that times out is recorded as dropped for the process, like a stalled parse: later requests get `unavailable-grammar` at once and start no thread; ADR-GRAMMAR-WATCHDOG-THREAD says so
- [ ] Tests: a second language is served while the first one's load stalls; after a load timeout the next request neither waits nor adds a thread
