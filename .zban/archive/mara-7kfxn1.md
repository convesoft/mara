---
title: One mechanism for debug-only test hooks, and a release build that holds none
status: done
type: task
parent: mara-779tms
discovered_from: mara-0mxw21
created_at: 2026-11-21T16:35:20Z
closed_at: 2026-11-21T18:01:28Z
---

Goal: no environment variable that begins `MARA_TEST_` is honoured by a release build, and a test proves it for all of them, present and future.

Context: Found by the architecture audit after 0.4.0-alpha.4 (Gemini, at `2fa2c1b`, 2026-11-21); locations are as of that commit. ADR-DEBUG-PAUSE-POINT (docs/authoring/writes.mara.md) requires debug-only hooks to be absent from release builds. There are eleven, in six modules: `MARA_TEST_PAUSE` (`write.rs`), `MARA_TEST_PAUSE_EVALUATION` (`rules.rs`), `MARA_TEST_PAUSE_WATCH` (`watch.rs`), `MARA_TEST_DERIVED` (`session.rs`), `MARA_TEST_IDLE_SECONDS` (`cli/daemon/server.rs`), `MARA_TEST_GRAMMAR_LOAD_MS`, `MARA_TEST_GRAMMAR_BUDGET_MS`, `MARA_TEST_GRAMMAR_STALL`, `MARA_TEST_GRAMMAR_LOAD_STALL`, `MARA_TEST_GRAMMAR_EVENTS` (`grammar.rs`) and `MARA_TEST_GRAMMAR_CATALOG` (`cli/code/catalog.rs`), which redirects where `mara code add` fetches from. The release checks (`crates/mara/tests/performance/pause_points.rs`, `idle_period.rs`) look in the binary for `MARA_TEST_PAUSE`, `MARA_TEST_DERIVED` and `MARA_TEST_IDLE_SECONDS` only: none of the six grammar variables is checked. The reviews found each of them compiled out by reading; nothing asserts it. The pause points are three implementations (one writes its marker atomically and reports errors, two do neither), and there are two appenders of event lines.

- [x] The release-build test fails when the binary holds the bytes `MARA_TEST_` anywhere
- [x] One internal module holds the hooks behind one `cfg(debug_assertions)` boundary: one pause, one appender of event lines; the existing hooks use it, with their variables' names and behaviour unchanged
- [x] ADR-DEBUG-PAUSE-POINT names the rule (the prefix and the one check)

## Summary of Changes

- New module `crates/mara/src/hooks.rs` (`mara::hooks`): the enum `Hook` of the eleven hooks, and `value`, the one function that names their variables and reads them. It is the one `cfg(debug_assertions)` boundary: a release build's `value` returns `None`, so it neither reads nor holds any name. On top of it: `path`, `number`, the one `pause` (creates `paused` whole through a staged file, waits until it is gone, reports errors) and the one `append` of event lines.
- `write.rs`, `rules.rs`, `watch.rs`, `session.rs`, `grammar.rs`, `cli/daemon/server.rs` and `cli/code/catalog.rs` use it. Their own pause functions, appenders, env reads and `cfg(debug_assertions)` stubs are gone. Every variable keeps its name and what it does in a debug build. No test file that uses them changed.
- `crates/mara/tests/performance/pause_points.rs`: new case `a_release_binary_holds_no_test_hook` fails when the release binary holds `MARA_TEST_` anywhere and names what it found. The per-variable byte checks in `pause_points.rs` and `idle_period.rs` are dropped in favour of it. The behavioural checks (a write that doesn't pause, a daemon that keeps the 30-minute idle period) stay.
- Shown to fail for the right reason: a temporary uncommitted change in `catalog.rs` that read `MARA_TEST_GRAMMAR_CATALOG` directly made the case fail with `the release binary holds MARA_TEST_, as ["MARA_TEST_GRAMMAR_CATALOG"]`. It passes at head. `grep -ac MARA_TEST_ target/release/mara` gives 0.
- Docs: ADR-DEBUG-PAUSE-POINT has the rule (the prefix, the module, the one check). DES-WRITE-PROCEDURE and DES-PERFORMANCE-HARNESS point to the one check.
- After merging the integration tip (5c155f4), the module comment of `catalog.rs` no longer says the shipped catalog is empty: it points to DES-GRAMMAR-ASSETS (closes mara-gjn0mr, at the orchestrator's request).

Checks, on the merged head: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked` with warnings denied, `cargo test --workspace --locked`, `project validate` (0 errors, 1 warning that was already there on `docs/distribution.mara.md`), `cargo test --package mara --test performance --release --locked`, the `wasm32` build of `mara-engine` and `zban check`: all pass.

## Decisions made

- The hooks live in `mara::hooks`, public but `#[doc(hidden)]`, only because the binary crate reads two hooks (`server.rs`, `catalog.rs`). `value`, `pause` and `append` are `pub(crate)`; the module exports only `Hook`, `path` and `number`. Recorded in ADR-DEBUG-PAUSE-POINT.
- The boundary is one function, `value`. Hook logic after it compiles in both builds, but every function of the module takes the hook and acts only on what `value` gives (`append` no longer takes a path), so a release build reaches none of it, from the CLI or from another crate, and holds none of the names. Recorded in ADR-DEBUG-PAUSE-POINT.
- An empty value counts as unset for every hook. This was already true for all of them except `MARA_TEST_GRAMMAR_EVENTS`, which used to take an empty value to mean the working directory. Recorded in ADR-DEBUG-PAUSE-POINT.
- The pause of an evaluation and of the watcher now also creates `paused` whole. When it can't create the file, it goes on without pausing, as before. Only the writer refuses the edit then. Recorded in ADR-DEBUG-PAUSE-POINT.
- The grammar load hold (wait while `hold` exists) stays in `grammar.rs`. It writes no marker, so it is not a pause point. Recorded in ADR-DEBUG-PAUSE-POINT.
