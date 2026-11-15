---
title: MCP server with CLI parity
status: todo
type: feature
parent: mara-6k50w0
blocked_by:
  - mara-w0xasr
  - mara-esxqzv
  - mara-pvqpse
  - mara-sj9jfq
  - mara-wjy3sh
created_at: 2026-11-13T22:10:06Z
---

Context: docs/hosts.mara.md (REQ-CLI-MCP-PARITY, DES-OPERATIONS).

- [x] Confirm the operation names with the maintainer and set DES-OPERATIONS to Accepted (ADR-GROUP-VERB-COMMANDS)
- [ ] Stdio MCP server exposing every document operation; project per call or bound at start
- [ ] VER-CLI-MCP-PARITY passes (docs/verification-hosts.mara.md), and the MCP case of VER-CONCURRENT-WRITERS (docs/verification-writes.mara.md)
- [ ] Work-counter and scaling cases for `mcp` (DES-PERFORMANCE-HARNESS)
