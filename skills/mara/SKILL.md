---
name: mara
description: Use Mara to initialize, discover, author, relate, retrieve, search, or validate structured project knowledge in Git-tracked *.mara.md files.
---

# Mara project knowledge

Use the Mara MCP tools as the structured interface to a project's canonical
`*.mara.md` knowledge.

## Select the project

- Resolve the intended project root to an absolute path.
- Pass that path as `project` on every project-bound tool call unless the MCP
  server was explicitly started with `mara mcp --project PATH`.
- If `project` is omitted, Mara discovers the nearest project from the MCP
  server's execution directory.
- Treat each operation as scoped to one project. Do not infer workspace or
  cross-project behavior.

If the intended root has no `.mara/project.toml` and the user wants to start a
Mara project, call `project_init` with the absolute root, or omit `project` when
the MCP server was started with that root bound by `--project`. Use the default
`minimal` template unless the user explicitly requests `empty`. Do not create
or modify `AGENTS.md` as part of Mara onboarding.

## Choose the operation

| Intent | MCP operation |
|---|---|
| Discover vocabulary and field/edge constraints | `schema_list`, then `schema_get` for selected declarations; omit kind/name to get the full schema |
| Find items by text or enumerate exact filters | `item_search` or `item_list` |
| Read selected items, metadata, and direct relations | `item_get` |
| Inspect direct neighbours, optionally by direction/relation | `item_related`, then `item_get` for selected neighbour bodies |
| Create an item, optionally with initial edges | `item_create` |
| Change title, custom fields, or body | `item_update` |
| Relocate an item to a file/line | `item_move`; preserves ID and MID |
| Change a human ID and supported internal references | `item_rename`; preserves MID |
| Add or remove an existing item's typed edge | `relation_add` or `relation_remove` |
| Delete an unreferenced item | `item_delete`; resolve reported relation/mention blockers explicitly |
| Check an item or whole-project integrity | `item_validate` or `project_validate` |

Use mutations only when the user has asked to change project knowledge. Choose
the structured mutation for the semantic change. An invalid-argument error calls
for correcting the input or selecting the right operation; it is not a reason
to bypass validation by editing source lines. Mara source files remain canonical;
MCP results are not a separate authoring store.

## Retrieve enough context

For example, search with `{"query":"recovery","limit":5}`, select an ID from
the results, and pass it to `item_get` and `item_related` as `id`. Use the project
context selected above. Use `excerpts:true` on search when matching passages
help selection; excerpts may skip content and do not replace an item read.

Search, list, related, and get return `has_more` and `next_cursor`. When requested
content is incomplete, repeat the same operation with that opaque `cursor`,
keeping project, handle/query, filters, limit, and excerpt options unchanged.
Continue until the needed content is retrieved; full enumeration/read requires
`has_more:false`. Get can split body, metadata values, and relations across pages:
use their byte/index ranges to reconstruct content, including complete titles.
Restart without a cursor after source/schema changes. Related follows only direct
edges; choose further neighbours explicitly.

## Keep metadata inputs distinct

Call `schema_get` before authoring unfamiliar flavours, fields, or relations.
The shared `:key: value` source syntax does not make these interchangeable:

- **Structural metadata:** pass title through `title`. Mara generates the
  immutable MID; never supply, copy, or edit it. Creation `id` is a new human ID.
- **Custom fields:** use `fields:[{"key":"...","value":"..."}]` only for
  fields declared on that flavour. Supply required fields; repeat keys only
  when allowed. Update replaces all values of each supplied key; use
  `clear_fields` to remove optional keys. Omitted update values stay unchanged.
- **Typed relations:** `justifies` and `satisfies` are relations, not custom
  fields. `fields:[{"key":"justifies","value":"REQ-EXAMPLE"}]` is invalid.
  Use creation `relations` or explicit relation operations; inspect allowed
  source/target flavours first. Incoming backlinks are derived, never authored.

## Author and verify

1. Inspect the schema and resolve existing targets with `item_get`. In this
   example, the schema permits `decision` → `justifies` → `requirement`, and
   `REQ-EXAMPLE` already exists. Replace `/absolute/project` with the selected root,
   or omit `project` when the server is bound to it.
2. Call `item_create` with a meaningful body and any required custom fields:

```json
{
  "project": "/absolute/project",
  "flavour": "decision",
  "id": "ADR-EXAMPLE",
  "file": "decisions.mara.md",
  "title": "Keep edits recoverable",
  "body": "Preserve the previous content until validation succeeds so rejected edits can be retried."
}
```

3. Check `complete` and `missing`; a blank required body creates an incomplete
   scaffold. Fill it with `item_update` before claiming completion.
4. Call `relation_add` to add the edge:

```json
{
  "project": "/absolute/project",
  "source": "ADR-EXAMPLE",
  "relation": "justifies",
  "target": "REQ-EXAMPLE"
}
```

When initial edges are already known, prefer adding
`"relations":[{"relation":"justifies","target":"REQ-EXAMPLE"}]` to step 2
instead of step 4. Creation validates and publishes the item and initial edges
atomically, rejecting the whole request if an edge is invalid. Targets accept
exact human IDs or MIDs. Do not add the same edge again; use `relation_add` and
`relation_remove` for later changes (both take `source`, `relation`, `target`).

5. Call `item_get` with `id:"ADR-EXAMPLE"`; inspect generated MID, title, body,
   custom metadata, and outgoing relations. Call `item_related` with
   `id:"ADR-EXAMPLE",direction:"outgoing"`, then with
   `id:"REQ-EXAMPLE",direction:"incoming"` to verify both views of the edge.
6. Call `item_validate` with `id:"ADR-EXAMPLE"`; use `project_validate` for
   corpus-wide integrity after relation or reference changes. Use the same project
   context. Require `valid:true`, not just successful transport. Project
   validation `paths` filters reported diagnostics, not whole-project validity.

Update warnings about existing scaffold bodies still count as errors in explicit
validation. Pending transactions block mutations; use `project_transaction_rollback`
for explicit recovery after stopping other writers.
