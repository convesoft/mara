---
title: Teach code markers in the agent guide
status: done
type: task
parent: mara-779tms
blocked_by:
  - mara-yhxq5x
discovered_from: mara-ttvp9z
created_at: 2026-11-16T16:43:26Z
closed_at: 2026-11-21T10:35:58Z
---

Goal: an agent that writes code learns from `mara prime` and the MCP instructions how to link it to items with `@mara <relation> <ID>` markers.

Context: DES-AGENT-GUIDE (docs/agents.mara.md) lists the sections find and read, author, repair, validate; none mentions code markers (DES-CODE-MARKERS in docs/code.mara.md), although agents are the main authors of code and markers are how code links to requirements (REQ-CODE-MARKER-LINKS). The guide depends only on the binary, so it can show the marker form but not the project's relation names; it can send the agent to `schema get` for the relations whose sources include `code`. The MCP rendering must stay within 3,800 characters (VER-AGENT-GUIDE).

- [x] Decide whether the guide gets a short code-links part (marker form, own line, one relation and one ID, `invalid-marker`/`unresolved-marker` in validation, `item rename` rewrites markers) and record it in DES-AGENT-GUIDE
- [x] VER-AGENT-GUIDE still passes

## Summary of Changes

- `crates/mara/src/cli/guide.md`: new section `## Link code` between repair and validate: markers count in files a `[code.languages.<name>]` table includes; form `@mara <relation> <ID>` on a comment line, one relation, one ID, never a MID; `schema get` for relations with targets for `code`; `relation list` shows the links, `project validate` reports `invalid-marker` and `unresolved-marker`, `item rename` rewrites markers. Other text shortened so that the MCP rendering stays within 3,800 characters (3,783 unbound); nothing DES-AGENT-GUIDE requires was dropped.
- DES-AGENT-GUIDE records the section and why; VER-AGENT-GUIDE's section list names it; `tests/prime.rs` and `tests/mcp/guide.rs` check it in order.
- Checked against the binary in a disposable project: relation list, both diagnostics and the marker rewrite by `item rename` behave as the guide says. `mara code add` is not mentioned (mara-h0hxa3).

Checks: cargo fmt --check, clippy (warnings denied), cargo test --workspace, project validate (valid, 1 existing warning), performance test in release, wasm32 build of mara-engine, zban check: all pass.

## Decisions made

- The guide gets a separate `## Link code` section (orchestrator's answer) rather than bullets under Author; recorded in DES-AGENT-GUIDE, and VER-AGENT-GUIDE's section list extended to match.
- The section also says that markers count only in files of a `[code.languages.<name>]` table, since `project init` writes no `[code]` and markers elsewhere do nothing.
