# Mara workflow

This project keeps Mara items in Markdown documents, checked against its schema.

cli: Run `mara` for every operation's arguments. `--project <dir>` names the project root; without it, the nearest directory at or above the working directory that holds `.mara/config.toml` is the project. `--format json` prints one JSON object: the result, or `{"error": {"code", "message", "diagnostics"}}`. Exit status 0 is success or a valid result, 1 a failure or an invalid result, 2 rejected arguments.
mcp: Each tool returns one JSON object: the result, or `{"error": {"code", "message", "diagnostics"}}`.
mcp-bound: This server works on the one project it was started for; its tools take no `{{--project}}`.
mcp-unbound: Each tool takes `{{--project}}`, the project root; without it, the project is found from where the server started. Pass it when you work in another checkout or worktree.

## Find and read

- The documents are the files that `include` in `.mara/config.toml` matches, such as `docs/**/*.mara.md`.
- Start with `{{schema list}}`, the types with their ID prefixes and the relations. `{{schema get <name>}}` gives a type's fields and allowed values, or a relation's targets for each source type.
- `{{search <query>}}` finds items and narrative by their words; `{{item list}}` lists items. Narrow both with `{{--type}}`, `{{--field}}` `key=value`, `{{--relation}}`, `{{--path}}` and `{{--id}}`.
- `{{get <node>}}` reads an item with its path, line and MID, a whole document, or narrative. A node is an ID, a MID, a path from the project root, or `<path>:<line>`.
- `{{relation list <node>}}` lists an item's relations both ways, each with its line.
- A result cut short gives a cursor: pass it as `{{--cursor}}` for the next page. `{{--limit}}` sets the page size.

## Author

Name an item by its ID, its MID or `<path>:<line>`. Write types, relations, fields and values by the names the schema gives, such as `{{--field}}` `status=draft`.

- `{{item create <type> <path>}}` needs `{{--title}}`, `{{--body}}` and a `{{--field}}` for `status` and for each required field. The ID is the type's prefix and the title in capitals; `{{--id}}` chooses another. It ends the document, or goes before the line `{{--line}}` gives; a missing document is created when `include` matches it.
- `{{item update <item>}}` changes `{{--title}}` or `{{--body}}`, replaces every value of a field with `{{--field}}`, or removes an optional field with `{{--clear-field}}`.
- `{{relation add <source> <relation> <target>}}` and `{{relation remove <source> <relation> <target>}}` write or remove one row in the source's table, by the relation's forward name.
- `{{item move <item> <path>}}` (`{{--line}}` as for create) and `{{item rename <item> <new-id>}}` also rewrite every link to the item.
- `{{item delete <item>}}` is refused while another item or narrative links to it; its diagnostics give each such link.
- A refused request changes no file, but a `changed-file` stop may have replaced some. When its message names replaced files, run `{{project fix}}` and `{{project validate}}`, then go on with the item's current ID or its MID; otherwise read again and repeat the request.

## Repair after hand edits

An item, or many at once, may be written by hand without an anchor and with relation targets as bare IDs:

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

- Put a `---` line with a blank line on each side before every item, the first one after a title or other text included; leave it out only for an item that starts the document, which never starts with `---`. A forgotten one is reported, and `{{project fix}}` adds it.
- Then run `{{project fix}}` once. It gives each such item an anchor with a new MID, turns bare IDs in relation rows into links, and repairs link paths and texts that moving or renaming items by hand left stale.
- Never make up a MID, change one, or copy an anchor to another item.
- To mention an item in text, write `[<ID>](#<MID>)` with the MID from `{{get}}`; `{{project fix}}` writes the path.

## Validate

- `{{project validate}}` checks the project, or the files and directories given by `{{--path}}`. `{{item validate <item>}}` checks one item and `{{schema validate}}` the schema file. Any error makes the result invalid.
- After changing documents, run `{{project fix}}`, then `{{project validate}}`, and resolve every error before you finish.
- `invalid-project` means the configuration or the schema has an error: `{{project validate}}` reports where; fix it by hand.
