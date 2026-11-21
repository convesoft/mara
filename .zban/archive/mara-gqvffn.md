---
title: Loading a grammar does not hold up other languages, and a load that times out is not tried again
status: done
type: bug
parent: mara-779tms
created_at: 2026-11-20T20:22:51Z
closed_at: 2026-11-21T10:05:22Z
---

Goal: one grammar that loads slowly or never finishes loading costs only the requests that need that grammar, and costs them once.

Context: Found by the maintainer's code audit of 2026-11-20 (at `a5f8ec0`, then `a5f8ec0..c6a9706`), filed at the maintainer's request. Locations were checked at `c6a9706`. Items: DES-GRAMMAR-ASSETS and ADR-GRAMMAR-WATCHDOG-THREAD (docs/code/index.mara.md), REQ-READS-DURING-VALIDATION (docs/performance.mara.md). Related backlog: mara-vts1tz (parses of one grammar run one at a time).

- `crates/mara/src/grammar.rs`, `get`: the process-wide registry (`REGISTRY`, line 136) is locked across `load`, which reads `grammar.wasm` and `tags.scm`, hashes both, checks the module's exports, starts the grammar's thread, compiles the module and the query, and waits up to `LOAD_LIMIT` (10 s). Meanwhile every other request that calls `grammar::get`, also for another language that is already loaded, and every `abandon` or `abandoned`, waits.
- A parse that overruns its budget is recorded in the registry's dropped set (mara-s9gak4). A load that times out (`loaded.recv_timeout(LOAD_LIMIT)`, line 316) returns an error without recording it, and a failed load is not reusable, so every later request tries again: another thread left behind and another 10 s with the registry locked, each time.

Done when both hold, with tests.

- [x] A load in progress does not block `get` for another language or checksum; two requests for the same grammar still load it once
- [x] A load that times out is recorded as dropped for the process, like a stalled parse: later requests get `unavailable-grammar` at once and start no thread; ADR-GRAMMAR-WATCHDOG-THREAD says so
- [x] Tests: a second language is served while the first one's load stalls; after a load timeout the next request neither waits nor adds a thread

## Summary of Changes

- `crates/mara/src/grammar.rs`: the registry is locked only to look a grammar up. Each grammar (language name and checksum) has a slot of its own, `Arc<Mutex<Option<Loaded>>>`, which a load holds locked: a request for another grammar is served meanwhile, and one for the same grammar waits for that load and reuses it. A load that does not end within `LOAD_LIMIT` (10 s) is recorded as dropped for the process with the reason "did not load within 10.0 s and is no longer used in this process", and its slot leaves the registry; later requests, and those that waited for that load, get `unavailable-grammar` at once and start no thread. `abandon` (stalled parse) and the load timeout share `drop_for_process`. A load whose thread ends without answering (a panic) is joined and reported as "cannot be loaded", instead of being reported as a timeout.
- Debug-only test hooks, after ADR-DEBUG-PAUSE-POINT and the existing grammar hooks: `MARA_TEST_GRAMMAR_LOAD_STALL=<tree-sitter name>` parks that grammar's thread before it loads anything; `MARA_TEST_GRAMMAR_LOAD_MS` replaces the 10 s load limit.
- Docs: DES-GRAMMAR-ASSETS (the 10 s load limit, a timed-out load dropped for the process, a load holds up only requests for its own grammar), ADR-GRAMMAR-WATCHDOG-THREAD (load timeout dropped the same way; trying a timed-out load again rejected), VER-GRAMMAR-MARKERS (two cases below).
- Tests: `code::grammar::a_grammar_that_does_not_load_in_time_stays_dropped_in_its_process` (three `relation_list` calls to one `mara mcp`: only the first waits 2 s, all report the timeout and basic-level endpoints, the grammar thread count stays 1); `daemon::client::a_grammar_that_does_not_load_holds_up_no_other_language` (one daemon: while a Rust project's `project validate` waits for a stalled load, a Python project's `get src/lib.py:2` returns the declaration; a second request for the stalled grammar waits and starts no other load: one `mara-grammar-ru` thread). Each fails with its fix removed: a global lock around `get` made the daemon test fail (the other request answered only after 10.5 s); not recording the timeout made the mcp test fail (call 1 waited 2.0 s).

Checks: `cargo fmt --all --check`, clippy with warnings denied, `cargo test --workspace --locked` (all pass), `project validate` (valid, 0 errors, 1 pre-existing warning), the release `performance` test under `berth --exclusive` (22 passed, 2 ignored), the `wasm32` build of `mara-engine`, `zban check`: all exit 0. No new entry in `coredumpctl list mara` after the runs.

## Decisions made

- The load limit stays 10 s and a timed-out load is dropped for the process by language name and checksum, like a stalled parse; it ends with the process, and another checksum is another grammar (DES-GRAMMAR-ASSETS, ADR-GRAMMAR-WATCHDOG-THREAD).
- A request that was waiting for a load of the same grammar when it timed out gets `unavailable-grammar` with the same reason, without trying again (DES-GRAMMAR-ASSETS).
- A load that ends while the grammar is dropped (by a stalled parse of the grammar it replaced) is not used by that request (DES-GRAMMAR-ASSETS: later requests neither load nor wait).
- Test hooks `MARA_TEST_GRAMMAR_LOAD_STALL` and `MARA_TEST_GRAMMAR_LOAD_MS`, debug builds only, like `MARA_TEST_GRAMMAR_STALL` and `MARA_TEST_GRAMMAR_BUDGET_MS` (VER-GRAMMAR-MARKERS).

## Review fixes

The review of `249a65e` found five things to fix; all are fixed here.

1. Requests that waited for a load share its failure. A slot (`Slot` in `grammar.rs`) counts the loads that ended (`attempts`). A request reads that count before it waits; if a load ended meanwhile, it takes that load's result, grammar or failure, without loading. A later request that waited for nothing checks the files again and loads again after a failure.
2. A load that ends after its request gave up keeps what it loaded: its thread parks with the `Worker` instead of dropping its WebAssembly code, which could coincide with the process's exit. This was backlog mara-dn4bkp, filed during this issue and deleted again here, since it is done in this change. Recorded in ADR-GRAMMAR-WATCHDOG-THREAD.
3. Tests ordered by events, not by time. New debug hook `MARA_TEST_GRAMMAR_EVENTS=<dir>`: the process appends `load <name>` when a grammar's thread starts a load and `wait <name>` when a request waits for another's load, and a load that `MARA_TEST_GRAMMAR_LOAD_STALL` names waits while `<dir>/hold` exists, instead of forever. `daemon::grammar::a_grammar_load_holds_up_only_requests_for_its_grammar` replaces the earlier daemon test. It moves to the new `tests/daemon/grammar.rs` with `grammar_project`, because `client.rs` was at 870 lines after the merge. The test holds the load of a Rust grammar with an invalid tags query and waits for the `load rust` event. It then starts a request of a second project for that grammar and waits for `wait rust`. A Python `get` in a third project returns its declaration with no `unavailable-grammar` while both Rust requests are still running. The test then removes `hold`: both requests report `unavailable-grammar` with the query's failure, after one `load rust`, and a later request loads again (two). The load limit is 120 s, so it never ends the load. The mcp test's limit is 6 s, with later calls bounded by 3 s, against about 0.1 s for a basic-level call. Mutations: without failure sharing, the daemon test fails with two loads; with a global lock around `get`, no `wait rust` event arrives.
4. The files' stats are sampled under the slot's lock, so that a request cannot store a load under stats older than another request's load.
5. DES-GRAMMAR-ASSETS now states what the 10 s cover: from the start of the grammar's thread to the end of compiling its module and tags query, with reading, hashing and checking the files before that not counted. It also states that a load failing for another reason gives its failure to the requests that waited for it, and that a later request tries again. VER-GRAMMAR-MARKERS names the hooks its two cases use.

Merged `chore/clear-worktree-for-rewrite` at `a6ac4c6`. The seven local checks were run again at the new head.

Added decisions:
- A load that ended while a request waited gives its result to that request even if the files changed meanwhile. The next request checks the stats again (DES-GRAMMAR-ASSETS).
- The late `Worker` is kept by parking its thread, not by `mem::forget`: no thread exit work runs either (ADR-GRAMMAR-WATCHDOG-THREAD).
- Test hook `MARA_TEST_GRAMMAR_EVENTS`, debug builds only (VER-GRAMMAR-MARKERS).

## Re-check fixes

The re-check of `2f47e42` found two more defects. Both are fixed in `crates/mara/src/grammar.rs`.

1. A request that gave up on a grammar's thread could still see that thread tear its WebAssembly code down. This could happen when a load ended in the moment after its timeout, when a load ended late with an error, and when a parse that had been abandoned ended later. The request and the thread now share a state, `LOADING`, `ANSWERED` or `GAVE_UP`, which replaces the `abandoned` flag. The thread claims `ANSWERED` by compare-and-swap before it sends its answer, and before any load failure drops what it loaded (the `answer` callback passed into `Worker::load`). If the request has already given up, the thread parks and keeps what it loaded. On timeout, the request claims `GAVE_UP` by compare-and-swap. If the thread had already claimed `ANSWERED`, the request waits for the answer, which is on its way. When `run` returns after the request gave up, whether at load time or at an abandoned parse, the thread parks. A released grammar's thread is still joined and still ends. Recorded in ADR-GRAMMAR-WATCHDOG-THREAD.
2. A request that waited on a load could take that load's result for other files than its own: two pins with one checksum but different versions name different cache directories. A finished load now records its directory (`Loaded::directory`), and a waiter shares the result only when that directory is its own. Otherwise it goes on as a request that did not wait: it checks the stats of its own files and loads them.

Merged `chore/clear-worktree-for-rewrite` at `6fac655`, and ran the seven local checks again at the new head.
