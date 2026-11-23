---
title: "Split the install check: the case runner and process harness apart from the cases"
status: todo
type: task
priority: deferred
tags:
  - deferred
  - backlog
  - audit
blocked_by:
  - mara-09wgx8
discovered_from: mara-c8scn7
created_at: 2026-11-23T10:22:28Z
---

Goal: `packages/mara-npm/scripts/verify-install.mjs` holds the verification cases, and a helper module holds the harness.

Context: architecture audit after rc.1. The script has 590 lines, a 263-line `main()` and module-global state for started processes; it mixes process-table inspection, the npm sandbox install, raw MCP framing and all cases. rc.2 and macOS add cases to it. It stays pure Node (`VER-INSTALL-WITHOUT-TOOLCHAIN`).

- [ ] `verifyCase`, `processExecutables`, `install` and `mcpInitialize` in a helper module with explicit state
