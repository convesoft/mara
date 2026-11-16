---
title: "Client routing: on-demand start, first call in-process"
status: in-progress
type: feature
parent: mara-0mxw21
blocked_by:
  - mara-17gykj
created_at: 2026-11-13T22:10:06Z
---

Context: REQ-FIRST-CALL-IN-PROCESS and DES-DAEMON-LIFECYCLE in docs/daemon.mara.md. The MCP server becomes a client of the daemon, so the "Each is a cold request" bullet of DES-MCP-SERVER (docs/hosts.mara.md) changes with this issue.

- [ ] CLI and MCP clients start the daemon in the background and serve the current request in-process
- [ ] No start with `CI`, `MARA_NO_DAEMON=1` or the direct-mode flag
- [ ] A dispatched request is never replayed
- [ ] Test: the other cases of VER-DAEMON-START, and the SIGKILL cases of VER-DAEMON-WRITES (docs/verification-daemon.mara.md)

Note from mara-gfb6n6 (2026-11-16): `query execute`, `query view` and `query gate` take `--input <name>=@<file>`. Once a client sends an operation to the daemon, such a path is resolved from the client's working directory, not the daemon's (read it in the client, or send the working directory with the request, as for `--project`).
