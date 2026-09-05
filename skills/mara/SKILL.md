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

## Work with the corpus

1. Call `schema_get` before authoring unfamiliar flavours, fields, or relations.
2. Use `item_search`, `item_list`, and `item_related` for bounded discovery;
   call `item_get` only for selected full items.
3. Use `item_create`, `item_update`, `item_move`, `relation_add`, and
   `relation_remove` only when the user has asked to change project knowledge.
4. Run the narrowest relevant validation after a mutation and use
   `project_validate` when the requested work affects corpus-wide integrity.

Mara source files remain canonical. Do not treat MCP results as a separate
authoring store. Use `item_update` for partial title, custom-field, or body edits;
use `item_move` to relocate an item while preserving identity. Update warnings
about existing scaffold bodies still count as errors in explicit validation.
Rename and delete operations are not available yet; edit the source directly
and validate when those operations are needed.
