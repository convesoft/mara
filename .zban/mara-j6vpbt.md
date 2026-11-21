---
title: "Shared test helpers for the daemon tests: grammar threads, Soufflé processes, an MCP client, the grammar project"
status: todo
type: task
priority: deferred
tags:
  - deferred
  - backlog
  - audit
discovered_from: mara-0mxw21
created_at: 2026-11-21T16:35:20Z
---

Goal: each probe of the daemon tests exists once, under `crates/mara/tests/support/`.

Context: Found by the architecture audit after 0.4.0-alpha.4 (Gemini, at `2fa2c1b`, 2026-11-21); locations are as of that commit. Wave: seam. Three helpers read `/proc/<pid>/task` for `mara-grammar` threads (`tests/daemon/client.rs` lines 733-741; identical `grammar_threads` in `tests/code/facts.rs` and `tests/code/grammar.rs`). Finding a `souffle` child is written twice (`tests/daemon/fresh.rs`, `reads.rs`), an MCP client over stdio twice (`tests/daemon/mcp.rs` `Server`, `reads.rs` `Session`), and `client.rs` (lines 601-635) sets a grammar project up inline right above a call of `grammar::grammar_project`. `tests/mcp/guide.rs` (lines 146-150) compares the bound and unbound instructions one way, so an extra or reordered line in the bound text passes.

- [ ] One helper each; the tests use them
- [ ] `guide.rs` compares the two instruction texts for equality after substituting the project sentence

Note from the orchestrator, 2026-11-21 (architecture audit after alpha.5): worse since alpha.5. New copies: a daemon harness (`Daemon`) and an MCP client (`Mcp`) in `crates/mara/tests/code/main.rs`, two inline daemon harnesses in `crates/mara/tests/performance/code_get.rs`, and `Runtime` in `crates/mara/tests/performance/daemon.rs`. mara-xh6mp6 (alpha.6) fixes the macOS build of the first; mara-5d6pcs shares the CLI-to-MCP translation and the fixture-grammar installer.
