# Mara workflow

cli: Run `mara` for every operation's arguments. `--project <dir>` names the project root; without it, the nearest directory at or above the working directory that holds `.mara/config.toml` is the project. `--format json` prints one JSON object: the result, or `{"error": {"code", "message", "diagnostics"}}`. Exit status 0 is success or a valid result, 1 a failure or an invalid result, 2 rejected arguments.
mcp: Each tool returns one JSON object: the result, or `{"error": {"code", "message", "diagnostics"}}`.
mcp-bound: This server works on the one project it was started for; its tools take no `{{--project}}`.
mcp-unbound: Each tool takes `{{--project}}`, the project root, else found from where the server started. Pass it for another checkout or worktree.

## Find and read

- Documents are the files that `include` in `.mara/config.toml` matches.
- Start with `{{schema list}}`: types, ID prefixes and relations. `{{schema get <name>}}` gives a type's fields and allowed values, or a relation's targets per source type.
- `{{search <query>}}` finds items and narrative by their words; `{{item list}}` lists items. Narrow both with `{{--type}}`, `{{--field}}` `key=value`, `{{--relation}}`, `{{--path}}` and `{{--id}}`.
- `{{get <node>}}` reads an item with its path, line and MID, a whole document, or narrative. A node is an ID, a MID, a path from the project root, or `<path>:<line>`.
- `{{relation list <node>}}` lists relations both ways.
- `{{query list}}` names queries, views, invariants and gates; `{{query execute <name>}}`, `{{query view <name>}}` and `{{query gate <name>}}` run one, and `{{query report <name>}}` explains an outcome.
- Pass the cursor of a result cut short as `{{--cursor}}` for the next page; `{{--limit}}` sets the page size.

## Author

Name an item by its ID, MID or `<path>:<line>`, and types, relations, fields and values as the schema does, such as `{{--field}}` `status=draft`.

- `{{item create <type> <path>}}` needs `{{--title}}`, `{{--body}}` and a `{{--field}}` for `status` and for each required field. The ID is the type's prefix and the title in capitals; `{{--id}}` chooses another. It goes at the document's end or before `{{--line}}`; a missing document that `include` matches is created.
- `{{item update <item>}}` sets `{{--title}}` or `{{--body}}`, replaces all values of a field with `{{--field}}`, or clears an optional field with `{{--clear-field}}`.
- `{{relation add <source> <relation> <target>}}` and `{{relation remove <source> <relation> <target>}}` edit one row of the source's table, by the relation's forward name.
- `{{item move <item> <path>}}` (`{{--line}}` as for create) and `{{item rename <item> <new-id>}}` also rewrite every link to the item.
- `{{item delete <item>}}` is refused while relations, mentions or code markers outside the item name it; its diagnostics give each.
- If a failed edit lists replaced files, run `{{project fix}}`, then `{{project validate}}` and resolve errors before continuing with the item's current ID or MID. Otherwise reread before retrying.

## Repair after hand edits

Items written by hand may lack an anchor and have bare IDs as relation targets:

```markdown
---

## Passkeys replace passwords

| Type | Requirement |
| --- | --- |
| ID | REQ-PASSKEYS |
| Status | Draft |
| Satisfies | NEED-SIGN-IN |

Users sign in with a passkey.
```

- Put `---` between blank lines before each item except at the document's start. `{{project fix}}` adds a missing separator.
- Then run `{{project fix}}` once: it gives each such item an anchor with a new MID, links bare relation targets, and repairs stale document links.
- Never make up a MID, change one, or copy an anchor to another item.
- To mention an item in text, write `[<ID>](#<MID>)` with the MID from `{{get}}`; `{{project fix}}` writes the path.

## Link code

- In files indexed under `[code.languages.<name>]` in `.mara/config.toml`, put only `@mara <relation> <ID>` after the prefix on its own comment line. Use one forward relation and one ID, never a MID. `{{schema get <name>}}` must list the item's type under `code`.
- `{{relation list <node>}}` shows them, `{{project validate}}` reports `invalid-marker` and `unresolved-marker`, and `{{item rename <item> <new-id>}}` rewrites markers.

## Validate

- `{{project validate}}` checks the project, or what `{{--path}}` names. `{{item validate <item>}}` checks one item and `{{schema validate}}` the schema file. Any error makes the result invalid.
- After changing documents, run `{{project fix}}`, then `{{project validate}}`, and resolve every error.
- `invalid-project` means an error in the configuration or schema, which `{{project validate}}` locates; fix it by hand.
