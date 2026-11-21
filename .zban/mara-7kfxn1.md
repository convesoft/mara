---
title: One mechanism for debug-only test hooks, and a release build that holds none
status: todo
type: task
parent: mara-779tms
discovered_from: mara-0mxw21
created_at: 2026-11-21T16:35:20Z
---

Goal: no environment variable that begins `MARA_TEST_` is honoured by a release build, and a test proves it for all of them, present and future.

Context: Found by the architecture audit after 0.4.0-alpha.4 (Gemini, at `2fa2c1b`, 2026-11-21); locations are as of that commit. ADR-DEBUG-PAUSE-POINT (docs/authoring/writes.mara.md) requires debug-only hooks to be absent from release builds. There are eleven, in six modules: `MARA_TEST_PAUSE` (`write.rs`), `MARA_TEST_PAUSE_EVALUATION` (`rules.rs`), `MARA_TEST_PAUSE_WATCH` (`watch.rs`), `MARA_TEST_DERIVED` (`session.rs`), `MARA_TEST_IDLE_SECONDS` (`cli/daemon/server.rs`), `MARA_TEST_GRAMMAR_LOAD_MS`, `MARA_TEST_GRAMMAR_BUDGET_MS`, `MARA_TEST_GRAMMAR_STALL`, `MARA_TEST_GRAMMAR_LOAD_STALL`, `MARA_TEST_GRAMMAR_EVENTS` (`grammar.rs`) and `MARA_TEST_GRAMMAR_CATALOG` (`cli/code/catalog.rs`), which redirects where `mara code add` fetches from. The release checks (`crates/mara/tests/performance/pause_points.rs`, `idle_period.rs`) look in the binary for `MARA_TEST_PAUSE`, `MARA_TEST_DERIVED` and `MARA_TEST_IDLE_SECONDS` only: none of the six grammar variables is checked. The reviews found each of them compiled out by reading; nothing asserts it. The pause points are three implementations (one writes its marker atomically and reports errors, two do neither), and there are two appenders of event lines.

- [ ] The release-build test fails when the binary holds the bytes `MARA_TEST_` anywhere
- [ ] One internal module holds the hooks behind one `cfg(debug_assertions)` boundary: one pause, one appender of event lines; the existing hooks use it, with their variables' names and behaviour unchanged
- [ ] ADR-DEBUG-PAUSE-POINT names the rule (the prefix and the one check)
