---
name: mara
description: Use Mara to discover and read items and narrative, or author and validate structured project knowledge in Git-tracked *.mara.md files.
---

# Mara project knowledge

Prefer the Mara MCP tools for a project's canonical `*.mara.md` knowledge.
When MCP is unavailable, use an available Mara CLI invocation with `--format json`
for structured results. The same operation selection, authoring, continuation,
and validation rules apply to both surfaces.

This skill targets the 0.2 interface: schema format 2 and unified `search`,
`get`, and `related`. Use the skill shipped with the selected executable or
the same source revision. If an older installation exposes a different
interface, report the mismatch and use its matching guidance; do not silently
change the version pin or substitute removed commands.

## Resolve the CLI fallback

Installing the skill does not install `mara` on PATH. Reuse the configured MCP
launcher: keep its executable, runner arguments, exact package version, and
environment; replace the `mcp` operation with the required CLI operation. Carry
any bound project into the CLI's `--project` option.

For the Bash examples below, put that launcher in an argument array `mara_cli`:
`mara_cli=(npx -y '@convesoft/mara@<configured-version>')` (replace the placeholder
with the existing exact pin), or `mara_cli=('/absolute/path/to/mara')` for a direct
executable. Use `mara_cli=(mara)` only when `command -v mara` resolves it.
Check `"${mara_cli[@]}" --version` and the operation's `--help`; preserve the pin
and use only supported options. If no launcher is available, report the missing
executable or runner rather than assuming a PATH installation or changing versions.

## Select the project

- Resolve the intended project root to an absolute path.
- Pass that path as `project` on every project-bound tool call unless the MCP
  server was explicitly started with `mara mcp --project PATH`.
- If `project` is omitted, Mara discovers the nearest project from the MCP
  server's execution directory.
- For CLI calls, use `"${mara_cli[@]}" --project /absolute/project --format json ...`.
  Without `--project`, the CLI discovers from its working directory.
- Treat each operation as scoped to one project. Do not infer workspace or
  cross-project behavior.

If the intended root has no `.mara/project.toml` and the user wants to start a
Mara project, call `project_init` with the absolute root, or omit `project` when
the MCP server was started with that root bound by `--project`. Use the default
`minimal` template unless the user explicitly requests `empty` or `engineering`.
Pass the selected name as `template` to `project_init`.
`engineering` includes engineering flavours, selection guidance, and traceability
relations; all templates generate configuration and schema only. The CLI equivalent
is `"${mara_cli[@]}" --project /absolute/project --format json project init --template <template>`,
where `<template>` is the selected `minimal`, `empty`, or `engineering` name.
Do not create or modify `AGENTS.md` as part of Mara onboarding.

## Choose vocabulary from the schema

Call `schema_list` with `{"kind":"flavour"}`, then `schema_get` with
`{"kind":"flavour","name":"requirement"}` for a candidate. CLI equivalents
are `schema list flavour` and `schema get flavour requirement`.

- `description` states purpose; `use_when` identifies suitable knowledge.
- `avoid_when` excludes unsuitable content; `distinguish_from` compares
  confusable flavours. Read the alternative declaration when the distinction
  affects your choice.
- `id_prefix`, `body`, and `fields` specify creation constraints. Guidance keys
  belong to the schema declaration, not an item's `fields` or body.

Use the selected project's declarations, including custom flavours. Keep
supporting narrative as Markdown when it does not need an independent identity;
search/get/related can still discover, read, and navigate it.

Schema format 2 requires all four guidance keys directly on every flavour:
a nonblank `description`, a nonempty list of nonblank `use_when` entries,
an `avoid_when` list (`[]` is valid), and a `distinguish_from` mapping (`{}` is
valid). Distinction targets must be other declared flavours with nonblank
explanations. When asked to migrate format 1, edit the existing schema in place,
set `format_version: 2`, and supply meaningful guidance. Preserve custom
flavours, prefixes, fields, relations, document bytes, IDs, and MIDs; do not
reinitialize or replace the schema with a template. Require `valid:true` from
both `schema_validate` and `project_validate` (CLI `schema validate` and
`project validate`).

For the engineering template, inspect `schema_get` relation declarations before
connecting items. `verification` describes a repeatable check; `evidence`
records its result. The added relations are `verifies` (verification →
requirement/design), `validates` (verification → goal/scenario), `evidences`
(evidence → verification), `implements` (artifact → requirement/design),
`affects` (risk → affected knowledge), and `mitigates`
(requirement/design/decision/verification → risk). Add only meaningful links;
no complete trace chain or placeholder items are required. Existing projects
do not gain these declarations automatically.

## Choose the operation

CLI entries below follow `"${mara_cli[@]}" --project /absolute/project --format json`;
inspect `<command> --help` for positional arguments and options.

| Intent | MCP operation | CLI command |
|---|---|---|
| Discover vocabulary and field/edge constraints | `schema_list` with kind, then `schema_get`; omit kind/name for the full schema | `schema list flavour` or `schema list relation`, then `schema get` |
| Search items and narrative, or list items with exact filters | `search` or `item_list` | `search`, `item list` |
| Read an item, section, Markdown block, or document | `get` | `get` |
| Inspect direct connections from any node, then read a selected neighbour | `related`, then `get` | `related`, then `get` |
| Create an item, optionally with initial edges | `item_create` | `item create` |
| Change title, custom fields, or body | `item_update` | `item update` |
| Relocate an item; preserve ID and MID | `item_move` | `item move` |
| Change human ID and supported references; preserve MID | `item_rename` | `item rename` |
| Add or remove an existing item's typed edge | `relation_add` or `relation_remove` | `relation add`, `relation remove` |
| Delete an item; resolve reported relation/mention blockers | `item_delete` | `item delete` |
| Check an item or whole-project integrity | `item_validate` or `project_validate` | `item validate`, `project validate` |

Use mutations only when the user has asked to change project knowledge. Choose
the structured mutation for the semantic change. An invalid-argument error calls
for correcting the input or selecting the right operation; it is not a reason
to bypass validation by editing source lines. Mara source files remain canonical;
MCP results are not a separate authoring store.

## Discovery and reading

Call `search` with `{"query":"recovery","limit":5}`. Results contain
`{node, excerpt}`; pass a selected `node.reference` to `get` as
`{"reference":"<selected reference>"}`. Get also accepts exact item IDs/MIDs.
Use the project context selected above. One source excerpt is included per search
hit; it may omit content and does not replace a consecutive read. Item ID,
flavour, custom-field, and schema-relation filters select items only; path
filters also cover narrative. There is no node-kind filter.

Get returns `node`, `content`, `content_range`, `metadata`, and `metadata_range`.
Items return their parsed body and ordered metadata; other nodes return their
original Markdown span, including contained source for sections and documents,
with empty metadata. Read `node.context.parent` or `node.context.section` through
get when structural context is needed. Get does not enumerate neighbours or
accept `limit`.

Call `related` with `{"reference":"<selected reference>"}` for direct schema
relations, mentions, and containment. It returns `node` and
`connections:[{relation,direction,neighbour,source}]`; pass a selected
`neighbour.reference` to `get` or another `related` call. Each call follows only
direct connections; there is no automatic expansion or hops option.

Use `direction:"incoming"` or `"outgoing"`; omission includes both. Related
`relations` accepts `schema:name` and `builtin:name`, with short names allowed
only when unambiguous in the vocabulary. Related `flavours` selects item
neighbours only. JSON represents containment as `contains` with direction;
human output displays its incoming view as `contained_by`. To find sibling
context, inspect `related` with `relations:["builtin:contains"]` and
`direction:"incoming"`, then select the parent's outgoing containment. Read
chosen children with `get`.

Search, item list, related, and get return `has_more` and `next_cursor`. Repeat
the same operation with that opaque `cursor`, keeping project, reference/query,
filters, and any supported limit unchanged. Continue until the needed content
is retrieved; full enumeration/read requires `has_more:false`. Get splits
consecutive content and metadata values across pages: use their byte/index
ranges to reconstruct complete values, including titles and repeated metadata.
Restart without a cursor after source/schema changes. Structural discovery
handles identify source in a document snapshot; if stale, search again.
Item MIDs retain durable identity. Search and related default to 20 entries
and accept `limit` from 1 through 100; related counts connections, including
different connections to the same neighbour. The byte budget may shorten pages.

Unified discovery responses use `format_version: 1`, independently of schema
format 2 and the application version. Inspect `node.kind` (item, section, block,
or document); only items have ID/MID/flavour. Item list retains its item-only
response. On upgrade, discard old cursors and update parsers for the mixed
`results`, consecutive `content`, and `connections` shapes above.

CLI retrieval uses the same JSON result fields:
`"${mara_cli[@]}" --project /absolute/project --format json search recovery --limit 5`,
then `"${mara_cli[@]}" --project /absolute/project --format json get '<reference>'`.
Use `related '<reference>'` for connections, `--relation builtin:mentions` to
select explicit mentions, and `--cursor '<next_cursor>'` for continuation.

## Preserve authored references

Use `[[ID]]`/`[[MID]]` in item bodies or narrative for item mentions, and
Markdown links for documents, heading sections, or explicit anchors, for example
`[policy](./policies.mara.md#retry-policy)`. Resolve relative paths from the
linking document. `mentions` and containment are derived; author them in
Markdown, not with relation mutations. Code examples and escaped references
remain literal. External URLs are not network-validated.

Rename rewrites typed relation targets and supported wiki mentions in items and
narrative, preserving MID. Create/update validate new internal references;
create/update/move/delete reject changes that break or retarget surviving links,
including generated anchors and links to nodes inside an item. Move can affect
relative links. Delete can be blocked by references to contained sections or
blocks. Resolve reported source locations before retrying; Markdown links are
not automatically repaired. Do not bypass a rejected mutation with raw edits.

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

CLI equivalents are `--title`, repeatable `--field KEY=VALUE`, update
`--clear-field KEY`, and creation `--relation NAME=TARGET`. Never pass a relation
through `--field`. CLI `--body -` reads stdin; MCP `body` is literal text.

## Author and verify

1. Inspect the schema and resolve existing targets with `get` using `reference`.
   In this example, the schema permits `decision` → `justifies` → `requirement`, and
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

5. Call `get` with `reference:"ADR-EXAMPLE"`; inspect `node` for the generated
   MID/title, `content` for the body, and `metadata` for authored values.
   Call `related` with `reference:"ADR-EXAMPLE",direction:"outgoing"`, then with
   `reference:"REQ-EXAMPLE",direction:"incoming"` to verify both views of the edge.
6. Call `item_validate` with `id:"ADR-EXAMPLE"`; use `project_validate` for
   corpus-wide integrity after relation or reference changes. Use the same project
   context. Require `valid:true`, not just successful transport. Project
   validation `paths` filters reported diagnostics, not whole-project validity.

Update warnings about existing scaffold bodies still count as errors in explicit
validation. Pending transactions block mutations; use `project_transaction_rollback`
(`project transaction rollback` in CLI) for explicit recovery after stopping
other writers.

For the same authoring workflow through CLI, after resolving `mara_cli` and
`REQ-EXAMPLE`, use initial relations atomically and inspect both directions:

```bash
mara_project=/absolute/project
"${mara_cli[@]}" --project "$mara_project" --format json schema get
"${mara_cli[@]}" --project "$mara_project" --format json get REQ-EXAMPLE
"${mara_cli[@]}" --project "$mara_project" --format json item create decision ADR-EXAMPLE decisions.mara.md \
  --title 'Keep edits recoverable' \
  --body 'Preserve the previous content until validation succeeds so rejected edits can be retried.' \
  --relation justifies=REQ-EXAMPLE
"${mara_cli[@]}" --project "$mara_project" --format json get ADR-EXAMPLE
"${mara_cli[@]}" --project "$mara_project" --format json related ADR-EXAMPLE --direction outgoing
"${mara_cli[@]}" --project "$mara_project" --format json related REQ-EXAMPLE --direction incoming
"${mara_cli[@]}" --project "$mara_project" --format json project validate
```
