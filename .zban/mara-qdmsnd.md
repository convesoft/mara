---
title: An MCP call whose arguments object names a parameter twice takes the last value
status: todo
type: bug
priority: deferred
tags:
  - deferred
  - backlog
discovered_from: mara-v9tfhw
created_at: 2026-11-21T09:40:17Z
---

Goal: decide whether a JSON-RPC `tools/call` whose `arguments` object has a duplicate key, such as `{"limit": 1, "limit": 2}`, is rejected like a repeated CLI option.

Context: DES-OPERATIONS (docs/hosts/index.mara.md) now says that an option that does not repeat, given twice, is rejected with `invalid-argument` and its last value never wins (mara-v9tfhw). In MCP such a parameter takes one value. `rmcp` parses the request into a `serde_json::Map`, which keeps the last of duplicate keys without reporting them, so `crates/mara/src/cli/mcp.rs` never sees the first. RFC 8259 leaves duplicate names to the parser. Rejecting them would need a duplicate-detecting parse before or inside `rmcp`, which reaches into the SDK's request handling (ADR-RMCP-SERVER). This is unverified against a real client and no known client sends duplicate keys.

- [ ] Decide: accept the parser's behaviour and say so in DES-MCP-SERVER, or reject duplicates and say how without re-implementing `rmcp`'s parsing
