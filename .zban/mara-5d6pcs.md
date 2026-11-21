---
title: "Test helpers shared by the code-add, grammar and MCP tests: one CLI-to-MCP translation, one fixture-grammar installer"
status: todo
type: task
priority: deferred
tags:
  - deferred
  - backlog
  - audit
related:
  - mara-j6vpbt
  - mara-fwy8zs
discovered_from: mara-779tms
created_at: 2026-11-21T21:32:35Z
---

Goal: the integration tests translate a CLI call to an MCP call in one place and install a fixture grammar in one place.

Context: found by the architecture audit after 0.4.0-alpha.5 (Gemini, at `368fad1`, 2026-11-21); wave: seam. Locations are as of that commit.
- `crates/mara/tests/performance/code_add.rs` (near 139–149, 286–323, 401–446) parses the CLI's usage again, keeps a list of 25 operations by hand and has a second translator of CLI arguments to MCP arguments (`arguments`), which differs from `mcp_request` in `crates/mara/tests/support/requests.rs` (near 49–165) for `--opt=val` and integer arrays.
- Four test modules read a fixture `.wasm` and `tags.scm`, compute the checksum, write the per-user cache entry and patch `[code.languages.<lang>]` each in their own way: `tests/performance/work_counters.rs` (near 178–204), `tests/performance/code_add.rs` (near 109–128), `tests/daemon/client.rs` (near 612–630), `tests/code/main.rs` (near 77–80, 150–179); `tests/support/grammar_server.rs` (near 14–29) has the fixture versions. Two of them write `"0.24.2"` beside `FIXTURE_VERSIONS`.
The audit also reports mara-fwy8zs and mara-j6vpbt as worse after alpha.5: `tests/code/main.rs` (near 371–381) adds a fourth list of in-process operations, and alpha.5 added another daemon harness and another MCP client there, two inline daemon harnesses in `tests/performance/code_get.rs` (near 185–228, 247–276) and `Runtime` in `tests/performance/daemon.rs`.

- [ ] `tests/performance` uses `support/requests.rs` for usage and for the MCP form of a call; the 25-operation list and `arguments` are gone
- [ ] One helper in `tests/support/` installs a fixture grammar; the four copies use it, and no test names a fixture version beside `FIXTURE_VERSIONS`
