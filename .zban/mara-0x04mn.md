---
title: Teach code markers in the agent guide
status: todo
type: task
parent: mara-779tms
blocked_by:
  - mara-yhxq5x
discovered_from: mara-ttvp9z
created_at: 2026-11-16T16:43:26Z
---

Goal: an agent that writes code learns from `mara prime` and the MCP instructions how to link it to items with `@mara <relation> <ID>` markers.

Context: DES-AGENT-GUIDE (docs/agents.mara.md) lists the sections find and read, author, repair, validate; none mentions code markers (DES-CODE-MARKERS in docs/code.mara.md), although agents are the main authors of code and markers are how code links to requirements (REQ-CODE-MARKER-LINKS). The guide depends only on the binary, so it can show the marker form but not the project's relation names; it can send the agent to `schema get` for the relations whose sources include `code`. The MCP rendering must stay within 3,800 characters (VER-AGENT-GUIDE).

- [ ] Decide whether the guide gets a short code-links part (marker form, own line, one relation and one ID, `invalid-marker`/`unresolved-marker` in validation, `item rename` rewrites markers) and record it in DES-AGENT-GUIDE
- [ ] VER-AGENT-GUIDE still passes
