---
title: "Client routing: on-demand start, first call in-process"
status: todo
type: feature
parent: mara-0mxw21
blocked_by:
  - mara-17gykj
created_at: 2026-11-13T22:10:06Z
---

Context: REQ-FIRST-CALL-IN-PROCESS and DES-DAEMON-LIFECYCLE in docs/daemon.mara.md.

- [ ] CLI and MCP clients start the daemon in the background and serve the current request in-process
- [ ] No start with `CI`, `MARA_NO_DAEMON=1` or the direct-mode flag
- [ ] A dispatched request is never replayed
