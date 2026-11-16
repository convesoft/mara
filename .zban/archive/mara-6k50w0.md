---
title: "0.4.0-alpha.2: author through the CLI and MCP"
status: done
type: epic
blocked_by:
  - mara-t1rffa
  - mara-4y2cp4
created_at: 2026-11-13T22:10:06Z
closed_at: 2026-11-16T15:26:50Z
---

Slice outcome: items are created and revised through CLI and MCP operations, 0.3.0 corpora convert, and agents get their guidance from `mara prime` and the MCP server instructions.

Done when an agent can create, relate, move, rename and fix items in this repository through MCP without hand edits.

## Summary of Changes

Every issue of the slice is integrated on `chore/clear-worktree-for-rewrite`: the Verification items (mara-4y2cp4), the item writer and write procedure (mara-6ex559), `project init` (mara-sj9jfq), `item create`, `update`, `delete` and `relation add`, `remove` (mara-w0xasr, mara-s55h22), `item move` and `rename` (mara-esxqzv, mara-jnvzxr), `project fix` (mara-pvqpse), `project convert` (mara-wjy3sh, mara-0z37ab), the MCP server on `rmcp` (mara-8vqk4a), and `mara prime` with the agent guide as the server's instructions (mara-18193r).

"Done when", run by the orchestrator on 2026-11-16 at integration commit 8198bba: the binary built from that commit, started as `mara mcp` and driven over stdio with nothing but its `instructions` and `tools/list`, on a copy of this repository's `.mara/` and `docs/`:

- `project_validate`: valid, 0 errors, 0 warnings.
- `item_create` a Requirement in docs/authoring.mara.md and a Verification in docs/verification-authoring.mara.md; `relation_add` `verifies` between them.
- `item_move` the Requirement to docs/writes.mara.md: three files written, the inbound link path rewritten.
- `item_rename` it: the `ID` row and the inbound link text rewritten; `get` by the old ID answers `unknown-item`; `project_validate` valid.
- A third item written by hand without an anchor and with its relation named by ID: `project_validate` reports `incomplete-item`; one `project_fix` adds the anchor and writes the relation link; a second `project_fix` finds nothing.
- `item_delete` of the targeted Requirement is refused with `targeted-item`; after `relation_remove` and the deletes, `project_validate` is valid and `diff -r` shows the copy's `docs/` identical to the original.

One finding went to the backlog as mara-ytn283: a row whose key the type does not declare, which the orchestrator's own hand-written item had by mistake, cannot be removed by an operation. The decisions of the slice are listed in mara-5sqeze for the maintainer's review.

The slice can be tagged `v0.4.0-alpha.2` by the maintainer; the orchestrator creates no tag.
