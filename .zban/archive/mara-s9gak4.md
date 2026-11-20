---
title: Grammar-level endpoints
status: done
type: feature
parent: mara-779tms
blocked_by:
  - mara-yhxq5x
created_at: 2026-11-13T22:10:06Z
closed_at: 2026-11-16T21:31:43Z
---

Context: DES-CODE-MARKERS in docs/code.mara.md.

- [x] Tree-sitter WASM grammars with `tags.scm`; endpoint is file and owning declaration
- [x] ABI check on load; fallback to the basic level with a diagnostic

- [x] VER-GRAMMAR-MARKERS passes, and the grammar-level cases of VER-CODE-FACTS
- [x] VER-CODE-EDITS passes with the grammars installed (markers before declarations, marker-like lines in raw strings), and the grammar-level `get` row of VER-CODE-WORK-COUNTERS (1 code scan); mara-yhxq5x ran both at the basic level only

## Summary of Changes

Grammar-level endpoints, in direct mode and through `mara mcp`.

- Configuration: `[code.languages.<name>]` takes an optional `grammar = { version, checksum = "blake3:…" }` pin (`mara_engine::config::GrammarPin`); an invalid pin is `invalid-value` and leaves the language at the basic level.
- Runtime (`crates/mara/src/grammar.rs`, `tree-sitter` 0.27 with its `wasm` feature, only in the `mara` crate): the pinned files are read from the per-user cache, checked against the checksum, loaded into tree-sitter's WebAssembly store with the ABI range checked, and run on a thread per grammar. A parse and its tags query are cancelled at a budget of 1 s + 1 s per 256 KiB; a grammar whose thread does not answer by twice that and 1 s more is abandoned for the process. A missing, mismatching, unloadable or abandoned grammar is `unavailable-grammar` at the pin's line, naming `mara code add`; a file it could not parse falls back alone, with `unavailable-grammar` naming the language and the file.
- Scan (`mara_engine::code`, `code/outline.rs`): from the comment, attribute and `@definition.*` ranges a grammar found, markers only in comment nodes, each owned by the declaration its comment precedes, else the one containing it, else the file; the owner of any line likewise. The language key holds the grammar's checksum, so a file is scanned once per content and grammar.
- Reads: a code node carries its location line and `declaration` at the grammar level; `relation list <path>:<line>` lists the markers of the location that owns the line; `get <path>:<line>` returns that declaration, or the whole file, scanning that one file (1 code scan).
- Docs: DES-CODE-MARKERS, DES-CODE-READERS, DES-GRAMMAR-ASSETS (pin, cache, time and memory bounds), the `unavailable-grammar` row of DES-SCHEMA-CHECKS, VER-GRAMMAR-MARKERS's reference to this issue, and ADR-GRAMMAR-WATCHDOG-THREAD.
- Fixtures: official release files of tree-sitter-rust v0.24.2 and tree-sitter-python v0.25.0 (`.wasm` and `queries/tags.scm` from the release tarball), with source, SHA-256 and MIT licences in `crates/mara/tests/fixtures/grammars/`.
- Tests: `tests/code/grammar.rs` (VER-GRAMMAR-MARKERS through the CLI and `mara mcp`, a cancelled and a never-returning parse), `edits::renames_markers_and_nothing_else_with_grammars` (VER-CODE-EDITS with grammars, raw strings untouched, CLI and `mara mcp`), `work_counters::grammar_get_counts` (the grammar `get` row), engine unit tests of ownership and of markers only in comment nodes.

Left elsewhere: the daemon cases (noted on mara-9fphtg); every case of VER-CODE-FACTS, which needs code links as program input (noted on mara-hk2arx); `code add` and the catalog (mara-h0hxa3).

Cost: a clean release build of `mara` takes 93 s instead of 21 s, and the binary is 27.5 MB instead of 10.2 MB (wasmtime with Cranelift). `cargo test --workspace` uses 2 m 12 s of CPU instead of 1 m 33 s, with nothing to build. A release grammar-level request loads a grammar in about 30 ms; a debug one in about 0.6 s.

Checks: `cargo fmt --all --check`, clippy with warnings denied, `cargo test --workspace --locked` (590 passed), `project validate` (valid), the release `performance` test (20 passed, 2 ignored), the `wasm32` build of `mara-engine`, `zban check`: all exit 0.

## Decisions made

- The pin: `grammar = { version, checksum }` in the language's table; checksum = BLAKE3 of the BLAKE3 hashes of the `.wasm` and `tags.scm`; version restricted to letters, digits, `.-_+`, not starting with `.`, since it is a path component (DES-GRAMMAR-ASSETS).
- The cache: `$XDG_CACHE_HOME/mara/grammars` (absolute only), else `~/.cache/mara/grammars`, `~/Library/Caches/mara/grammars` on macOS; files at `<language>/<version>/<checksum>/grammar.wasm` and `tags.scm`; the tree-sitter name is the language's name with `-` as `_` (DES-GRAMMAR-ASSETS).
- Grammar files are not counted among input reads and hashes, so the `get` row holds as written; a host reloads a grammar only when its files' stats change (DES-GRAMMAR-ASSETS).
- Time and memory: 128 MiB (tree-sitter's store), 1 s + 1 s per 256 KiB per file, a watchdog at twice that plus 1 s that abandons the grammar for the process; wasmtime epochs and fuel cannot be used through tree-sitter's API (DES-GRAMMAR-ASSETS, ADR-GRAMMAR-WATCHDOG-THREAD).
- A file the grammar could not parse falls back alone with `unavailable-grammar` at the pin's line naming the file, keeping that diagnostic's documented place (DES-GRAMMAR-ASSETS, DES-SCHEMA-CHECKS).
- Attributes between a marker and its declaration are nodes of kinds `attribute_item`, `inner_attribute_item`, `decorator`, `annotation`, `marker_annotation`, `attribute_list`; a preceded declaration may also be the child of a node starting there on the same line (`export function`); "the start of its line" is its first text (DES-CODE-MARKERS).
- The owner of an arbitrary line follows the marker rules, and `get` of a line the file owns returns the whole file; `get <path>` also scans the file at the grammar level, so that it reports `unavailable-grammar` like every operation that reads code (DES-CODE-MARKERS, DES-CODE-READERS).
- Every file of a grammar language is parsed, not only those holding `@mara`; skipping the others is filed as backlog mara-jtjr3h.
- Test hooks in debug builds only, after ADR-DEBUG-PAUSE-POINT: `MARA_TEST_GRAMMAR_BUDGET_MS` and `MARA_TEST_GRAMMAR_STALL`, since no released grammar traps or loops.

## Rejected approaches

- Wasmtime epoch interruption or fuel: tree-sitter creates the store and exposes neither; both trap immediately at their defaults.
- A child process per parse: a grammar compile on every request and a counted subprocess.

## Review fixes

The review of `5dff734` (fallback cases) found four defects. Fixed:

1. A grammar dropped in the middle of a request left the files scanned before it at the grammar level. The walk now reads and scans again, at the basic level, each file of that language it had scanned with the grammar (`Walk::rescan` in `inputs.rs`), so reads, `item delete` and `item rename` all see the basic level the diagnostic names; the message has one "so". Test: `grammar::a_grammar_dropped_during_a_request_leaves_its_language_at_the_basic_level` (relation list and rename with a marker-like line in a raw string of `src/a.rs` and the stall in `src/z.rs`).
2. A dropped grammar was not remembered across requests of one process. Dropped grammars are now recorded process-wide by language name and checksum (`grammar::dropped`); a later request neither loads nor waits for them, and starts no thread. Test: `grammar::a_dropped_grammar_stays_dropped_in_its_process` (three `relation_list` calls in one `mara mcp`: only the first waits, and the thread count stays the same).
3. Two files with the same contents that fall back were both named after the first. A scan's fallback reason no longer names a file; each file's diagnostic adds its own path. Test: `grammar::each_file_that_falls_back_is_named`.
4. A valid WebAssembly module that is no grammar ended the process by an assertion of tree-sitter's store. Before loading, Mara now checks the module's exports (`grammar/module.rs`): `tree_sitter_<name>` returns an `i32` from no parameters, and each initializer takes and returns nothing. Tests: `grammar::a_module_that_is_no_grammar_falls_back` (the review's module), `grammar::module::tests::takes_a_grammar_and_refuses_other_modules`. A module that passes and still breaks another assertion of the store can end the process: stated in DES-GRAMMAR-ASSETS, filed as mara-6pkdpr.

Found while testing: a grammar's thread dropping its WebAssembly code while the process exited aborted in `__deregister_frame` (one core dump of `relation list` in a debug test run). A released grammar's thread is now joined, unless it was abandoned, and a thread whose load failed is joined before the failure returns.

Each test above failed with its fix removed (the module check's mutation was run with core dumps off). Daemon parity: `client::code_operations_through_the_daemon_match_direct_mode` compares `get`, `relation list` and `project validate` of code locations, with their stats lines, served by the daemon and in direct mode at the grammar level. Docs: DES-GRAMMAR-ASSETS (export check and its limit, a dropped grammar for the whole request and the process, the daemon's cache), ADR-GRAMMAR-WATCHDOG-THREAD (what ends the dropped state: the process's end; another checksum is another grammar).

## Decisions made in the review round

- The dropped record lives as long as the process, for every request and project it serves; a changed cache file under the same pin does not clear it, and a new checksum is a new grammar.
- A daemon finds the grammar cache from its own environment, that of the client that started it; the request does not carry `HOME` or `XDG_CACHE_HOME` (DES-GRAMMAR-ASSETS).

## Review fixes, second part

The rest of the review of `5dff734` found two more. Fixed:

5. Ownership work was quadratic in a run of comments: each comment skipped the remaining run again. `Outline::new` now resolves where the text after each comment or attribute continues once, from the last run back (`crates/mara-engine/src/code/outline.rs`), so a run of N comments costs O(N log N) at most (a binary search per run). Test: `scaling::grammar_growth_per_doubling`, the first scaling case at the grammar level: `relation list` of a marker before 1,000 × size lines of `///` documentation and a function, through the release CLI with the Rust grammar.
6. The grammar was loaded again on every call of one `mara mcp`, since its cache lived in each request's `Inputs`. Loaded grammars and the record of dropped ones now share one process-wide registry (`grammar::get`); each request still checks the cached files' stats. A grammar's parses are serialized, so that one request's watchdog never counts another request's job. Test: `grammar::one_process_loads_a_grammar_once` (two `get` calls in one `mara mcp`: one grammar thread, the same; a removed cache directory is seen by the third call).

Not changed: `project validate` still loads every pinned grammar, also one whose language has no file. Loading on a language's first file would drop the `unavailable-grammar` of an unused language's missing grammar; DES-GRAMMAR-ASSETS now says so. In a long-running process the registry makes the load once.

Docs: DES-GRAMMAR-ASSETS also states that the 128 MiB is the module's linear memory, not the process's; that finding markers and owners runs after the time budget, in linear work; and what the store gives a module (a C library subset, the lexer callbacks, builtins including `tree_sitter_debug_message`, which prints to standard output).

Each test failed with its fix removed (HEAD's `outline.rs` for 5; no reuse in `grammar::get` for 6).
