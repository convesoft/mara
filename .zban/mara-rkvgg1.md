---
title: mara mcp drops a malformed JSON line without a parse error
status: todo
type: bug
priority: deferred
tags:
  - deferred
  - backlog
discovered_from: mara-8vqk4a
created_at: 2026-11-16T12:56:13Z
---

Goal: a line that is not JSON gets a JSON-RPC parse error from `mara mcp`.

Context: found by the review of mara-8vqk4a at `cb03d4b` and rated exotic, so it went to the backlog under the maintainer's rules of 2026-11-15. Nothing here starts without the maintainer. It is `rmcp` transport behaviour reached through `crates/mara/src/cli/mcp.rs:63`, and needs corrupt input from a custom or broken client.

Reviewer's reproduction: after initialization, send `{broken json` followed by `{"jsonrpc":"2.0","id":9,"method":"ping"}`. Only the ping receives a response; malformed JSON is silently discarded rather than answered with a parse error. The server keeps serving and exits normally.

- [ ] Decide whether to configure or patch around `rmcp`, report it upstream (posting to an external project needs the maintainer's go-ahead), or state the behaviour in DES-MCP-SERVER
